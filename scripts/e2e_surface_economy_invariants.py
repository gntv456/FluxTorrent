#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""对外接口面 + 经济玩法不变式闸门（0285 第二批）。

覆盖 2026-10-06 版主第二轮深测（社区治理 / 对外接口面 / 经济防刷）里
已修且可本机复验的缺陷：
  J1  商店重放不再铸造卡牌（旧：同键 qty=1 后再 qty=100 → 白拿 99 张）
  J2  详情聚合缓存不再把待审/被拒种泄漏给普通会员
  J3  RSS 不再暴露匿名上传者的真名
  J5  禁言真的挡发帖/评论/站短（写 users.forumpost + 三通道拒绝）
  J6  API 令牌跟随 token_revocations（改密/踢会话后立刻失效）

用法：python scripts/e2e_surface_economy_invariants.py
可覆盖：FLUX_API_BASE / FLUX_PROBE / FLUX_TORRENT
脚本会写库（探针号），结束时自清。
"""
import base64
import hashlib
import hmac
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request

API = os.environ.get("FLUX_API_BASE", "http://127.0.0.1:8080/api/v1")
PROBE = int(os.environ.get("FLUX_PROBE", "39"))
ROOT = 1
TORRENT = int(os.environ.get("FLUX_TORRENT", "40079"))
results = []


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if r.returncode:
        raise RuntimeError("psql 失败：" + r.stderr.strip())
    return r.stdout.strip()


def redis_get(pat):
    pw = re.search(r"^REDIS_PASSWORD=(.+)$",
                   open(os.environ.get("FLUX_ENV", "docker/.env"),
                        encoding="utf-8").read(), re.M).group(1).strip()
    r = subprocess.run(
        ["docker", "exec", "flux-redis", "redis-cli", "-a", pw,
         "--no-auth-warning", "--scan", "--pattern", pat],
        capture_output=True, text=True)
    return [x for x in r.stdout.split("\n") if x.strip()]


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


def call(tok, method, path, body=None, raw_auth=None):
    auth = raw_auth or ("Bearer " + tok)
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(API + path, method=method,
                                 headers={"Authorization": auth,
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


def check(name, ok, detail=""):
    results.append((name, ok))
    print(("  PASS  " if ok else "  FAIL  ") + name
          + (("  [%s]" % detail) if detail else ""))


def shop_probe():
    """返回 (可消费的探针 token, 说明)。临时密码未改的账号花不了钱。"""
    flag = psql("SELECT must_reset_password FROM users WHERE id=%d" % PROBE)
    if flag == "t":
        return None, "探针号仍是临时密码（花钱端点会拒），J1 需要人工准备"
    psql("UPDATE users SET spark_balance = 0 WHERE id=%d" % PROBE)
    return jwt(PROBE, 1), ""


def main():
    print("api=%s probe=%d torrent=%d" % (API, PROBE, TORRENT))
    root = jwt(ROOT, 99)
    st, _ = call(root, "GET", "/me")
    if st != 200:
        print("自检失败：root JWT 不可用（%s）" % st)
        return 2
    print("自检通过：root token 可用\n")
    try:
        # ---------- J5 禁言落地 ----------
        print("[J5] 禁言是否真的挡住发言")
        topic = psql("SELECT id FROM topics ORDER BY id LIMIT 1")
        psql("UPDATE users SET status=0, forumpost=TRUE WHERE id=%d" % PROBE)
        p = jwt(PROBE, 1)
        st_ok, d_ok = call(p, "POST", "/torrents/%d/comments" % TORRENT,
                           {"body": "E2E 禁言探针：未禁言时应可发言"})
        check("J5a 未禁言时评论可发（阳性对照）", st_ok == 200,
              "HTTP %s %s" % (st_ok, str(d_ok.get("message", ""))[:40]))
        st, _ = call(root, "POST", "/admin/users/status",
                     {"user_id": PROBE, "status": 1, "reason": "E2E 禁言"})
        fp = psql("SELECT forumpost FROM users WHERE id=%d" % PROBE)
        check("J5b 禁言写进 forumpost（旧版只改 status，论坛侧读不到）",
              st == 200 and fp == "f", "HTTP %s forumpost=%s" % (st, fp))
        c1 = call(p, "POST", "/torrents/%d/comments" % TORRENT,
                  {"body": "E2E 禁言探针：应被拒"})
        c2 = call(p, "POST", "/forums/topics/%s/reply" % topic,
                  {"body": "E2E 禁言探针：应被拒"})
        c3 = call(p, "POST", "/messages",
                  {"receiver_id": ROOT, "subject": "E2E", "body": "E2E"})
        blocked = [c1[0], c2[0], c3[0]]
        check("J5c 评论/回帖/站短三通道一律拒绝",
              all(code in (400, 403) for code in blocked),
              "codes=%s" % blocked)
        psql("DELETE FROM comments WHERE user_id=%d AND body LIKE 'E2E%%'"
             % PROBE)
        psql("UPDATE users SET status=0, forumpost=TRUE WHERE id=%d" % PROBE)

        # ---------- J2 聚合缓存越权 ----------
        print("\n[J2] 待审种不得经共享缓存泄漏")
        psql("UPDATE torrents SET approval_status=0 WHERE id=%d" % TORRENT)
        for k in redis_get("cache:tdetail:v1:%d:*" % TORRENT):
            subprocess.run(["docker", "exec", "flux-redis", "redis-cli", "-a",
                            re.search(r"^REDIS_PASSWORD=(.+)$",
                                      open("docker/.env",
                                           encoding="utf-8").read(),
                                      re.M).group(1).strip(),
                            "--no-auth-warning", "DEL", k],
                           capture_output=True)
        call(jwt(ROOT, 99), "GET", "/torrents/%d/aggregate" % TORRENT)
        leaked_keys = redis_get("cache:tdetail:v1:%d:*" % TORRENT)
        st_member, _ = call(jwt(PROBE, 1), "GET",
                            "/torrents/%d/aggregate" % TORRENT)
        check("J2a 待审种不进共享缓存", not leaked_keys,
              "keys=%s" % leaked_keys[:2])
        check("J2b 普通会员取待审种 = 404", st_member == 404,
              "HTTP %s" % st_member)
        psql("UPDATE torrents SET approval_status=1 WHERE id=%d" % TORRENT)

        # ---------- J3 RSS 匿名保护 ----------
        print("\n[J3] RSS 不得暴露匿名上传者真名")
        owner = psql("SELECT u.username FROM torrents t"
                     " JOIN users u ON u.id=t.owner_id WHERE t.id=%d"
                     % TORRENT)
        tname = psql("SELECT name FROM torrents WHERE id=%d" % TORRENT)
        psql("UPDATE torrents SET anonymous=TRUE WHERE id=%d" % TORRENT)
        feed = ""
        try:
            pk_probe = psql("SELECT passkey FROM users WHERE id=%d" % PROBE)
            req = urllib.request.Request("%s/rss/%s" % (API, pk_probe))
            with urllib.request.urlopen(req, timeout=25) as r:
                feed = r.read().decode("utf-8", "replace")
        except Exception as e:  # noqa: BLE001
            print("     RSS 取回失败：%s" % e)
        items = re.findall(r"<item>.*?</item>", feed, re.S)
        mine = [x for x in items if tname[:24] in x]
        check("J3 RSS 命中该匿名种", bool(mine),
              "条目 %d/%d" % (len(mine), len(items)))
        check("J3b 条目里不出现上传者账号名",
              bool(mine) and owner not in mine[0],
              "owner=%s" % owner)
        psql("UPDATE torrents SET anonymous=FALSE WHERE id=%d" % TORRENT)

        # ---------- J6 API 令牌跟随撤销线 ----------
        print("\n[J6] 改密/踢会话后 API 令牌必须失效")
        st, d = call(jwt(PROBE, 1), "POST", "/me/tokens",
                     {"name": "E2E 探针", "scopes": ["read"]})
        tok = ((d.get("data") or {}).get("token")
               or (d.get("data") or {}).get("plaintext"))
        check("J6a 令牌可签发", st == 200 and bool(tok), "HTTP %s" % st)
        ok_before = call(None, "GET", "/compat/nexusphp/user.json",
                         raw_auth="Token %s" % tok)[0] if tok else 0
        psql("INSERT INTO token_revocations (user_id, nbf) VALUES (%d, "
             "EXTRACT(EPOCH FROM now() + interval '1 hour')::bigint) "
             "ON CONFLICT (user_id) DO UPDATE SET "
             "nbf = GREATEST(token_revocations.nbf, EXCLUDED.nbf),"
             " updated_at = now()" % PROBE)
        ok_after = call(None, "GET", "/compat/nexusphp/user.json",
                        raw_auth="Token %s" % tok)[0] if tok else 0
        check("J6b 撤销线之后令牌立即 401（旧版继续可用 180 天）",
              ok_before == 200 and ok_after == 401,
              "撤销前=%s 撤销后=%s" % (ok_before, ok_after))
        psql("DELETE FROM token_revocations WHERE user_id=%d" % PROBE)
        psql("DELETE FROM api_tokens WHERE user_id=%d AND name='E2E 探针'"
             % PROBE)

        # ---------- J1 商店重放 ----------
        print("\n[J1] 商店重放不得铸造卡牌")
        item = psql("SELECT id || '|' || price FROM shop_items "
                    "WHERE kind='temp_invite' AND active LIMIT 1")
        tok_p, why = shop_probe()
        if not item or tok_p is None:
            check("J1 商店重放（前置不满足）", False,
                  why or "没有可用的 temp_invite 商品")
        else:
            item_id, price = item.split("|")
            psql("UPDATE users SET spark_balance=%d WHERE id=%d"
                 % (int(price) * 3, PROBE))
            psql("DELETE FROM user_vouchers WHERE user_id=%d"
                 " AND kind='invite' AND expires_at > now()" % PROBE)
            n0 = int(psql("SELECT count(*) FROM user_vouchers"
                          " WHERE user_id=%d" % PROBE))
            s1, d1 = call(tok_p, "POST", "/shop/buy",
                          {"item_id": int(item_id), "qty": 1,
                           "idempotency_key": "e2e-j1"})
            n1 = int(psql("SELECT count(*) FROM user_vouchers"
                          " WHERE user_id=%d" % PROBE))
            s2, d2 = call(tok_p, "POST", "/shop/buy",
                          {"item_id": int(item_id), "qty": 100,
                           "idempotency_key": "e2e-j1"})
            n2 = int(psql("SELECT count(*) FROM user_vouchers"
                          " WHERE user_id=%d" % PROBE))
            check("J1 同键重放 qty=100 不再白拿 99 张（旧行为）",
                  s1 == 200 and n1 == n0 + 1 and n2 == n1,
                  "首次 +%d，重放 +%d（HTTP %s %s）"
                  % (n1 - n0, n2 - n1, s2,
                     str((d2.get("data") or {}))[:40]))
            psql("DELETE FROM user_vouchers WHERE user_id=%d" % PROBE)
            psql("DELETE FROM shop_orders WHERE user_id=%d"
                 " AND idempotency_key LIKE 'shop:%d:e2e-j1%%'"
                 % (PROBE, PROBE))
            psql("DELETE FROM spark_ledger WHERE user_id=%d"
                 " AND idempotency_key LIKE 'shop:%d:e2e-j1%%'"
                 % (PROBE, PROBE))
            psql("UPDATE users SET spark_balance=0 WHERE id=%d" % PROBE)
    finally:
        cleanup()

    failed = [n for n, ok in results if not ok]
    print("\n===== %d/%d 通过 ====="
          % (len(results) - len(failed), len(results)))
    if failed:
        print("未通过：" + ", ".join(failed))
        return 1
    return 0


def cleanup():
    psql("UPDATE users SET status=0, forumpost=TRUE WHERE id=%d" % PROBE)
    psql("UPDATE torrents SET approval_status=1 WHERE id=%d" % TORRENT)
    psql("DELETE FROM comments WHERE user_id=%d AND body LIKE 'E2E%%'"
         % PROBE)
    psql("DELETE FROM messages WHERE sender_id=%d AND body LIKE 'E2E%%'"
         % PROBE)
    psql("DELETE FROM token_revocations WHERE user_id=%d" % PROBE)
    psql("DELETE FROM api_tokens WHERE user_id=%d AND name='E2E 探针'"
         % PROBE)
    print("\n[cleanup] 探针发言/待审态/匿名态/令牌/撤销线 均已复位")


if __name__ == "__main__":
    sys.exit(main())
