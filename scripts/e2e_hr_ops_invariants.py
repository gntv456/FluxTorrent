#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""H&R 可信度与运维不变式闸门（0286）。

覆盖 2026-10-06 第三轮实测（H&R 判定质量 / 促销留痕 / 捐赠档位）：
  K1  管理组手动关掉的下载权限不被 H&R 作业自动放开（旧：5 分钟内自动失效）
  K2  enable_hr 是真开关（旧：后台能改、代码零读）
  K3  促销倍率落进 traffic_ledger.promotion_kind + reason（旧：死列，无法追溯）
  K4  捐赠档位按 (人, 档) 只发一次的台账在位（结构断言）

用法：python scripts/e2e_hr_ops_invariants.py
需要：栈在跑（api/worker/postgres/redis/tracker），探针号 status<2。
"""
import base64
import binascii
import hashlib
import hmac
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

API = os.environ.get("FLUX_API_BASE", "http://127.0.0.1:8080/api/v1")
TRACKER = os.environ.get("FLUX_TRACKER", "http://127.0.0.1:7070")
PROBE = int(os.environ.get("FLUX_PROBE", "39"))
ROOT = 1
TORRENT = int(os.environ.get("FLUX_TORRENT", "40"))
INFO_HASH = os.environ.get(
    "FLUX_INFO_HASH", "1acaa148ab6bd78d5a19d274836b8e97081da6ce")
results = []


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if r.returncode:
        raise RuntimeError("psql 失败：" + r.stderr.strip())
    return r.stdout.strip()


def secret():
    txt = open(os.environ.get("FLUX_ENV", "docker/.env"),
               encoding="utf-8").read()
    return re.search(r"^JWT_SECRET=(.+)$", txt, re.M).group(1) \
        .strip().strip('"').strip("'")


def b64(raw):
    return base64.urlsafe_b64encode(raw).rstrip(b"=")


def jwt(sub, cls, ttl=600):
    now = int(time.time())
    h = b64(json.dumps({"alg": "HS256", "typ": "JWT"},
                       separators=(",", ":")).encode())
    p = b64(json.dumps({"sub": sub, "class_id": cls, "iat": now,
                        "exp": now + ttl}, separators=(",", ":")).encode())
    sig = b64(hmac.new(secret().encode(), h + b"." + p,
                       hashlib.sha256).digest())
    return b".".join([h, p, sig]).decode()


def call(tok, method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(API + path, method=method,
                                 headers={"Authorization": "Bearer " + tok,
                                          "Content-Type": "application/json"},
                                 data=data)
    try:
        with urllib.request.urlopen(req, timeout=25) as r:
            return r.status, json.loads(r.read().decode() or "{}")
    except urllib.error.HTTPError as e:
        raw = e.read().decode()
        try:
            return e.code, json.loads(raw)
        except Exception:
            return e.code, {"raw": raw[:200]}


def announce(passkey, up, down, left):
    q = urllib.parse.urlencode(
        {"info_hash": binascii.unhexlify(INFO_HASH),
         "peer_id": b"AX52syhrops0000001", "port": "6881",
         "uploaded": str(up), "downloaded": str(down), "left": str(left),
         "event": "started", "numwant": "0"},
        quote_via=urllib.parse.quote)
    url = "%s/announce/%s?%s" % (TRACKER, passkey, q)
    with urllib.request.urlopen(url, timeout=15) as r:
        body = r.read()
    return "OK" if b"failure" not in body else body[:60].decode(
        "utf-8", "replace")


def run_job(tok, job, timeout=200):
    """触发 worker 任务并等它真的跑完（job_triggers.status: pending→done）"""
    st, _ = call(tok, "POST", "/admin/jobs/run", {"job": job})
    if st != 200:
        return False, "触发失败 HTTP %s" % st
    end = time.time() + timeout
    while time.time() < end:
        time.sleep(4)
        s = psql("SELECT COALESCE(status,'-') FROM job_triggers "
                 "WHERE job='%s' ORDER BY id DESC LIMIT 1" % job)
        if s != "pending":
            return s == "done", s
    return False, "超时仍 pending"


def run_hr_chain(tok):
    """H&R 的两个作业：建快照/判定在 hr_enforce，处罚与自动恢复在 hr_punish。"""
    a, ia = run_job(tok, "hr_enforce")
    if not a:
        return False, "hr_enforce=" + ia
    b, ib = run_job(tok, "hr_punish")
    return b, "enforce=%s punish=%s" % (ia, ib)


def check(name, ok, detail=""):
    results.append((name, ok))
    print(("  PASS  " if ok else "  FAIL  ") + name
          + (("  [%s]" % detail) if detail else ""))


def main():
    print("api=%s probe=%d torrent=%d" % (API, PROBE, TORRENT))
    root = jwt(ROOT, 99)
    if call(root, "GET", "/me")[0] != 200:
        print("自检失败：root JWT 不可用")
        return 2
    print("自检通过\n")
    pk = psql("SELECT passkey FROM users WHERE id=%d" % PROBE)
    try:
        # ---------- K1 手动冻结不被自动放开 ----------
        print("[K1] 管理组关闭的下载权限，H&R 作业不得自动放开")
        psql("UPDATE users SET status=0, download_enabled=TRUE, "
             "download_locked_by=NULL WHERE id=%d" % PROBE)
        # 注意：/admin/users/flags 是 PUT-only（POST 会 404）
        st, _ = call(root, "PUT", "/admin/users/flags",
                     {"user_id": PROBE, "download_enabled": False})
        lock = psql("SELECT COALESCE(download_locked_by,'-') FROM users "
                    "WHERE id=%d" % PROBE)
        frozen = psql("SELECT download_enabled FROM users WHERE id=%d" % PROBE)
        check("K1a 手动关下载会记录锁来源 staff",
              st == 200 and lock == "staff" and frozen in ("f", "false"),
              "HTTP %s locked_by=%s enabled=%s" % (st, lock, frozen))
        ok_job, info = run_hr_chain(root)
        still = psql("SELECT download_enabled FROM users WHERE id=%d" % PROBE)
        check("K1b hr_punish 跑完后 staff 冻结仍在（旧：自动放开）",
              ok_job and still in ("f", "false"),
              "job=%s download_enabled=%s" % (info, still))
        # 正向对照：H&R 自己下的锁仍要能自动解，否则等于把自助闭环改成纯人工
        psql("UPDATE users SET download_enabled=FALSE, "
             "download_locked_by='hr' WHERE id=%d" % PROBE)
        ok_job2, info2 = run_hr_chain(root)
        auto = psql("SELECT download_enabled FROM users WHERE id=%d" % PROBE)
        check("K1c hr 自锁在违规数低于阈值时仍自动放开",
              ok_job2 and auto in ("t", "true"),
              "job=%s download_enabled=%s" % (info2, auto))
        psql("UPDATE users SET download_enabled=TRUE, "
             "download_locked_by=NULL WHERE id=%d" % PROBE)

        # ---------- K2 enable_hr 真生效 ----------
        print("\n[K2] enable_hr=no 时不再建 H&R 快照")
        psql("DELETE FROM hr_snapshots WHERE user_id=%d AND torrent_id=%d"
             % (PROBE, TORRENT))
        psql("DELETE FROM snatches WHERE user_id=%d AND torrent_id=%d"
             % (PROBE, TORRENT))
        psql("INSERT INTO snatches (user_id, torrent_id, uploaded, "
             "downloaded, last_up, last_down, leeching, seeding, "
             "completed_at, last_seen_at, seeded_seconds) VALUES "
             "(%d, %d, 0, 20971520, 0, 0, FALSE, FALSE, now(), now(), 0)"
             % (PROBE, TORRENT))
        psql("UPDATE site_settings SET value='no' WHERE name='enable_hr'")
        run_job(root, "hr_enforce")
        off = int(psql("SELECT count(*) FROM hr_snapshots WHERE user_id=%d "
                       "AND torrent_id=%d" % (PROBE, TORRENT)))
        psql("UPDATE site_settings SET value='yes' WHERE name='enable_hr'")
        run_job(root, "hr_enforce")
        on = int(psql("SELECT count(*) FROM hr_snapshots WHERE user_id=%d "
                      "AND torrent_id=%d" % (PROBE, TORRENT)))
        check("K2 关闭=不建快照 / 开启=建快照", off == 0 and on >= 1,
              "off=%d on=%d" % (off, on))

        # ---------- K3 促销倍率落账 ----------
        print("\n[K3] 促销倍率写进 traffic_ledger.promotion_kind")
        psql("DELETE FROM promotions WHERE torrent_id=%d" % TORRENT)
        psql("DELETE FROM traffic_ledger WHERE user_id=%d AND torrent_id=%d"
             % (PROBE, TORRENT))
        psql("UPDATE snatches SET last_up=0, last_down=0, uploaded=0, "
             "downloaded=0, last_seen_at=now(), seeded_seconds=0 "
             "WHERE user_id=%d AND torrent_id=%d" % (PROBE, TORRENT))
        psql("INSERT INTO promotions (scope, torrent_id, kind, starts_at, "
             "ends_at, source) VALUES ('torrent', %d, 'x2', "
             "now() - interval '1 hour', now() + interval '1 hour', "
             "'manual')" % TORRENT)
        announce(pk, 1073741824, 1073741824, 0)
        got = False
        for _ in range(24):
            time.sleep(5)
            row = psql("SELECT promotion_kind || '|' || "
                       "COALESCE(reason,'-') FROM traffic_ledger "
                       "WHERE user_id=%d AND torrent_id=%d "
                       "ORDER BY id DESC LIMIT 1" % (PROBE, TORRENT))
            if row:
                got = True
                break
        check("K3 落账带促销码与出处", got and row.startswith("2|"),
              row if got else "没等到流水")
        check("K3b 上传按 x2 计入",
              got and int(psql("SELECT delta_up FROM traffic_ledger "
                               "WHERE user_id=%d AND torrent_id=%d "
                               "ORDER BY id DESC LIMIT 1"
                               % (PROBE, TORRENT))) >= 2 * 1073741824 - 1,
              "" if not got else psql(
                  "SELECT delta_up || '/' || delta_down FROM traffic_ledger "
                  "WHERE user_id=%d AND torrent_id=%d ORDER BY id DESC "
                  "LIMIT 1" % (PROBE, TORRENT)))
        psql("DELETE FROM promotions WHERE torrent_id=%d" % TORRENT)

        # ---------- K4 结构断言 ----------
        print("\n[K4] 捐赠档位一次性台账（结构）")
        pk_ok = psql("SELECT count(*) FROM pg_constraint WHERE conrelid = "
                     "'user_donation_tiers'::regclass AND contype = 'p'")
        cols = psql("SELECT string_agg(column_name, ',' ORDER BY "
                    "ordinal_position) FROM information_schema.columns "
                    "WHERE table_name='user_donation_tiers'")
        check("K4 user_donation_tiers 主键 (user_id, min_usd) 在位",
              pk_ok == "1" and cols == "user_id,min_usd,tier_name,"
              "order_no,granted_at", cols)
    finally:
        psql("DELETE FROM hr_snapshots WHERE user_id=%d AND torrent_id=%d"
             % (PROBE, TORRENT))
        psql("DELETE FROM snatches WHERE user_id=%d AND torrent_id=%d"
             % (PROBE, TORRENT))
        psql("DELETE FROM traffic_ledger WHERE user_id=%d" % PROBE)
        psql("DELETE FROM promotions WHERE torrent_id=%d AND source='manual'"
             % TORRENT)
        psql("UPDATE users SET status=0, download_enabled=TRUE, "
             "download_locked_by=NULL, uploaded=0, downloaded=0 "
             "WHERE id=%d" % PROBE)
        psql("UPDATE site_settings SET value='yes' WHERE name='enable_hr'")
        print("\n[cleanup] 探针快照/snatch/流水/促销/开关/下载锁 已复位")

    failed = [n for n, ok in results if not ok]
    print("\n===== %d/%d 通过 ====="
          % (len(results) - len(failed), len(results)))
    if failed:
        print("未通过：" + ", ".join(failed))
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
