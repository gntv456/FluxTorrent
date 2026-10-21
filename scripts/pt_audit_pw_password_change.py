# -*- coding: utf-8 -*-
"""闸门 PW：改密口的旧密码校验必须限流（P-498，2026-10-21 批二百三十五）。

缺陷：`/me/password/change` 验旧密码，但不限次。登录侧有 acctlock
（失败计数 + PoW），这一口是登录之外的第二个密码 oracle：会话被偷的人
可以无限猜旧密码，猜中后顺手改密——改密会撤销全部旧 token，原主被踢下
线且无法再用旧密码登录。现网旧镜像实测（探针号 7324）：12 次错旧密码
全是 400/1002「旧密码不正确」，无一次 429，原密码仍可登录。

修复：该口按用户加 5 次/分的固定窗（键 pwchange:{uid}），与登录同档。
本闸门同时验证限流不误伤正常改密：换窗后正确旧密码仍能改密并登录。

夹具：一支探针号（注册→错码→改密→停用），跑完回读零残留。
"""
import atexit
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
TAG = int(time.time()) % 100000
UNAME = "pwm%d" % TAG
OLD = "password123"
NEW = "newpass-987-abc"
UID = None
CID = None


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError(r.stderr.strip()[:200])
    return r.stdout.strip()


def ex(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-c", q],
        capture_output=True, text=True, encoding="utf-8")
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


def pass_hash():
    # 只取前缀会假绿：Argon2id 的 PHC 串前 16 位（$argon2id$v=19$m）
    # 对所有哈希都相同，改密后也比不出差异。取全串的 md5。
    return psql("SELECT md5(pass_hash) FROM users WHERE id = %d" % UID)


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


print("== PW0 前置：探针号 ==")
clear_register_rl()
time.sleep(6)
codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 1},
               token=ROOT)[1].get("data") or {}).get("codes")) or []
ok("PW0a 拿到邀请码", bool(codes))
if codes:
    code, CID = codes[0]["code"], codes[0]["id"]
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    s, r = call("POST", "/auth/register", {
        "username": UNAME, "email": "%s@pw.local" % UNAME,
        "password": OLD, "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]) if len(nums) >= 2 else 0,
        "captcha_token": "", "fields": {}})
    UID = (r.get("data") or {}).get("user_id")
    ok("PW0b 探针号建好", bool(UID), "%s %s" % (s, str(r.get("message"))[:60]))
TOK = None
if UID:
    UID = int(UID)
    s, r = login_probe(OLD)
    TOK = (r.get("data") or {}).get("token")
    ok("PW0c 探针号可凭密码登录", s == 200 and bool(TOK), "http=%s" % s)
ok("PW0d 前置齐备（后续断言才有意义）", bool(TOK))
if not TOK:
    summary()
    sys.exit(0)

print("== PW-A P-498：错旧密码必须限流 ==")
h0 = pass_hash()
seen = []
for i in range(12):
    s, r = call("POST", "/me/password/change",
                {"old_password": "wrong-old-%d" % i,
                 "new_password": NEW}, token=TOK)
    seen.append(s)
    if i == 0:
        first = r
ok("PW-A1 错旧密码：前 5 次 400、12 次内必现 429",
   seen[:5] == [400] * 5 and 429 in seen, "statuses=%s" % seen)
ok("PW-A2 首拒口径为 400/1002 且明确「旧密码不正确」",
   first.get("code") == 1002
   and "旧密码不正确" in str(first.get("message") or ""),
   "code=%s msg=%s" % (first.get("code"),
                       str(first.get("message"))[:60]))
s, r = login_probe(OLD)
ok("PW-A3 限流命中后原密码仍可登录（没被猜到）", s == 200
   and bool((r.get("data") or {}).get("token")), "http=%s" % s)
ok("PW-A4 限流命中后密码哈希未变", pass_hash() == h0,
   "before=%s after=%s" % (h0, pass_hash()))

print("== PW-B 换窗后正确改密仍可用（限流不误伤）==")
# 固定窗按分钟分桶：等下一桶再走正确改密
time.sleep(60 - int(time.time()) % 60 + 1)
s, r = call("POST", "/me/password/change",
            {"old_password": OLD, "new_password": NEW}, token=TOK)
ok("PW-B1 正确旧密码可完成改密", s == 200, "http=%s msg=%s"
   % (s, str(r.get("message"))[:60]))
ok("PW-B2 改密后哈希已更换", pass_hash() != h0,
   "before=%s after=%s" % (h0, pass_hash()))
s, r = login_probe(NEW)
ok("PW-B3 新密码可登录（正控）", s == 200
   and bool((r.get("data") or {}).get("token")), "http=%s" % s)

print("== PW-S 静态：改密口带 rate_window ==")


def be_guarded(src):
    if "async fn me_password_change" not in src:
        return False
    seg = src.split("async fn me_password_change")[1].split("#[derive")[0]
    return "rate_window" in seg


me = open(os.path.join(REPO, "apps/api/src/auth_http/me_security.rs"),
          encoding="utf-8").read()
ok("PW-S1 改密函数段含 rate_window", be_guarded(me))
old_shape = ("async fn me_password_change(req: HttpRequest) "
             "-> DomainResult<impl Responder> {\n"
             "    let auth = require_auth(&req, &state).await?;\n}")
ok("PW-S1b 承重自证：旧的无限制形状必须判红",
   be_guarded(old_shape) is False)

print("== PW-Z 收尾 ==")
cleanup()
if UID:
    left = psql("SELECT count(*) FROM users WHERE id = %d AND "
                "(status < 2 OR totp_enabled OR totp_secret IS NOT NULL)"
                % UID)
    ok("PW-Z1 探针号已停用且 2FA 归零", left == "0", "残留=%s" % left)
    m = psql("SELECT count(*) FROM messages WHERE receiver_id = %d" % UID)
    ok("PW-Z2 站内信已回收", m == "0", "残留=%s" % m)
if CID:
    inv = psql("SELECT count(*) FROM invites WHERE id = %s" % CID)
    ok("PW-Z3 邀请行已回收", inv == "0", "残留=%s" % inv)
summary()
