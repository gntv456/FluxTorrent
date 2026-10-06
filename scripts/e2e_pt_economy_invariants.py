#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""PT 经济与安全不变式闸门（0285）。

覆盖 2026-10-05 版主实测审计里「本机实测复现」的缺陷，修完必须一直绿：
  I1  announce 计数器回放不再无限铸造上传量
  I2  announce 幂等键表（announce_seen）在位并随事件增长
  I3  管理端补流量走流水、不被下一次 announce 抹掉
  I4  用户详情接口不再下发 passkey 明文
  I5  reveal/reset 需专项权限 + 严格高于目标 + 理由必填
  I6  审计读模型带出现场（目标与理由）
  I7  审核队列下发内容面
  I8  同一举报人对同一目标不再产生重复工单

用法：python scripts/e2e_pt_economy_invariants.py
可覆盖：FLUX_API_BASE / FLUX_TRACKER / FLUX_PROBE / FLUX_TORRENT /
        FLUX_INFO_HASH / FLUX_ENV
前置：栈已起（api/worker/tracker/postgres/redis），迁移到 HEAD。
脚本会写库（探针号），结束时自清；对 root 只读 + 临时改级。
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
    "FLUX_INFO_HASH", "1acaa148ab6bd78d5a19d274836b8e97081da6ce"
)
BIG = 500_000_000_000
PEER = b"AX52syinvariants01"
GB = 1073741824
results = []


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", q],
        capture_output=True, text=True)
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
    claims = {"sub": sub, "class_id": cls, "iat": now, "exp": now + ttl}
    p = b64(json.dumps(claims, separators=(",", ":")).encode())
    body = h + b"." + p
    sig = b64(hmac.new(secret().encode(), body, hashlib.sha256).digest())
    return b".".join([h, p, sig]).decode()


def call(tok, method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(
        API + path, method=method,
        headers={"Authorization": "Bearer " + tok,
                 "Content-Type": "application/json"},
        data=data)
    try:
        with urllib.request.urlopen(req, timeout=20) as r:
            return r.status, json.loads(r.read().decode() or "{}")
    except urllib.error.HTTPError as e:
        raw = e.read().decode()
        try:
            return e.code, json.loads(raw)
        except Exception:
            return e.code, {"raw": raw[:200]}


def announce(passkey, up, down=0, left=0, event="started"):
    q = urllib.parse.urlencode(
        {"info_hash": binascii.unhexlify(INFO_HASH),
         "peer_id": PEER, "port": "6881", "uploaded": str(up),
         "downloaded": str(down), "left": str(left), "event": event,
         "numwant": "0"},
        quote_via=urllib.parse.quote)
    url = "%s/announce/%s?%s" % (TRACKER, passkey, q)
    try:
        with urllib.request.urlopen(url, timeout=15) as r:
            body = r.read()
        return "OK" if b"failure" not in body else body[:60].decode(
            "utf-8", "replace")
    except Exception as e:  # noqa: BLE001
        return "ERR %s" % e


def ledger(uid):
    v = psql("SELECT count(*), COALESCE(sum(delta_up),0) "
             "FROM traffic_ledger WHERE user_id=%d" % uid)
    parts = (v.split("|") + ["0", "0"])[:2]
    return int(parts[0]), int(parts[1])


def wait_ledger(uid, at_least, secs=150):
    end = time.time() + secs
    while time.time() < end:
        time.sleep(5)
        if ledger(uid)[0] >= at_least:
            return True
    return False


def check(name, ok, detail=""):
    results.append((name, ok))
    mark = "PASS" if ok else "FAIL"
    print("  %s  %s%s" % (mark, name, ("  [%s]" % detail) if detail else ""))


def upload_of(uid):
    return int(psql("SELECT uploaded FROM users WHERE id=%d" % uid))


def main():
    print("api=%s tracker=%s probe=%d torrent=%d" %
          (API, TRACKER, PROBE, TORRENT))
    old_class = psql("SELECT class_id FROM users WHERE id=%d" % PROBE)
    st, _ = call(jwt(ROOT, 99), "GET", "/me")
    if st != 200:
        print("自检失败：root JWT 不可用（%s），断言不可信" % st)
        return 2
    print("自检通过：root token 可用\n")
    root_tok = jwt(ROOT, 99)
    try:
        # ---------- I1 / I2 ----------
        pk = psql("SELECT passkey FROM users WHERE id=%d" % PROBE)
        reset_snatch(PROBE)
        psql("DELETE FROM traffic_ledger WHERE user_id=%d"
             " AND torrent_id=%d" % (PROBE, TORRENT))
        psql("DELETE FROM announce_seen WHERE user_id=%d" % PROBE)
        base_n, base_up = ledger(PROBE)
        print("[I1] 回放攻击：同一个 500GB 计数反复上报")
        for up in (BIG, 0, BIG, 0, BIG):
            announce(pk, up)
            time.sleep(1.5)
        wait_ledger(PROBE, base_n + 1, 120)
        n1, up1 = ledger(PROBE)
        credited = up1 - base_up
        ceiling = int(psql("SELECT COALESCE("
                           "(SELECT value FROM site_settings"
                           " WHERE name='traffic_credit_max_bps'),"
                           " '2147483648')"))
        bound = 60 * ceiling
        check("I1 回放不再无限铸造（旧：1.5TB）",
              credited <= bound,
              "入账 %s B / 参照上限 %s B"
              % (format(credited, ","), format(bound, ",")))
        hits = psql("SELECT count(*) FROM cheat_events WHERE agent LIKE"
                    " 'speed:%d' AND reason LIKE 'over_ceiling%%'" % TORRENT)
        check("I1b 超上限部分留痕 cheat_events", hits != "0", "rows=%s" % hits)
        seen = psql("SELECT count(*) FROM announce_seen")
        check("I2 announce_seen 有行（每事件一行）", int(seen) > 0,
              "rows=%s" % seen)

        # ---------- I3 ----------
        print("\n[I3] 补 1GB 后用户再 announce，量是否还在")
        psql("UPDATE users SET uploaded=0 WHERE id=%d" % PROBE)
        psql("DELETE FROM traffic_ledger WHERE user_id=%d"
             " AND torrent_id IS NULL" % PROBE)
        st, _ = call(root_tok, "POST", "/admin/users/adjust",
                     {"user_id": PROBE, "uploaded_delta": GB,
                      "note": "E2E 补偿理由"})
        after_adjust = upload_of(PROBE)
        rows = psql("SELECT count(*) FROM traffic_ledger WHERE user_id=%d"
                    " AND torrent_id IS NULL"
                    " AND reason LIKE '%%E2E 补偿理由%%'" % PROBE)
        announce(pk, 0, 0, 1048576)
        for _ in range(24):
            time.sleep(5)
            if upload_of(PROBE) >= GB:
                break
        after_announce = upload_of(PROBE)
        check("I3 adjust 返回 200", st == 200, "HTTP %s" % st)
        check("I3b 补量写进流水（带理由与操作者）", rows != "0",
              "rows=%s" % rows)
        check("I3c announce 后补量未被抹掉（旧：归零）",
              after_adjust == GB and after_announce >= GB,
              "adjust 后 %s / announce 后 %s" %
              (format(after_adjust, ","), format(after_announce, ",")))

        # ---------- I4 / I5 / I6 ----------
        print("\n[I4/I5/I6] 以 class 92（论坛版主）身份打管理端")
        psql("UPDATE users SET class_id=92 WHERE id=%d" % PROBE)
        mod = jwt(PROBE, 92)
        root_pk = psql("SELECT passkey FROM users WHERE id=%d" % ROOT)
        st, d = call(mod, "GET", "/admin/users/%d" % ROOT)
        body = json.dumps(d, ensure_ascii=False)
        check("I4 详情接口不含明文 passkey",
              root_pk not in body and "passkey_masked" in body,
              "HTTP %s" % st)
        st, d = call(mod, "POST", "/admin/warned",
                     {"user_id": ROOT, "weeks": 1,
                      "reason": "E2E 越权探测"})
        check("I5 版主不能警告站长（严格高于目标）", st == 403,
              "HTTP %s %s" % (st, d.get("message", "")))
        warned = psql("SELECT COALESCE(warned_reason,'') FROM users"
                      " WHERE id=%d" % ROOT)
        check("I5b 库里确实没写进警告", warned == "", repr(warned[:30]))
        st, _ = call(mod, "POST", "/admin/users/passkey/reveal",
                     {"user_id": ROOT, "reason": "E2E 越权探测"})
        check("I5c 版主不能 reveal 站长 passkey", st in (401, 403),
              "HTTP %s" % st)
        # 注意目标选探针号：ensure_outranks 要求「严格高于」，
        # 站长 reveal 自己的 passkey 本就该走自助 /me/passkey/rotate。
        st, d = call(root_tok, "POST", "/admin/users/passkey/reveal",
                     {"user_id": PROBE, "reason": "E2E 正向用例"})
        probe_pk = psql("SELECT passkey FROM users WHERE id=%d" % PROBE)
        check("I5d 站长可 reveal 低等级成员（需理由）",
              st == 200 and probe_pk in json.dumps(d), "HTTP %s" % st)
        st, _ = call(root_tok, "POST", "/admin/users/passkey/reveal",
                     {"user_id": PROBE, "reason": ""})
        check("I5e 空理由被拒", st == 400, "HTTP %s" % st)
        row = psql("SELECT action || '|' || COALESCE(ref::text,'')"
                   " FROM audit_log WHERE action='user.passkey.reveal'"
                   " ORDER BY id DESC LIMIT 1")
        check("I6 审计现场带目标与理由",
              '"reason"' in row and '"id"' in row, row[:70])

        # ---------- I7 ----------
        print("\n[I7] 审核队列内容面")
        psql("UPDATE torrents SET approval_status=0"
             " WHERE id IN (40049, 40079)")
        st, d = call(root_tok, "GET", "/admin/reviews")
        data = d.get("data")
        # 0288 起该端点返回分页对象 {items,limit,offset,total}；旧版是裸数组
        rows = (data.get("items") if isinstance(data, dict) else data) or []
        keys = set(rows[0].keys()) if rows else set()
        need = {"small_descr", "descr_excerpt", "numfiles", "screenshots",
                "owner_name", "category_name", "dup_hash",
                "owner_approved"}
        missing = sorted(need - keys)
        check("I7 队列下发内容字段", st == 200 and rows and not missing,
              "%d 条，缺 %s" % (len(rows), missing or "无"))

        # ---------- I8 ----------
        print("\n[I8] 举报去重")
        victim = jwt(PROBE, 1)
        psql("DELETE FROM reports WHERE reporter_id=%d"
             " AND ref_type='torrent' AND ref_id=%d" % (PROBE, TORRENT))
        st1, _ = call(victim, "POST", "/reports",
                      {"ref_type": "torrent", "ref_id": TORRENT,
                       "reason": "E2E 重复举报探测"})
        st2, d2 = call(victim, "POST", "/reports",
                       {"ref_type": "torrent", "ref_id": TORRENT,
                        "reason": "E2E 重复举报探测"})
        dup = (d2.get("data") or {}).get("duplicate")
        n = psql("SELECT count(*) FROM reports WHERE reporter_id=%d"
                 " AND ref_type='torrent' AND ref_id=%d"
                 " AND status IN (0,2)" % (PROBE, TORRENT))
        check("I8 第二次同目标举报不再新增工单",
              st1 == 200 and st2 == 200 and dup is True and n == "1",
              "%s/%s duplicate=%s 队列行=%s" % (st1, st2, dup, n))
    finally:
        cleanup()

    failed = [n for n, ok in results if not ok]
    print("\n===== %d/%d 通过 =====" %
          (len(results) - len(failed), len(results)))
    if failed:
        print("未通过：" + ", ".join(failed))
        return 1
    return 0


def reset_snatch(uid):
    # last_seen_at 一并归零：否则首报的「距上次上报秒数」是历史大值，
    # allowance 会把 500GB 全额放行，断言就测不出回放防线了。
    psql("UPDATE snatches SET last_up=0, last_down=0, uploaded=0,"
         " downloaded=0, seeded_seconds=0, last_seen_at=now()"
         " WHERE user_id=%d AND torrent_id=%d" % (uid, TORRENT))


def cleanup():
    psql("DELETE FROM traffic_ledger WHERE user_id=%d" % PROBE)
    psql("DELETE FROM announce_seen WHERE user_id=%d" % PROBE)
    psql("DELETE FROM cheat_events WHERE agent LIKE 'speed:%d'"
         " AND reason LIKE 'over_ceiling%%'" % TORRENT)
    reset_snatch(PROBE)
    psql("UPDATE users SET class_id=1, uploaded=0 WHERE id=%d" % PROBE)
    psql("UPDATE users SET warned_until=NULL, warned_reason=NULL"
         " WHERE id=%d" % ROOT)
    psql("DELETE FROM reports WHERE reporter_id=%d"
         " AND reason LIKE 'E2E%%'" % PROBE)
    psql("UPDATE torrents SET approval_status=1"
         " WHERE id IN (40049, 40079)")
    print("\n[cleanup] 探针流水/幂等行/作弊事件/警告/重复举报/待审状态已复位")


if __name__ == "__main__":
    sys.exit(main())
