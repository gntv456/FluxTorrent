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

  S8 注册仪式要 step-up（P-107）：begin/finish 原本只要 `require_auth`，
     拿到一次会话就能自绑 passkey；而 passkey 登录不查 TOTP ⇒ 给已开 2FA 的
     成员绑一把之后，**用 passkey 登录完全绕过 2FA**。本闸先真开 2FA（闸门
     自己按 RFC 6238 算码，顺带证明两边口径一致），再只用会话打 begin——
     红线是它直接发 challenge，并把整条绕过链的后果打在日志里。

  S9 rp_id 的口径（P-114）：relying party 是**站点本身**，旧实现第一优先级
     取 announce_url 的 host——PT 站的 announce 常在另一台主机甚至裸 IP，
     而浏览器要求 rpId 与页面可注册域一致 ⇒ 这类部署下每次注册都当场
     SecurityError，passkey 整条不可用（面板上就是这句英文报错）。

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
    call, clear_register_rl, login, ok, summary, totp_now)

ROOT = login()
TAG = int(time.time()) % 100000
CLEAN = []


def clear_rl(pattern):
    """清掉本闸自己打出来的限流计数（与 clear_register_rl 同一口径：只碰
    点名的那族键，站点限流机制本身不动）。S8 的档判据要一个干净的窗口才能
    说「第 n 次才被拦」，否则前面各节正常的 begin 会把额度吃光，把判据打成
    429 假红。"""
    cmd = ('redis-cli -a "$REDIS_PASSWORD" --no-auth-warning --scan '
           '--pattern "%s" | xargs -r redis-cli -a "$REDIS_PASSWORD" '
           '--no-auth-warning del' % pattern)
    subprocess.run(["docker", "exec", "flux-redis", "sh", "-c", cmd],
                   capture_output=True, text=True, encoding="utf-8")


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
    """一把 P-256 密钥，同时给出两种公钥编码：
    - SPKI DER：浏览器 `getPublicKey()` 交回来的形态（本闸门主用，
      因为「闸门按真实客户端形状跑」才有意义 —— P-82 就是被服务端
      只认 SEC1 卡死的）；
    - SEC1 未压缩点（0x04‖X‖Y）：库里实际存的形状，非浏览器客户端会交这个。"""
    priv = ec.generate_private_key(ec.SECP256R1())
    pub = priv.public_key()
    sec1 = pub.public_bytes(Encoding.X962, PublicFormat.UncompressedPoint)
    spki = pub.public_bytes(Encoding.DER, PublicFormat.SubjectPublicKeyInfo)
    return priv, sec1, spki


def cose_of(sec1):
    """手工 CBOR 编出 COSE_Key {1:2, 3:-7, -1:1, -2:x, -3:y}——即
    authenticatorData 里那个结构。服务端不该拿它当公钥收（P-82 负控）。"""
    x, y = sec1[1:33], sec1[33:65]
    return (b"\xa5" + b"\x01\x02" + b"\x03\x26" + b"\x20\x01"
            + b"\x21\x58\x20" + x + b"\x22\x58\x20" + y)


def authdata(rp, uv=True, count=1):
    """rpIdHash(32) || flags(1) || signCount(4)；UV 位是 0x04。"""
    return (hashlib.sha256(rp.encode()).digest()
            + bytes([0x04 if uv else 0x00]) + count.to_bytes(4, "big"))


def begin_login(username):
    s, r = call("POST", "/auth/passkey-login/begin", {"username": username})
    return s, (r.get("data") or {})


def reg_bind(token, priv, pub, cred_id, rp, step_up=None):
    """走一遍注册仪式（begin→finish）。

    P-107 之后 begin 要复核凭证：探针号口令都是 password123，默认就带上
    （旧镜像上这个字段被忽略，两态都能走完正常动线）。`step_up` 用来覆盖
    （如密码错的负控）。
    """
    body = {"label": "AV%d" % TAG, "password": "password123"}
    if step_up is not None:
        body = {"label": "AV%d" % TAG, **step_up}
    s, r = call("POST", "/auth/passkeys/begin", body, token=token)
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
vpriv, vsec1, vspki = new_cred()
VCRED = "av-cred-v-%d" % TAG
s_b, (s_f, r_f) = reg_bind(VTOK, vpriv, vspki, VCRED, RP)
ok("S1 注册仪式可走完（begin %s / finish %s）——公钥按浏览器形态（SPKI "
   "DER）提交" % (s_b, s_f), s_f == 200, brief(r_f))
ok("S1 正控：库里落下了本人 uid 的凭证",
   psql("SELECT user_id FROM user_passkeys WHERE cred_id = '%s'"
        % VCRED) == str(VID), "cred=%s" % VCRED)
# 设备名以 **begin 的 label** 落库（finish 的 body 里根本没有 label 字段）——
# 这条钉住契约，web 侧「先问名字再 begin」才是有依据的写法（P-112）。
ok("S1 正控：落库的 label 就是 begin 给的那个名字（finish 不接收 label）",
   psql("SELECT label FROM user_passkeys WHERE cred_id = '%s'"
        % VCRED) == "AV%d" % TAG,
   "label=%s" % psql("SELECT label FROM user_passkeys WHERE cred_id = '%s'"
                     % VCRED))
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

print("== S1c 公钥编码口径（P-82）==")
COSE_CRED = "av-cose-%d" % TAG
_, (sc, rc) = reg_bind(VTOK, vpriv, cose_of(vsec1), COSE_CRED, RP)
ok("S1c COSE_Key（authenticatorData 里那个 CBOR 结构）必须明确拒——"
   "不许当 SEC1 蒙混收下，也不许 500", sc >= 400 and sc < 500
   and "COSE" in str(rc), "http=%s %s" % (sc, brief(rc)))
SEC1_CRED = VCRED + "-sec1"
_, (sc2, rc2) = reg_bind(VTOK, vpriv, vsec1, SEC1_CRED, RP)
ok("S1c 正控：SEC1 未压缩点照收（库里实际存的形状，非浏览器客户端交这个）",
   sc2 == 200, brief(rc2))
_, _, sc3, rc3 = pk_login(VNAME, vpriv, SEC1_CRED, RP, count=4)
ok("S1c 用 SPKI 绑进来的那把能登录之外，SEC1 绑的这把也能（归一只在入口，"
   "没把两条路都改坏）", sc3 == 200, "http=%s %s" % (sc3, brief(rc3)))

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
apriv, asec1, aspki = new_cred()
_, (s3f, r3f) = reg_bind(ATOK, apriv, aspki, VCRED, RP)
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
_, (s4b, r4b) = reg_bind(ATOK, apriv, aspki, afresh, RP)
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

print("== S8 注册仪式要 step-up（P-107：会话被偷不得白送一把永久后门）==")
# 先给「受害者」真开 2FA：闸门自己按 RFC 6238 算码，口径与服务端 totp_at 同
s, r = call("POST", "/me/2fa/setup",
            {"password": "password123"}, token=VTOK)
SEC_B32 = ((r.get("data") or {}).get("secret") or "").strip()
ok("S8 前置：探针可生成 2FA 密钥（拿得到 base32）", bool(SEC_B32), brief(r))
s, r = call("POST", "/me/2fa/enable", {"code": totp_now(SEC_B32)}, token=VTOK)
ok("S8 前置：探针已开 2FA（闸门自算的码被服务端接受 = 两边口径一致）",
   s == 200, "%s %s" % (s, brief(r)))
CLEAN.append("UPDATE users SET totp_secret = NULL, totp_enabled = FALSE "
             "WHERE id = %s" % VID)
# begin 的档要干净窗口：前面各节的正常 begin 也在同一个桶里计过数
clear_rl("rl:pkregbegin*")

# 攻击者手里只有偷来的一次会话：没有密码，没有验证码
STOLEN = "av-steal-%d" % TAG
s_steal, r_steal = call("POST", "/auth/passkeys/begin", {"label": "stolen"},
                        token=VTOK)
CH_STEAL = (r_steal.get("data") or {}).get("challenge") or ""
ok("S8 仅凭会话（不给密码/码）begin 必须被拒（红线：200 直接发 challenge"
   " ⇒ 攻击者自绑一把 passkey，此后 passkey 登录完全不问 2FA）",
   s_steal != 200 and not CH_STEAL, "%s %s" % (s_steal, brief(r_steal)))
if CH_STEAL:
    # 红线要把后果打在日志上（修好后不再进入这一支）。S7 那节会连打匿名端点
    # 把 pklogin 的档吃光，所以这里先清掉本闸自己造的计数，好让「绑上之后真能
    # 用它登录」这一步不被自己的前一轮拖成 429 假象。
    clear_rl("rl:pklogin*")
    sk, _, pk = new_cred()
    cd = json.dumps({"type": "webauthn.create", "challenge": CH_STEAL})
    s_ff, r_ff = call("POST", "/auth/passkeys/finish", {
        "cred_id": STOLEN, "public_key": b64u(pk),
        "client_data_json": b64u(cd.encode()),
        "auth_data": b64u(authdata(RP)), "sign_count": 1}, token=VTOK)
    _, _, s_lg, r_lg = pk_login(VNAME, sk, STOLEN, RP, count=2)
    print("   [红线后果] finish=%s，passkey 登录=%s，token 解出 uid=%s"
          "——该号已开 2FA，而攻击者自始至终没提供任何 2FA 凭证"
          % (s_ff, s_lg, token_uid((r_lg.get("data") or {}).get("token")
                                   or "")))
s_pw, r_pw = call("POST", "/auth/passkeys/begin",
                  {"label": "x", "password": "Wrong#2026pw"}, token=VTOK)
ok("S8 密码错必须拒（400 + 点到凭证），不许静默放行",
   s_pw == 400 and "凭证" in str(r_pw.get("message", "")),
   "%s %s" % (s_pw, brief(r_pw)))
s_np, r_np = call("POST", "/auth/passkeys/begin",
                  {"label": "x", "password": "password123"}, token=VTOK)
ok("S8 开了 2FA 却只复核密码，仍必须拒（红线：200——2FA 在这一口被跳过）",
   s_np == 400 and "验证码" in str(r_np.get("message", "")),
   "%s %s" % (s_np, brief(r_np)))
sk2, _, pk2 = new_cred()
s_okb, r_okb = call("POST", "/auth/passkeys/begin", {
    "label": "legit", "password": "password123",
    "totp": totp_now(SEC_B32)}, token=VTOK)
ok("S8 正控：密码 + 当前验证码 = 正常动线照走（step-up 不是把功能关掉）",
   s_okb == 200 and (r_okb.get("data") or {}).get("challenge"),
   "%s %s" % (s_okb, brief(r_okb)))
s_fin, r_fin = call("POST", "/auth/passkeys/finish", {
    "cred_id": "av-legit-%d" % TAG, "public_key": b64u(pk2),
    "client_data_json": b64u(json.dumps({
        "type": "webauthn.create",
        "challenge": (r_okb.get("data") or {}).get("challenge") or ""
    }).encode()),
    "auth_data": b64u(authdata(RP)), "sign_count": 1}, token=VTOK)
got = psql("SELECT user_id FROM user_passkeys WHERE cred_id = "
           "'av-legit-%d'" % TAG)
ok("S8 正控：合法 step-up 后 finish 成功且库里落本人凭证（uid=%s）" % VID,
   s_fin == 200 and got == str(VID), "%s %s 库里=%s"
   % (s_fin, brief(r_fin), got))
codes = []
for _ in range(6):
    st, _ = call("POST", "/auth/passkeys/begin",
                 {"label": "x", "password": "Wrong#2026pw"}, token=VTOK,
                 raw=True)
    codes.append(st)
ok("S8 begin 也要有档（连发 6 次错密码必须出现 429；红线：全 200，"
   "无上限 = 在线爆破密码的口）", 429 in codes,
   "状态序列=%s" % codes)
# 复核只挡在 begin，那么 finish 必须真的离不开 challenge——否则攻击者可以
# 自己编一个 challenge 直接收尾，step-up 就白加了。
NOKEY = "av-nocred-%d" % TAG
s_no, r_no = call("POST", "/auth/passkeys/finish", {
    "cred_id": NOKEY, "public_key": b64u(new_cred()[2]),
    "client_data_json": b64u(json.dumps(
        {"type": "webauthn.create", "challenge": "forged-by-attacker"}
    ).encode()),
    "auth_data": b64u(authdata(RP)), "sign_count": 1}, token=VTOK)
ok("S8 跳过 begin 直接 finish 必须被拒，且库里不落凭证（challenge 是唯一"
   "入口：begin 不放行这条链就走不完）",
   s_no == 400 and psql("SELECT count(*) FROM user_passkeys WHERE "
   "cred_id = '%s'" % NOKEY) == "0", "%s %s" % (s_no, brief(r_no)))

print("== S9 rp_id 的口径（P-114：relying party 是站点本身，不是 tracker）==")


def site_host():
    """读 api 容器自己的 PUBLIC_SITE_URL（与产品同一份配置，不写死）。"""
    out = subprocess.run(
        ["docker", "exec", "flux-api", "sh", "-c", "printenv PUBLIC_SITE_URL"],
        capture_output=True, text=True, encoding="utf-8").stdout.strip()
    if not out:
        return None
    return re.split(r"[:/?]", out.split("://")[-1])[0]


SH = site_host()
ANN = psql("SELECT value FROM site_settings WHERE name = 'announce_url'")
clear_rl("rl:pkregbegin*")  # S8 把这一档吃满了，这里要的是干净的 begin
s_b9, r_b9 = call("POST", "/auth/passkeys/begin", {
    "label": "rp", "password": "password123",
    "totp": totp_now(SEC_B32)}, token=VTOK)
RID = (r_b9.get("data") or {}).get("rp_id")
ok("S9 begin 回里的 rp_id 必须是**站点对外地址**的 host（红线：它取的是 "
   "announce_url 的 host ⇒ rpId 与页面域不一致，浏览器对每一次注册当场 "
   "SecurityError，passkey 在「announce 主机 ≠ 站点主机」的部署里整条不可用）",
   SH is not None and RID == SH,
   "rp_id=%s 站点 host=%s announce=%s" % (RID, SH, ANN))
# S7 那节把 pklogin 的桶连打 25 次吃满了——这里要的是「同源」这件事本身，
# 不是自己被限流打出的空 data（先清自己造的计数，别把探针的账记到站点头上）
clear_rl("rl:pklogin*")
_, d9 = begin_login(VNAME)
ok("S9 登录侧的 rp_id 与注册侧同源（两处一旦漂移，已绑凭证集体验不过）",
   d9.get("rp_id") == RID, "%s vs %s" % (d9.get("rp_id"), RID))

print("== S10 收尾自检 ==")
ok("S10 COSE 那把没在库里留下任何行（拒了就是没写）",
   psql("SELECT count(*) FROM user_passkeys WHERE cred_id = '%s'"
        % COSE_CRED) == "0", "left=%s" % psql(
            "SELECT count(*) FROM user_passkeys WHERE cred_id = '%s'"
            % COSE_CRED))
call("POST", "/auth/passkeys/remove", {"cred_id": VCRED}, token=VTOK)
call("POST", "/auth/passkeys/remove", {"cred_id": SEC1_CRED}, token=VTOK)
call("POST", "/auth/passkeys/remove", {"cred_id": afresh}, token=ATOK)
left = psql("SELECT count(*) FROM user_passkeys WHERE cred_id LIKE "
            "'av-cred-%%-%d'" % TAG)
ok("S10 探针凭证已清（库里不残留）", left == "0", "left=%s" % left)
for stmt in list(reversed(CLEAN)):
    ex(stmt)
summary()
