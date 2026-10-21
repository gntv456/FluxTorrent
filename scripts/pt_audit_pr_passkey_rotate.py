# -*- coding: utf-8 -*-
"""闸门 PR：轮换 passkey 必须复核本人（P-409，2026-10-21 批二百三十六）。

旧形状（P-409 在册，`e9dabc79` 起记录）：`POST /me/passkey/rotate` 只要会话，
无旧密码、无 TOTP、无 body 就 http=200 —— 一次请求同时发新钥、废旧钥、
撤下载键与 RSS token，且**不触发任何登出联动**。会话被偷的人既白拿一份
新的 tracker/RSS 身份，又把受害者所有下载器与命名订阅打成 401；同文件
`/me/password/change` 却要旧密码 ⇒ 一个文件两把尺。

修复：先 step_up（密码必对；开了 2FA 再要当前动态码），并按用户 5 次/分
限流（复核会试密码，不能把这里变成第二个 oracle）；前端拆出
passkey-rotate.tsx 带密码/动态码输入。

本闸门在同一支探针号上验证：
  A 空 body / 错密码被拒且 passkey 不动；错密码 12 次内必现 429；
  B 开 2FA 后只给密码被拒、密码+动态码可轮换（正控）；
  S 后端/前端静态钉桩，旧形状判红。
夹具跑完清 2FA、停用、删邀请，回读零残留。
"""
import atexit
import base64
import hashlib
import hmac
import os
import re
import struct
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, dig, login, ok, summary)

ROOT = login()
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
TAG = int(time.time()) % 100000
UNAME = "pkr%d" % TAG
OLD = "password123"
SECRET = "JBSWY3DPEHPK3PXP"
UID = None
CID = None


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8",
        errors="replace")
    if r.returncode != 0:
        raise RuntimeError((r.stderr or "").strip()[:200])
    return (r.stdout or "").strip()


def ex(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-c", q],
        capture_output=True, text=True, encoding="utf-8",
        errors="replace")
    if r.returncode != 0:
        print("   ! SQL: %s" % (r.stderr or "").strip()[:150])
    return r.returncode


def cleanup():
    if UID:
        for t, col in (("messages", "receiver_id"),
                       ("email_verifications", "user_id"),
                       ("password_resets", "user_id"),
                       ("spark_ledger", "user_id"),
                       ("token_revocations", "user_id"),
                       ("login_events", "user_id")):
            ex("DELETE FROM %s WHERE %s = %d" % (t, col, UID))
        ex("UPDATE users SET totp_secret = NULL, totp_enabled = FALSE "
           "WHERE id = %d" % UID)
        ex("UPDATE users SET status = 2 WHERE id = %d" % UID)
    if CID:
        ex("DELETE FROM invites WHERE id = %s" % CID)


atexit.register(cleanup)


def passkey_db():
    return psql("SELECT passkey FROM users WHERE id = %d" % UID)


def totp(secret, step=None):
    key = base64.b32decode(secret)
    if step is None:
        step = int(time.time()) // 30
    msg = struct.pack(">Q", step)
    h = hmac.new(key, msg, hashlib.sha1).digest()
    o = h[-1] & 0x0F
    return (struct.unpack(">I", h[o:o + 4])[0] & 0x7FFFFFFF) % 1000000


def wait_next_minute():
    time.sleep(60 - int(time.time()) % 60 + 1)


def login_probe(pw, tries=3):
    last = (0, {})
    for i in range(tries):
        s, r = call("POST", "/auth/login",
                    {"username": UNAME, "password": pw})
        last = (s, r)
        if s != 429:
            break
        time.sleep(5 + i * 5)
    return last


print("== PR0 前置：探针号 ==")
clear_register_rl()
time.sleep(6)
codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 1},
               token=ROOT)[1].get("data") or {}).get("codes")) or []
ok("PR0a 拿到邀请码", bool(codes))
if codes:
    code, CID = codes[0]["code"], codes[0]["id"]
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    s, r = call("POST", "/auth/register", {
        "username": UNAME, "email": "%s@pk.local" % UNAME,
        "password": OLD, "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1])
        if len(nums) >= 2 else 0,
        "captcha_token": "", "fields": {}})
    UID = (r.get("data") or {}).get("user_id")
    ok("PR0b 探针号建好", bool(UID),
       "%s %s" % (s, str(r.get("message"))[:60]))
TOK = None
if UID:
    UID = int(UID)
    s, r = login_probe(OLD)
    TOK = (r.get("data") or {}).get("token")
    ok("PR0c 探针号可凭密码登录", s == 200 and bool(TOK), "http=%s" % s)
ok("PR0d 前置齐备（后续断言才有意义）", bool(TOK))
if not TOK:
    summary()
    sys.exit(0)

P0 = passkey_db()


def rotate(body):
    return call("POST", "/me/passkey/rotate", body, token=TOK)


print("== PR-A 复核被拒时不得轮换 ==")
s, r = rotate({})
ok("PR-A1 空 body 被拒（400/1002 且要求复核密码）",
   s == 400 and r.get("code") == 1002
   and "复核密码" in str(r.get("message") or ""),
   "http=%s code=%s msg=%s" % (s, r.get("code"),
                               str(r.get("message"))[:70]))
ok("PR-A2 被拒后 passkey 未换", passkey_db() == P0,
   "before=%s after=%s" % (P0, passkey_db()))
wait_next_minute()
seen = []
for i in range(12):
    s, r = rotate({"password": "wrong-%d" % i})
    seen.append(s)
    if i == 0:
        first = r
ok("PR-A3 错密码：前 5 次 400、12 次内必现 429",
   seen[:5] == [400] * 5 and 429 in seen, "statuses=%s" % seen)
ok("PR-A4 首拒口径 400/1002「凭证无效」",
   first.get("code") == 1002
   and "凭证无效" in str(first.get("message") or ""),
   "code=%s msg=%s" % (first.get("code"),
                       str(first.get("message"))[:70]))
ok("PR-A5 限流期间 passkey 仍未换", passkey_db() == P0,
   "before=%s after=%s" % (P0, passkey_db()))

print("== PR-B 开了 2FA 必须一并给动态码（正控） ==")
wait_next_minute()
ex("UPDATE users SET totp_secret = '%s', totp_enabled = TRUE "
   "WHERE id = %d" % (SECRET, UID))
s, r = rotate({"password": OLD})
ok("PR-B1 只给密码被拒且提示当前验证码",
   s == 400 and "验证码" in str(r.get("message") or ""),
   "http=%s msg=%s" % (s, str(r.get("message"))[:70]))
s, r = rotate({"password": OLD, "code": totp(SECRET)})
ok("PR-B2 密码+动态码可完成轮换", s == 200
   and bool(dig(r, "data", "passkey")), "http=%s msg=%s"
   % (s, str(r.get("message"))[:60]))
P1 = passkey_db()
ok("PR-B3 库里的 passkey 真的换了", bool(P1) and P1 != P0,
   "before=%s after=%s" % (P0, P1))
ok("PR-B4 返回的新钥与库里一致",
   dig(r, "data", "passkey") == P1,
   "resp=%s db=%s" % (dig(r, "data", "passkey"), P1))
ex("UPDATE users SET totp_secret = NULL, totp_enabled = FALSE "
   "WHERE id = %d" % UID)

print("== PR-S 静态：后端/前端都带上复核 ==")


def be_guarded(src):
    if "fn rotate_passkey" not in src:
        return False
    seg = src.split("fn rotate_passkey")[1].split("#[post")[0]
    return ("step_up" in seg and "rate_window" in seg
            and "body.password" in seg)


be = open(os.path.join(REPO,
                       "apps/api/src/auth_http/me_security.rs"),
          encoding="utf-8").read()
fe = open(os.path.join(REPO, "apps/web/components/passkey-rotate.tsx"),
          encoding="utf-8").read()
ok("PR-S1 后端 rotate 段含 step_up + rate_window + 密码字段",
   be_guarded(be))
OLD_BE = ("async fn rotate_passkey(req: HttpRequest) "
          "-> DomainResult<impl Responder> {\n"
          "    let auth = require_auth(&req, &state).await?;\n}")
ok("PR-S1b 承重自证：旧的无再验证形状必须判红",
   be_guarded(OLD_BE) is False)
ok("PR-S2 前端把 password/code 发到 rotate 口",
   '"/api/v1/me/passkey/rotate"' in fe and "password" in fe
   and "code" in fe)

print("== PR-Z 收尾 ==")
cleanup()
if UID:
    left = psql("SELECT count(*) FROM users WHERE id = %d AND "
                "(status < 2 OR totp_enabled OR totp_secret IS NOT NULL)"
                % UID)
    ok("PR-Z1 探针号已停用且 2FA 归零", left == "0",
       "残留=%s" % left)
    m = psql("SELECT count(*) FROM messages WHERE receiver_id = %d" % UID)
    ok("PR-Z2 站内信已回收", m == "0", "残留=%s" % m)
if CID:
    inv = psql("SELECT count(*) FROM invites WHERE id = %s" % CID)
    ok("PR-Z3 邀请行已回收", inv == "0", "残留=%s" % inv)
summary()
