# -*- coding: utf-8 -*-
"""闸门 CU：下载凭证/磁力口的付费与资格闸必须三处同口径（P-344）。

实测红线（批一百五十四）：同一个付费种、同一时刻——

    GET  /torrents/{id}/magnet  → 400「该种子为付费种子（N 魔力）…」  ← 闸在
    POST /downloads/keys       → 200 + fxk_ 凭证，库里真落一行        ← 闸漏

`issue_download_key()` 被三个口共用（POST /downloads/keys、
GET /crossseed/dl/{id}、NP 兼容层），而 `ensure_downloadable` 与付费判定
当初只写在 magnet 那一条路径上——于是「点一下下载」是一条完整的绕过扣费
通道。

本闸钉六件事（都在活体上做，临时给一颗他人付费种标价，收尾复原）：
  S1 写口前置：`issue_download_key` 必须同时调 `ensure_downloadable` 与
     付费判定（源码级，防有人日后把闸删掉）。
  S2 未购用户对付费种签凭证 → 必须被拒，且**库里一行都不许多**。
  S3 同场景 magnet 也拒（口径一致）。
  S4 正控：免费种仍能正常签出凭证（防「修成谁都不能签」）。
  S5 正控：付费种发布者本人仍能签（owner 放行）。
  S6 收尾：临时价格复原、download_keys / api_tokens 零残留。

取证的坑记在文件尾：require_token 要 `Authorization: Token <明文>`，
签发口是 POST /me/tokens（需 password 复核），scope 只有 read/upload/crossseed。
"""
import io
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import login, ok, summary  # noqa: E402

BASE = os.environ.get("FLUX_API_BASE", "http://127.0.0.1:8100/api/v1")
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
KEYS_FILE = os.path.join(REPO, "apps/api/src/compat_http/download_keys.rs")
TNAME = []
ORIG_PRICE = {}
TOUCHED = []


def call(method, path, body=None, auth=None):
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    if auth:
        req.add_header("Authorization", auth)
    data = json.dumps(body).encode() if body is not None else None
    try:
        r = urllib.request.urlopen(req, data, timeout=25)
        return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read())
        except Exception:  # noqa: BLE001
            return e.code, {}


def psql(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
         "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    return (r.stdout or "").strip()


ROOT = login()


def api_token():
    name = "cuprobe%d" % (int(time.time()) % 100000)
    s, r = call("POST", "/me/tokens",
                {"name": name, "scopes": ["read"], "password": "password123"},
                auth="Bearer " + ROOT)
    d = r.get("data") or {}
    tok = d.get("token") or d.get("plaintext") or d.get("key")
    if tok:
        TNAME.append(name)
    return tok


print("== S1 源码级：签发口必须同时带资格闸与付费闸 ==")
src = io.open(KEYS_FILE, encoding="utf-8").read()
fn = src[src.index("pub(crate) async fn issue_download_key"):]
fn = fn[:fn.index("\n}\n")]
ok("S1 签发口调用了 ensure_downloadable（下载资格）",
   "ensure_downloadable" in fn, "")
ok("S1 签发口判了付费（torrent_purchases）",
   "torrent_purchases" in fn and "price" in fn, "")

XT = api_token()
# 素材：「他人发布 + root 未购」的已审种。现网的付费种全是 root
# 自己发的（owner 放行），而付费门只能在非作者场景下被激活——
# 所以取一颗他人的已审种，临时标价后再测（收尾复原）。
PAID = psql("SELECT t.id FROM torrents t WHERE t.approval_status = 1 "
            "AND t.owner_id <> 1 "
            "AND t.id NOT IN (SELECT torrent_id FROM torrent_purchases "
            "WHERE user_id = 1) ORDER BY t.id LIMIT 1")


if not XT:
    print("[BLOCKED] 签不到 API Token（环境问题），S2~S5 跳过")
    summary()
    raise SystemExit(1)
if not PAID:
    print("[BLOCKED] 库里没有「他人发布且 root 未购」的已审种，S2/S3 跳过")
    print("   （不是产品红——没素材。本闸不越权造种子）")
    summary()
    raise SystemExit(1)

PAID = int(PAID)
# 免费种与付费种的正控素材（必须在 PAID 定下来之后取，
# 否则 id <> 那个排除得到的是空串 → 语法错/结果为空 → S4 静默跳过）。
FREE = psql("SELECT id FROM torrents WHERE price = 0 AND approval_status = 1 "
            "AND id <> %d LIMIT 1" % PAID)
OWN_SEED = psql("SELECT id FROM torrents WHERE price > 0 "
                "AND approval_status = 1 AND owner_id = 1 LIMIT 1")
print("== S2/S3 未购付费种：凭证口与磁力口都必须拒 ==")
ORIG_PRICE[PAID] = psql("SELECT price FROM torrents WHERE id = %d" % PAID)
psql("UPDATE torrents SET price = 999 WHERE id = %d" % PAID)
try:
    before = psql("SELECT count(*) FROM download_keys WHERE torrent_id = %d"
                  % PAID)
    s, r = call("POST", "/downloads/keys", {"torrent_id": PAID},
                auth="Token " + XT)
    after = psql("SELECT count(*) FROM download_keys WHERE torrent_id = %d"
                 % PAID)
    ok("S2 未购用户对付费种签凭证被拒（红线：200 + 落库）",
       s != 200 and int(after) == int(before),
       "http=%s 库行 %s→%s" % (s, before, after))
    ok("S2 回的是业务码与可读文案（不是 500）",
       r.get("code") in (1002, 402, 403),
       "code=%s msg=%s" % (r.get("code"), str(r.get("message"))[:60]))
    s2, r2 = call("GET", "/torrents/%d/magnet" % PAID, auth="Bearer " + ROOT)
    ok("S3 同场景 magnet 也拒（两口口径一致）", s2 != 200,
       "http=%s" % s2)

    print("== S4/S5 正控：免费种与发布者本人仍要能签 ==")
    if FREE:
        TOUCHED.append(int(FREE))
        s3, r3 = call("POST", "/downloads/keys", {"torrent_id": int(FREE)},
                      auth="Token " + XT)
        ok("S4 免费种正常签发（防修成谁都不能签）",
           s3 == 200 and bool((r3.get("data") or {}).get("download_url")),
           "http=%s" % s3)
    own = OWN_SEED
    if own:
        TOUCHED.append(int(own))
        s4, r4 = call("POST", "/downloads/keys", {"torrent_id": int(own)},
                      auth="Token " + XT)
        ok("S5 付费种发布者本人能签（owner 放行是既定口径）",
           s4 == 200, "http=%s %s" % (s4, str(r4.get("message"))[:40]))
finally:
    for tid, price in ORIG_PRICE.items():
        psql("UPDATE torrents SET price = %s WHERE id = %d" % (price, tid))
    psql("DELETE FROM download_keys WHERE torrent_id = %d" % PAID)


def cleanup():
    for n in TNAME:
        psql("DELETE FROM api_tokens WHERE name = '%s'" % n)
    # S4/S5 的正控也真签了凭证（那是它们应当的行为），收尾一并清。
    for t in TOUCHED:
        psql("DELETE FROM download_keys WHERE torrent_id = %d" % t)
    left = psql("SELECT count(*) FROM download_keys")
    tk = psql("SELECT count(*) FROM api_tokens WHERE name LIKE 'cuprobe%%'")
    price_ok = all(psql("SELECT price FROM torrents WHERE id = %d" % t) == p
                   for t, p in ORIG_PRICE.items())
    ok("S6 收尾：价格复原 / download_keys 复原 / 探针 Token 清零",
       price_ok and left == "0" and tk == "0",
       "价格复原=%s keys=%s tok=%s" % (price_ok, left, tk))


try:
    summary()
finally:
    cleanup()
