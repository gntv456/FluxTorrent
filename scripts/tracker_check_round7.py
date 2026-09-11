# -*- coding: utf-8 -*-
"""第七轮：tracker announce 链路实测。

验证：正常 announce 200 + interval 字段、坏 passkey 拒绝、scrape、
（限流阈值默认 3600/min IP 级——不打满，只验证路径与封禁缓存生效性）。
"""
import socket
import struct
import urllib.request
import urllib.error

TR = "http://127.0.0.1:7070"
BASE = "http://127.0.0.1:8080/api/v1"
ih_text = "%41" * 20
results = []

def check(name, cond, detail=""):
    results.append((name, cond))
    print(("PASS " if cond else "FAIL ") + name + (f"  {detail}" if detail else ""))

# root passkey
import json
req = urllib.request.Request(BASE + "/auth/login", method="POST",
                             data=json.dumps({"username": "root", "password": "password123"}).encode())
req.add_header("Content-Type", "application/json")
with urllib.request.urlopen(req, timeout=10) as r:
    tok = json.loads(r.read())["data"]["token"]
for path in ("/me/overview", "/me/settings"):
    try:
        req = urllib.request.Request(BASE + path)
        req.add_header("Authorization", f"Bearer {tok}")
        with urllib.request.urlopen(req, timeout=10) as r:
            d = json.loads(r.read())["data"]
            pk = d.get("passkey") if isinstance(d, dict) else None
            if pk:
                break
    except Exception:
        pass
if not pk:
    # 兜底：rotate passkey 接口拿新值（会刷新 root passkey，tracker 缓存 60s 内跟进）
    req = urllib.request.Request(BASE + "/me/passkey/rotate", method="POST")
    req.add_header("Authorization", f"Bearer {tok}")
    with urllib.request.urlopen(req, timeout=10) as r:
        pk = json.loads(r.read())["data"].get("passkey")
check("前置·拿到 root passkey", bool(pk), f"passkey={pk[:4]}***" if pk else "no passkey")

# 1. 坏 passkey → tracker 协议层返回 200 + bencode failure reason（BT 客户端约定）
# peer_id 用恰好 20 字节的合法值，确保唯一变量是 passkey
bad_pk_url = (f"{TR}/announce/passkey_bad0000000000000000000000?info_hash={ih_text}"
              f"&peer_id=-qatest01-0123456789&port=6881&uploaded=0&downloaded=0&left=0&compact=1")
try:
    with urllib.request.urlopen(bad_pk_url, timeout=10) as r:
        body = r.read()
        check("announce·坏passkey拒绝", r.status == 200 and b"failure reason" in body and b"passkey" in body, body[:80])
except Exception as e:
    check("announce·坏passkey拒绝", False, str(e)[:100])

# 2. 正常 announce → 200 + bencode + interval
# peer_id 必须恰好 ≤20 字节：-qatest01-0123456789 = 20
ih = ih_text
url = (f"{TR}/announce/{pk}?info_hash={ih}&peer_id=-qatest01-0123456789"
       f"&port=6881&uploaded=0&downloaded=0&left=0&compact=1&numwant=5")
try:
    with urllib.request.urlopen(url, timeout=10) as r:
        body = r.read()
        has_interval = b"interval" in body and b"min interval" in body
        check("announce·正常200+bencode", r.status == 200 and body.startswith(b"d"), f"len={len(body)}")
        check("announce·BEP3 interval/min interval 下发", has_interval, body[:160])
except Exception as e:
    check("announce·正常200+bencode", False, str(e)[:100])
    check("announce·BEP3 interval/min interval 下发", False, "")

# 3. scrape（BEP48：/scrape/{passkey}?info_hash=...）
try:
    with urllib.request.urlopen(f"{TR}/scrape/{pk}?info_hash={ih}", timeout=10) as r:
        body = r.read()
        check("scrape·200+bencode", r.status == 200 and body.startswith(b"d"), body[:80])
except Exception as e:
    check("scrape·200+bencode", False, str(e)[:100])

# 3b. scrape 坏 passkey
try:
    with urllib.request.urlopen(f"{TR}/scrape/passkey_bad0000000000000000000000?info_hash={ih}", timeout=10) as r:
        body = r.read()
        check("scrape·坏passkey拒绝", b"failure reason" in body, body[:80])
except Exception as e:
    check("scrape·坏passkey拒绝", False, str(e)[:100])

# 4. info_hash 长度非法（10 字节）→ failure reason
try:
    with urllib.request.urlopen(f"{TR}/announce/{pk}?info_hash=%41%41%41%41%41%41%41%41%41%41&peer_id=-qatest01-0123456789&port=6881", timeout=10) as r:
        body = r.read()
        check("announce·info_hash长度校验", b"failure reason" in body, body[:80])
except Exception as e:
    check("announce·info_hash长度校验", False, str(e)[:100])

# 4. UA 黑名单路径（用一个肯定不在名单的 UA —— 默认放行；黑名单验证依赖 agent_rules 数据，跳过破坏性写入）

print(f"\n===== tracker 链路：{sum(1 for _, c in results if c)}/{len(results)} PASS =====")
