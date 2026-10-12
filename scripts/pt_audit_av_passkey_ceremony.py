# -*- coding: utf-8 -*-
"""闸门 AV：passkey（无密码登录）仪式面（批三十一）。

钉住的事：
  S1 地基：登录态用户能绑定一把自己的 passkey 并**用它登录**——必须真的
     走完 begin/finish 两段并拿到本人 uid 的 token，不看返回值绿就算；
  S3 跨用户凭证覆盖（P0，账号接管）：`POST /auth/passkeys/finish` 的
     `INSERT … ON CONFLICT (cred_id) DO UPDATE SET public_key=$4`
     冲突目标是**全局唯一**的 cred_id（user_passkeys_cred_id_key），
     DO UPDATE 又不带 user_id 条件 ⇒ 任意登录用户把 body 里的 cred_id
     填成别人的凭证 ID（该 ID 由匿名端点 passkey-login/begin 直接回吐），
     就把受害者的公钥换成自己的，随后正常走登录仪式拿到**受害者身份的
     JWT**。注册侧对 client 自报的 (cred_id, public_key, auth_data) 三元组
     不做一致性校验，是根因；
  S2 反控：begin 对不存在的用户名也回 200 空列表（防枚举形状不许退化）；
  S5/S6 既有防线不许退：sign_count 克隆检测、UV 位强制；
  S7 这一族该有的限流：begin/finish 都是匿名端点，对照 /auth/login 的
     5 次/分 + 账号锁定，这里原本一条都没有。

判据取「渲染层」：接管是否成立不看接口返回什么，看**受害者库里那把公钥
有没有被换掉**，以及**攻击者拿到的 token 解出来是谁**。

跑法：
    PYTHONIOENCODING=utf-8 python scripts/pt_audit_av_passkey_ceremony.py
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
    call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
CLEAN = []


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % r.stderr.strip()[:180])
    return r.stdout.strip()


def ex(sql):
    subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-c", sql], capture_output=True, text=True,
        encoding="utf-8")


def brief(x):
    return str(x)[:170]


def b64u(b):
    return base64.urlsafe_b64encode(b).decode().rstrip("=")


def new_cred():
    """一把 P-256 密钥；公钥取 X9.62 非压缩点（65 字节，正是服务端
    p256::EncodedPoint::from_bytes 吃的形状）。"""
    priv = ec.generate_private_key(ec.SECP256R1())
    pub = priv.public_key().public_bytes(
        Encoding.X962, PublicFormat.UncompressedPoint)
    return priv, pub


def authdata(rp, uv=True, count=1):
    """rpIdHash(32) || flags(1) || signCount(4)；UV 位是 0x04。"""
    return (hashlib.sha256(rp.encode()).digest()
            + bytes([0x04 if uv else 0x00]) + count.to_bytes(4, "big"))


def begin_login(username):
    s, r = call("POST", "/auth/passkey-login/begin", {"username": username})
    return s, (r.get("data") or {})


def reg_bind(token, priv, pub, cred_id, rp):
    """走一遍注册仪式（begin→finish）。"""
    s, r = call("POST", "/auth/passkeys/begin", {"label": "AV%d" % TAG},
                token=token)
    chal = (r.get("data") or {}).get("challenge") or ""
    cd = json.dumps({"type": "webauthn.create", "challenge": chal})
    return s, call("POST", "/auth/passkeys/finish", {
        "cred_id": cred_id, "public_key": b64u(pub),
        "client_data_json": b64u(cd.encode()),
        "auth_data": b64u(authdata(rp)), "sign_count": 1}, token=token)


def der_to_raw(der):
    """ASN.1 SEQUENCE { r, s } -> 定宽 r‖s（64 字节）。浏览器不会发这种
    形态，本闸门用它当**负控**：不许有人把验签改回 from_slice 或「两种都收」
    （P-81 的口径就是 CTAP2 规定的 DER）。"""
    def rd(buf, i):
        return buf[i + 2:i + 2 + buf[i + 1]], i + 2 + buf[i + 1]
    seq, _ = rd(der, 0)
    r, i = rd(seq, 0)
    s, _ = rd(seq, i)
    return r.rjust(32, b"\0")[-32:] + s.rjust(32, b"\0")[-32:]


def pk_login(username, priv, cred_id, rp, count=2, uv=True, raw=False):
    """走一遍登录仪式，返回 (begin 状态, begin 数据, finish 状态, 响应)。"""
    s0, d = begin_login(username)
    ad = authdata(rp, uv=uv, count=count)
    cd = json.dumps({"type": "webauthn.get",
                     "challenge": d.get("challenge") or ""})
    msg = ad + hashlib.sha256(cd.encode()).digest()
    sig = priv.sign(msg, ec.ECDSA(ch.SHA256())) if priv else b""
    if sig and raw:
        sig = der_to_raw(sig)
    s1, r1 = call("POST", "/auth/passkey-login/finish", {
        "cred_id": cred_id, "client_data_json": b64u(cd.encode()),
        "auth_data": b64u(ad), "signature": b64u(sig),
        "sign_count": count})
    return s0, d, s1, r1


def token_uid(tok):
    """JWT 中段解出 sub——判「这枚 token 是谁」不看响应字段。"""
    try:
        part = tok.split(".")[1]
        part += "=" * (-len(part) % 4)
        return str(json.loads(base64.urlsafe_b64decode(part)).get("sub"))
    except Exception:  # noqa: BLE001
        return "?"


def pub_of(cred_id):
    return psql("SELECT encode(public_key,'hex') FROM user_passkeys "
                "WHERE cred_id = '%s'" % cred_id)


def mkuser(prefix, pw="password123"):
    """邀请码注册一个探针号（与 AA/AU 同一配方）。"""
    clear_register_rl()
    time.sleep(6)
    codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                   token=ROOT)[1].get("data") or {}).get("codes")) or []
    if not codes:
        return None, None, None
    code, cid = codes[0]["code"], codes[0]["id"]
    CLEAN.append("DELETE FROM invites WHERE id = %d" % cid)
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = [int(x) for x in re.findall(r"-?\d+", str(cap.get("question")))]
    name = "%s%d" % (prefix, TAG)
    s, r = call("POST", "/auth/register", {
        "username": name, "email": "%s@av.local" % name, "password": pw,
        "invite_code": code, "captcha_id": cap.get("captcha_id"),
        "captcha_answer": (nums[0] + nums[1]) if len(nums) > 1 else 1,
        "captcha_token": "", "fields": {}})
    uid = (r.get("data") or {}).get("user_id")
    if uid:
        CLEAN.append("DELETE FROM user_passkeys WHERE user_id = %s" % uid)
        # 探针号一律墓碑（status=2，全站闸门同口径），不物理删：注册即有一
        # 条系统私信，`DELETE FROM users` 会被 messages_receiver_id_fkey
        # 挡下来——挡下不等于删掉，探针号就此攒在库里（本闸门第一版就是这样）。
        CLEAN.append("UPDATE users SET status = 2 WHERE id = %s" % uid)
    tok = None
    for _ in range(10):
        tok = (call("POST", "/auth/login", {"username": name,
                                            "password": pw})[1].get("data")
               or {}).get("token")
        if tok:
            break
        time.sleep(2)
    ok("探针号 %s 可注册并登录（http=%s）" % (name, s), bool(tok and uid),
       brief(r))
    return name, uid, tok


print("== S0 前置：两个探针号（受害者 + 攻击者）==")
VNAME, VID, VTOK = mkuser("avv")
ANAME, AID, ATOK = mkuser("ava")
_, d0 = begin_login(VNAME)
RP = d0.get("rp_id") or "127.0.0.1"
print("   rpId=%s（服务端从 announce_url 推，闸门照它造 authData）" % RP)

print("== S1 地基：绑定自己的 passkey 并用它登录 ==")
vpriv, vpub = new_cred()
VCRED = "av-cred-v-%d" % TAG
s_b, (s_f, r_f) = reg_bind(VTOK, vpriv, vpub, VCRED, RP)
ok("S1 注册仪式可走完（begin %s / finish %s）" % (s_b, s_f), s_f == 200,
   brief(r_f))
ok("S1 正控：库里落下了本人 uid 的凭证",
   psql("SELECT user_id FROM user_passkeys WHERE cred_id = '%s'"
        % VCRED) == str(VID), "cred=%s" % VCRED)
_, dbs, s1, r1 = pk_login(VNAME, vpriv, VCRED, RP, count=2)
T1 = (r1.get("data") or {}).get("token") or ""
ok("S1 用这把 passkey 能登录，且 token 归属是本人（uid=%s）" % VID,
   s1 == 200 and token_uid(T1) == str(VID),
   "http=%s 解出=%s %s" % (s1, token_uid(T1), brief(r1)))

print("== S1b 签名编码口径（P-81 负控）==")
_, _, s1b, r1b = pk_login(VNAME, vpriv, VCRED, RP, count=3, raw=True)
ok("S1b 定宽 r‖s 这种「浏览器不会发」的形态必须被拒（验签口径只有一个："
   "CTAP2 的 ASN.1 DER；两种都收 = 口径没定）", s1b != 200,
   "http=%s %s" % (s1b, brief(r1b)))

print("== S2 begin 的防枚举形状 ==")
ids = dbs.get("cred_ids") or []
ok("S2 地基：begin 按用户名回该账号的 cred_ids（下一步的靶单）",
   VCRED in ids, brief(ids))
s2b, d2b = begin_login("nosuchuser%d" % TAG)
ok("S2 反控：用户名不存在也回 200 空列表（不许退化成 404 报「无此人」）",
   s2b == 200 and (d2b.get("cred_ids") or []) == [],
   "http=%s %s" % (s2b, brief(d2b)))

print("== S3 跨用户凭证覆盖（P0：账号接管）==")
before = pub_of(VCRED)
apriv, apub = new_cred()
_, (s3f, r3f) = reg_bind(ATOK, apriv, apub, VCRED, RP)
ok("S3 攻击者绑定「与别人同 cred_id」的凭证必须被拒（而不是覆盖）",
   s3f >= 400, "http=%s %s" % (s3f, brief(r3f)))
ok("S3 受害者存着的公钥一个字节都没变", before == pub_of(VCRED),
   "before=%s… after=%s…" % (before[:24], pub_of(VCRED)[:24]))
_, _, s3b, r3b = pk_login(VNAME, apriv, VCRED, RP, count=99)
T3 = (r3b.get("data") or {}).get("token") or ""
ok("S3 攻击者用这把钥匙登不进受害者账号（接管不成立）",
   not (s3b == 200 and token_uid(T3) == str(VID)),
   "http=%s 解出 uid=%s（不得是 %s）" % (s3b, token_uid(T3), VID))
ok("S3 这条拒必须来自「凭证不属于该用户」，不是别处炸了"
   "（500/429 都会让上一条假过）",
   s3b in (400, 401) and s3b != 500, "http=%s %s" % (s3b, brief(r3b)))

print("== S4 正控：拒掉越界不等于把正常动线也拒了 ==")
_, _, s4, r4 = pk_login(VNAME, vpriv, VCRED, RP, count=100)
ok("S4 受害者仍能用自己的 passkey 登录", s4 == 200,
   "http=%s %s" % (s4, brief(r4)))
afresh = "av-cred-a-%d" % TAG
_, (s4b, r4b) = reg_bind(ATOK, apriv, apub, afresh, RP)
ok("S4 攻击者绑定自己的新 cred_id 照常成功", s4b == 200,
   "http=%s %s" % (s4b, brief(r4b)))
_, _, s4c, r4c = pk_login(ANAME, apriv, afresh, RP, count=2)
ok("S4 用自己刚绑的凭证登进的是**自己**",
   s4c == 200 and token_uid(
       (r4c.get("data") or {}).get("token") or "") == str(AID),
   "http=%s %s" % (s4c, brief(r4c)))

print("== S5 克隆检测（sign_count 单调）不退 ==")
s5, r5 = pk_login(VNAME, vpriv, VCRED, RP, count=1)[2:]
ok("S5 计数器回退（重放旧值）必须被这条判据本身拒掉，而不是被别的错"
   "顺手挡掉（500/429 都不算过）",
   s5 == 400 and "签名计数异常" in str(r5),
   "http=%s %s" % (s5, brief(r5)))

print("== S6 UV 位强制不退 ==")
_, d6 = begin_login(VNAME)
cd6 = json.dumps({"type": "webauthn.get",
                  "challenge": d6.get("challenge") or ""})
ad6 = authdata(RP, uv=False, count=300)
sig6 = vpriv.sign(ad6 + hashlib.sha256(cd6.encode()).digest(),
                  ec.ECDSA(ch.SHA256()))
s6, r6 = call("POST", "/auth/passkey-login/finish", {
    "cred_id": VCRED, "client_data_json": b64u(cd6.encode()),
    "auth_data": b64u(ad6), "signature": b64u(sig6), "sign_count": 300})
ok("S6 签名有效但未做用户在场验证（UV=0）必须被这条判据拒（同样不许拿"
   "500 当过关）", s6 == 400 and "UV" in str(r6),
   "http=%s %s" % (s6, brief(r6)))

print("== S7 匿名仪式端点的限流（对照 /auth/login 的双档）==")
hits = []
for _ in range(25):
    st, _ = call("POST", "/auth/passkey-login/begin",
                 {"username": VNAME}, raw=True)
    hits.append(st)
ok("S7 begin：连续 25 次必须出现 429（匿名端点无上限 = 枚举 + 算力放大）",
   429 in hits, brief(sorted(set(hits))))
hits2 = []
for _ in range(12):
    st, _ = call("POST", "/auth/passkey-login/finish", {
        "cred_id": "x", "client_data_json": "", "auth_data": "",
        "signature": "", "sign_count": 0}, raw=True)
    hits2.append(st)
ok("S7 finish：连续 12 次必须出现 429（ECDSA 验签是 CPU 活，且这是"
   "认证端点）", 429 in hits2, brief(sorted(set(hits2))))

print("== S8 收尾自检 ==")
call("POST", "/auth/passkeys/remove", {"cred_id": VCRED}, token=VTOK)
call("POST", "/auth/passkeys/remove", {"cred_id": afresh}, token=ATOK)
left = psql("SELECT count(*) FROM user_passkeys WHERE cred_id LIKE "
            "'av-cred-%%-%d'" % TAG)
ok("S8 探针凭证已清（库里不残留）", left == "0", "left=%s" % left)
for stmt in list(reversed(CLEAN)):
    ex(stmt)
summary()
