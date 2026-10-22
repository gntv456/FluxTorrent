# -*- coding: utf-8 -*-
"""闸门 AW：passkey 仪式的断言来源校验（P-566）。

钉住的事：
  WebAuthn L2 要求 RP 校验 `clientDataJSON.origin`。rpId 只把断言圈在
  「同一注册域」——同域下的兄弟子域（用户主页、被接管的 CDN 子域）也能
  让浏览器用同一把 passkey 签出**合法**断言。服务端不看 origin，就等于
  把凭据借给整个注册域：攻击者子域从本站拿 challenge、签完把 body 中继
  回来，登录/绑定都成立。

  注册与登录两侧各打四档：
    正控：本站 origin 照常走完（校验不是把功能关掉）；
    外站 origin 必须 400，且注册不落库、登录不发 token；
    完全不带 origin（伪包/旧客户端）必须 400；
    origin="null"（沙箱 iframe）必须 400。

  判据看后果，不看返回文案：注册读库里那行在不在，登录解响应里的
  token 是谁。旧实现两处 finish 都不读 origin，因此上述三档负控会全绿
  ——那正是缺陷本体。

跑法：
    PYTHONIOENCODING=utf-8 python scripts/pt_audit_aw_passkey_origin.py
需要本地栈（flux-api / flux-postgres / flux-redis）与 python-cryptography。
"""
import base64
import hashlib
import json
import os
import re
import subprocess
import sys
import time

from cryptography.hazmat.primitives import hashes as ch
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives.serialization import (
    Encoding, PublicFormat)

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, dig, login, ok, psql_stdin, summary)

ROOT = login()
TAG = int(time.time()) % 100000
CLEAN = []
MISSING = object()
EVIL = "http://evil.example"


def psql(q):
    return psql_stdin(q)


def ex(sql):
    psql_stdin(sql)


def brief(x):
    return str(x)[:170]


def b64u(b):
    return base64.urlsafe_b64encode(b).decode().rstrip("=")


def authdata(rp, count=1):
    """rpIdHash(32) || flags(0x04=UV) || signCount(4)。"""
    return (hashlib.sha256(rp.encode()).digest() + b"\x04"
            + count.to_bytes(4, "big"))


def new_cred():
    """P-256 密钥 + SPKI DER（浏览器 getPublicKey() 的形态）。"""
    priv = ec.generate_private_key(ec.SECP256R1())
    pub = priv.public_key().public_bytes(
        Encoding.DER, PublicFormat.SubjectPublicKeyInfo)
    return priv, pub


def token_uid(tok):
    try:
        part = tok.split(".")[1]
        part += "=" * (-len(part) % 4)
        return str(json.loads(base64.urlsafe_b64decode(part)).get("sub"))
    except Exception:  # noqa: BLE001
        return "?"


def client_data(typ, chal, origin=MISSING):
    """按浏览器形状拼 clientDataJSON；MISSING 表示刻意不带 origin。"""
    cd = {"type": typ, "challenge": chal}
    if origin is not MISSING:
        cd["origin"] = origin
    return json.dumps(cd)


def reg_attempt(tok, label, cred, pub, rp, origin=MISSING):
    """一次完整注册仪式，返回 (begin 状态, finish 状态, 响应)。"""
    s0, r0 = call("POST", "/auth/passkeys/begin", {
        "label": label, "password": "password123"}, token=tok)
    chal = dig(r0, "data", "challenge")
    if not chal:
        return s0, 0, r0
    cd = client_data("webauthn.create", chal, origin)
    s1, r1 = call("POST", "/auth/passkeys/finish", {
        "cred_id": cred, "public_key": b64u(pub),
        "client_data_json": b64u(cd.encode()),
        "auth_data": b64u(authdata(rp)), "sign_count": 1}, token=tok)
    return s0, s1, r1


def login_attempt(name, priv, cred, rp, origin=MISSING, count=2):
    """一次完整登录仪式（签名始终有效，只改 origin），返回 (状态, 响应)。"""
    _, r0 = call("POST", "/auth/passkey-login/begin", {"username": name})
    chal = dig(r0, "data", "challenge") or ""
    cd = client_data("webauthn.get", chal, origin)
    ad = authdata(rp, count)
    sig = priv.sign(ad + hashlib.sha256(cd.encode()).digest(),
                    ec.ECDSA(ch.SHA256()))
    return call("POST", "/auth/passkey-login/finish", {
        "cred_id": cred, "client_data_json": b64u(cd.encode()),
        "auth_data": b64u(ad), "signature": b64u(sig),
        "sign_count": count})


def clear_rl(pattern):
    """只清本闸自己打出来的限流计数（点名的那族键）。"""
    cmd = ('redis-cli -a "$REDIS_PASSWORD" --no-auth-warning --scan '
           '--pattern "%s" | xargs -r redis-cli -a "$REDIS_PASSWORD" '
           '--no-auth-warning del' % pattern)
    subprocess.run(["docker", "exec", "flux-redis", "sh", "-c", cmd],
                   capture_output=True, text=True, encoding="utf-8")


def container_env():
    r = subprocess.run(
        ["docker", "exec", "flux-api", "sh", "-c",
         "printenv PUBLIC_SITE_URL"],
        capture_output=True, text=True, encoding="utf-8")
    return (r.stdout or "").strip()


def site_origin():
    """站点页面的 origin，与服务端判据同一份配置（去 path/尾斜杠）。"""
    raw = (os.environ.get("PUBLIC_SITE_URL") or container_env()).strip()
    m = re.match(r"^(https?)://([^/?#]+)", raw, re.I)
    if not m:
        return "http://localhost:3000"
    return ("%s://%s" % (m.group(1), m.group(2))).lower()


def mkuser(prefix, pw="password123"):
    """邀请码注册一个探针号（与 AA/AV 同一配方），失败返回三个 None。"""
    clear_register_rl()
    time.sleep(6)
    _, r = call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                token=ROOT)
    codes = dig(r, "data", "codes") or []
    if not codes:
        return None, None, None
    code, cid = codes[0]["code"], codes[0]["id"]
    CLEAN.append("DELETE FROM invites WHERE id = %d" % cid)
    _, cr = call("GET", "/auth/captcha", token=ROOT)
    cap = dig(cr, "data") or {}
    nums = [int(x) for x in re.findall(r"-?\d+", str(cap.get("question")))]
    name = "%s%d" % (prefix, TAG)
    _, rr = call("POST", "/auth/register", {
        "username": name, "email": "%s@aw.local" % name,
        "password": pw, "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": (nums[0] + nums[1]) if len(nums) > 1 else 1,
        "captcha_token": "", "fields": {}})
    uid = dig(rr, "data", "user_id")
    if uid:
        CLEAN.append("DELETE FROM user_passkeys WHERE user_id = %s" % uid)
        CLEAN.append("UPDATE users SET status = 2 WHERE id = %s" % uid)
    tok = None
    for _ in range(10):
        _, lr = call("POST", "/auth/login", {
            "username": name, "password": pw})
        tok = dig(lr, "data", "token")
        if tok:
            break
        time.sleep(2)
    return name, uid, tok


def cleanup():
    for stmt in reversed(CLEAN):
        try:
            ex(stmt)
        except Exception as e:  # noqa: BLE001
            print("   [cleanup] %s -> %s" % (stmt[:60], brief(e)))


print("== S0 前置：探针号 + 站点 origin ==")
ORIGIN = site_origin()
print("   站点 origin=%s（服务端从 PUBLIC_SITE_URL 取同一值）" % ORIGIN)
NAME, UID, TOK = mkuser("aw")
if not (UID and TOK):
    ok("S0 探针号可注册并登录（后续判据的前提）", False,
       "uid=%s tok=%s" % (UID, bool(TOK)))
    cleanup()
    summary()
_, br = call("POST", "/auth/passkey-login/begin", {"username": NAME})
RID = dig(br, "data", "rp_id")
RP = RID or "localhost"
ok("S0 begin 回 rp_id（闸门照它造 authData）", bool(RID), "rp=%s" % RID)

print("== S1 正控：本站 origin 的注册仪式照常走完 ==")
clear_rl("rl:pkreg*")
VPRIV, VSPKI = new_cred()
CRED = "aw-cred-%d" % TAG
s0, s1, r1 = reg_attempt(TOK, "AW%d" % TAG, CRED, VSPKI, RP, ORIGIN)
ok("S1 带本站 origin 的注册 finish=200，且库里落下本人凭证",
   s1 == 200 and psql("SELECT user_id FROM user_passkeys WHERE cred_id = "
                      "'%s'" % CRED) == str(UID),
   "begin=%s finish=%s %s" % (s0, s1, brief(r1)))

print("== S2-S4 负控：注册来源必须钉在本站页面上 ==")
C_EVIL = CRED + "-evil"
_, se, re_ = reg_attempt(TOK, "evil", C_EVIL, VSPKI, RP, EVIL)
ok("S2 外站 origin 的注册必须被拒（否则同域兄弟子域可替用户绑凭据）",
   se == 400 and "来源" in str(re_), "http=%s %s" % (se, brief(re_)))
C_NO = CRED + "-noorg"
_, sn, rn = reg_attempt(TOK, "noorg", C_NO, VSPKI, RP)
ok("S3 不带 origin 的注册必须被拒（浏览器必带；缺了就是伪包）",
   sn == 400 and "origin" in str(rn), "http=%s %s" % (sn, brief(rn)))
C_NULL = CRED + "-null"
_, su, ru = reg_attempt(TOK, "null", C_NULL, VSPKI, RP, "null")
ok("S4 origin=null（沙箱 iframe）的注册必须被拒",
   su == 400 and "来源" in str(ru), "http=%s %s" % (su, brief(ru)))
left = psql("SELECT count(*) FROM user_passkeys WHERE cred_id IN "
            "('%s','%s','%s')" % (C_EVIL, C_NO, C_NULL))
ok("S2-S4 三档负控都没落库（拒了就是没写）", left == "0",
   "left=%s/3" % left)

print("== S5 正控：本站 origin 的登录照常走完 ==")
clear_rl("rl:pklogin*")
s5, r5 = login_attempt(NAME, VPRIV, CRED, RP, ORIGIN, count=2)
T5 = dig(r5, "data", "token") or ""
ok("S5 带本站 origin 登录成功，token 归属本人（uid=%s）" % UID,
   s5 == 200 and token_uid(T5) == str(UID),
   "http=%s 解出=%s %s" % (s5, token_uid(T5), brief(r5)))

print("== S6-S8 负控：登录来源必须钉在本站页面上 ==")
s6, r6 = login_attempt(NAME, VPRIV, CRED, RP, EVIL, count=3)
ok("S6 外站 origin 的登录必须被拒（断言签名有效也不收）",
   s6 == 400 and "来源" in str(r6)
   and not dig(r6, "data", "token"),
   "http=%s token=%s %s" % (s6, bool(dig(r6, "data", "token")),
                            brief(r6)))
s7, r7 = login_attempt(NAME, VPRIV, CRED, RP, count=4)
ok("S7 不带 origin 的登录必须被拒，且不发 token",
   s7 == 400 and "origin" in str(r7)
   and not dig(r7, "data", "token"),
   "http=%s token=%s %s" % (s7, bool(dig(r7, "data", "token")),
                            brief(r7)))
s8, r8 = login_attempt(NAME, VPRIV, CRED, RP, "null", count=5)
ok("S8 origin=null 的登录必须被拒，且不发 token",
   s8 == 400 and "来源" in str(r8)
   and not dig(r8, "data", "token"),
   "http=%s token=%s %s" % (s8, bool(dig(r8, "data", "token")),
                            brief(r8)))

print("== S9 正控：三档负控没把正常会话弄坏 ==")
s9, r9 = login_attempt(NAME, VPRIV, CRED, RP, ORIGIN, count=6)
T9 = dig(r9, "data", "token") or ""
ok("S9 负控之后本站 origin 仍能登录（challenge 会话未被污染）",
   s9 == 200 and token_uid(T9) == str(UID),
   "http=%s 解出=%s" % (s9, token_uid(T9)))

print("== S10 收尾 ==")
call("POST", "/auth/passkeys/remove", {"cred_id": CRED}, token=TOK)
cleanup()
left = psql("SELECT count(*) FROM user_passkeys WHERE user_id = %s" % UID)
ok("S10 探针号名下的凭证已清零（收尾读回）", left == "0",
   "left=%s" % left)
left = psql("SELECT count(*) FROM user_passkeys WHERE cred_id LIKE "
            "'aw-cred-%d%%'" % TAG)
ok("S10 本轮 cred 前缀无残留（正向读回）", left == "0",
   "left=%s" % left)
summary()
