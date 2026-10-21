# -*- coding: utf-8 -*-
"""闸门 PV：启用 2FA 必须先复核本人密码（P-496，2026-10-21 批二百三十四）。

缺陷：`/me/2fa/setup` 只要会话就签发密钥，`/me/2fa/enable` 只认动态码。
会话被偷的人可以抢先绑上**自己的**验证器并启用：原主此后再用密码登录被
2010 挡住，想关掉又要攻击者的码——账号被锁死，只能找管理员救援。
这与 P-107（绑 passkey 要复核）是同一个威胁模型。

现网旧镜像实测（探针号 7318）：setup 不带密码 http=200 → enable 200 →
库里 totp_enabled=t → 原主密码登录 400/2010 → 攻击者带自己的码登录 200。

修复：setup 先走 twofa_http::step_up（密码必对）并按敏感动线限流；
前端签发按钮旁补「当前密码」输入框。enable 仍只认码——密钥只在复核通过后
才发得出来，攻击者无法据此算出有效码。

P-497（同批）：`enable`/`disable` 对错码没有限流。现网旧镜像实测两支
各连发 12 次错码全是 400/1002、无一次 429；disable 成功后 2FA 被移除，
且每码 ±1 窗口内有效 ⇒ 期望约 33 万次即可试中。修复=两入口各加
5 次/分的按用户固定窗（键 tfaenable/tfadisable）。

夹具：一支探针号（注册→用例→停用清密钥），跑完回读零残留。
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
    call, clear_register_rl, login, ok, summary)

ROOT = login()
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
TAG = int(time.time()) % 100000
UNAME = "pvm%d" % TAG
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
                       ("spark_ledger", "user_id")):
            ex("DELETE FROM %s WHERE %s = %d" % (t, col, UID))
        ex("UPDATE users SET totp_secret = NULL, totp_enabled = FALSE "
           "WHERE id = %d" % UID)
        ex("UPDATE users SET status = 2 WHERE id = %d" % UID)
    if CID:
        ex("DELETE FROM invites WHERE id = %s" % CID)


atexit.register(cleanup)


def totp(secret_b32, step=None):
    key = base64.b32decode(secret_b32)
    s = step if step is not None else int(time.time()) // 30
    h = hmac.new(key, struct.pack(">Q", s), hashlib.sha1).digest()
    o = h[-1] & 0x0F
    return (struct.unpack(">I", h[o:o + 4])[0] & 0x7FFFFFFF) % 1000000


def login_probe(totp_code=None):
    body = {"username": UNAME, "password": "password123"}
    if totp_code is not None:
        body["totp_code"] = totp_code
    return call("POST", "/auth/login", body)


def secret_state():
    return psql("SELECT coalesce(totp_secret,'-')||'|'||totp_enabled "
                "FROM users WHERE id = %d" % UID)


print("== PV0 前置：探针号 ==")
clear_register_rl()
time.sleep(6)
codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 1},
               token=ROOT)[1].get("data") or {}).get("codes")) or []
ok("PV0a 拿到邀请码", bool(codes))
if codes:
    code, CID = codes[0]["code"], codes[0]["id"]
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    s, r = call("POST", "/auth/register", {
        "username": UNAME, "email": "%s@pv.local" % UNAME,
        "password": "password123", "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]) if len(nums) >= 2 else 0,
        "captcha_token": "", "fields": {}})
    UID = (r.get("data") or {}).get("user_id")
    ok("PV0b 探针号建好", bool(UID), "%s %s" % (s, str(r.get("message"))[:60]))
TOK = None
if UID:
    UID = int(UID)
    s, r = login_probe()
    TOK = (r.get("data") or {}).get("token")
    ok("PV0c 探针号未开 2FA，可凭密码登录", s == 200 and bool(TOK),
       "http=%s" % s)
ok("PV0d 前置齐备（后续断言才有意义）", bool(TOK))
if not TOK:
    summary()
    sys.exit(0)

print("== PV-A P-496：setup 必须复核密码 ==")
if TOK:
    s, r = call("POST", "/me/2fa/setup", {}, token=TOK)
    ok("PV-A1 空密码签发密钥被拒（400/1002，提示复核密码）",
       s == 400 and r.get("code") == 1002
       and "复核密码" in str(r.get("message") or ""),
       "http=%s code=%s msg=%s" % (s, r.get("code"),
                                   str(r.get("message"))[:70]))
    ok("PV-A2 被拒后库里没有落任何密钥", secret_state() == "-|false",
       "secret|enabled=%s" % secret_state())
    s, r = call("POST", "/me/2fa/setup", {"password": "wrong-pass-1"},
                token=TOK)
    ok("PV-A3 错误密码同样被拒且不落密钥",
       s == 400 and r.get("code") == 1002 and secret_state() == "-|false",
       "http=%s %s" % (s, str(r.get("message"))[:70]))
    s, r = call("POST", "/me/2fa/setup", {"password": "password123"},
                token=TOK)
    secret = (r.get("data") or {}).get("secret")
    ok("PV-A4 本人密码正确时才签发密钥（正控）",
       s == 200 and bool(secret), "http=%s msg=%s"
       % (s, str(r.get("message"))[:60]))
    if secret:
        # P-497：悬空密钥（已 setup、未启用）也是可爆破的状态写口
        step = int(time.time()) // 30
        valid = {totp(secret, step - 1), totp(secret, step),
                 totp(secret, step + 1)}
        cand = [c for c in range(1, 1000) if c not in valid][:12]
        seen_en = []
        for c in cand:
            s, r = call("POST", "/me/2fa/enable", {"code": c},
                        token=TOK)
            seen_en.append(s)
        ok("PV-A4e 启用错码：前 5 次 400、12 次内必现 429",
           seen_en[:5] == [400] * 5 and 429 in seen_en,
           "statuses=%s" % seen_en)
        st4 = secret_state()
        ok("PV-A4f 限流命中后仍未启用（错码没把锁装上）",
           st4.endswith("|false") and not st4.startswith("-|"),
           "secret|enabled=%s" % st4)
        # 固定窗按分钟分桶：等下一桶再走正确启用
        time.sleep(60 - int(time.time()) % 60 + 1)
        s, r = call("POST", "/me/2fa/enable", {"code": totp(secret)},
                    token=TOK)
        ok("PV-A5 自算动态码可完成启用", s == 200,
           "http=%s msg=%s" % (s, str(r.get("message"))[:60]))
        st6 = secret_state()
        ok("PV-A6 启用后密钥在库、totp_enabled=true",
           st6.endswith("|true"), "secret|enabled=%s" % st6)
        s, r = call("POST", "/me/2fa/setup", {"password": "password123"},
                    token=TOK)
        ok("PV-A7 已启用时 setup 只回「已启用」、不换发密钥",
           s == 400 and "已启用" in str(r.get("message") or ""),
           "http=%s msg=%s" % (s, str(r.get("message"))[:60]))
        # P-497：关闭 2FA 的错码同样必须限流（成功即永久移除第二因子）
        step = int(time.time()) // 30
        valid = {totp(secret, step - 1), totp(secret, step),
                 totp(secret, step + 1)}
        cand = [c for c in range(1, 1000) if c not in valid][:12]
        seen_dis = []
        for c in cand:
            s, r = call("POST", "/me/2fa/disable", {"code": c},
                        token=TOK)
            seen_dis.append(s)
        ok("PV-A7d 关闭错码：前 5 次 400、12 次内必现 429",
           seen_dis[:5] == [400] * 5 and 429 in seen_dis,
           "statuses=%s" % seen_dis)
        st7 = secret_state()
        ok("PV-A7e 限流命中后 2FA 仍启用（错码没能关掉）",
           st7.endswith("|true"), "secret|enabled=%s" % st7)
        s, r = login_probe()
        ok("PV-A8 开了 2FA 后密码登录需要动态码（400/2010）",
           s == 400 and r.get("code") == 2010,
           "http=%s code=%s" % (s, r.get("code")))
        s, r = login_probe(totp(secret))
        ok("PV-A9 带本人动态码可登录（正控）", s == 200
           and bool((r.get("data") or {}).get("token")), "http=%s" % s)
        s, r = call("POST", "/admin/users/%d/2fa/clear" % UID, {},
                    token=ROOT)
        ok("PV-A10 管理员救援口仍可用", s == 200, "http=%s msg=%s"
           % (s, str(r.get("message"))[:60]))
        s, r = login_probe()
        ok("PV-A11 清掉后原主密码登录恢复", s == 200
           and bool((r.get("data") or {}).get("token")), "http=%s" % s)

print("== PV-S 静态：后端/前端都带上复核 ==")


def be_guarded(src):
    if "fn totp_setup" not in src:
        return False
    seg = src.split("fn totp_setup")[1].split("#[derive")[0]
    return ("step_up" in seg and "rate_window" in seg
            and "body.password" in seg)


def fe_password(src):
    if '"/api/v1/me/2fa/setup"' not in src:
        return False
    tail = src.split('"/api/v1/me/2fa/setup"')[1][:160]
    return "password" in tail


be = open(os.path.join(REPO, "apps/api/src/twofa_http.rs"),
          encoding="utf-8").read()
fe = open(os.path.join(REPO, "apps/web/components/twofa-setup.tsx"),
          encoding="utf-8").read()
ok("PV-S1 后端 setup 段含 step_up + rate_window + 密码字段",
   be_guarded(be))
old_be = ("async fn totp_setup(req: HttpRequest, state: Data) "
          "-> DomainResult<HttpResponse> {\n"
          "    let auth = require_auth(&req, &state).await?;\n}")
ok("PV-S1b 承重自证：旧的后端形状（无复核三件）必须判红",
   be_guarded(old_be) is False)
ok("PV-S2 前端签发请求体带 password", fe_password(fe))
old_fe = '"/api/v1/me/2fa/setup",\n        {},'
ok("PV-S2b 承重自证：旧的前端空 body 必须判红",
   fe_password(old_fe) is False)


def be_limited(src, fn, end):
    if ("async fn %s" % fn) not in src:
        return False
    seg = src.split("async fn %s" % fn)[1].split(end)[0]
    return "rate_window" in seg


ok("PV-S3 后端 enable/disable 都按用户限流",
   be_limited(be, "totp_enable", "#[derive(Deserialize)]")
   and be_limited(be, "totp_disable",
                  "pub async fn login_totp_check"))
old_lim = ("async fn totp_disable() {\n"
           "    let auth = require_auth().await?;\n}")
ok("PV-S3b 承重自证：无 rate_window 的旧形状判红",
   be_limited(old_lim, "totp_disable",
              "pub async fn login_totp_check") is False)

print("== PV-Z 收尾 ==")
cleanup()
if UID:
    left = psql("SELECT count(*) FROM users WHERE id = %d AND "
                "(status < 2 OR totp_enabled OR totp_secret IS NOT NULL)"
                % UID)
    ok("PV-Z1 探针号已停用且 2FA 归零", left == "0", "残留=%s" % left)
    m = psql("SELECT count(*) FROM messages WHERE receiver_id = %d" % UID)
    ok("PV-Z2 救援通知等站内信已回收", m == "0", "残留=%s" % m)
if CID:
    inv = psql("SELECT count(*) FROM invites WHERE id = %s" % CID)
    ok("PV-Z3 邀请行已回收", inv == "0", "残留=%s" % inv)
summary()
