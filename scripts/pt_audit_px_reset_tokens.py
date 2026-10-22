# -*- coding: utf-8 -*-
"""闸门 PX：找回密码链接的一次性与改密联动（P-563，2026-10-22）。

现网实证（部署镜像，探针号 7844）：同一账号连点找回会留下多枚可用的
重置令牌；用掉其中一枚后，另一枚仍能再次改密（200，且新密码可登录）；
自助改密（/me/password/change）也不动此前寄出的链接——攻击者手里的
待用链接能把刚换掉的密码再改回去。

修复：找回口按用户只留一枚可用（与激活信 email_verify.rs:74 同口径）；
重置成功时整账号收口（rows_affected=0 即并发重放，整笔回滚）；改密口
同样作废待用链接。

本闸门不依赖 SMTP：令牌行由本闸直接写入（明文哈希可控），走真 API。

  PX0 探针号可建可登
  PX1 两枚并存 → 用掉 T1 后 T2 必须 400，且密码哈希不变
  PX2 改密前寄出的 T3，改密后必须 400，且密码哈希不变
  PX3 源码：找回先作废旧枚；重置按 user_id 收口 + rows_affected 判空
  PX4 DB 事务 A/B：旧语句留 1 枚活的，新语句 0 枚；重放 UPDATE 0
"""
import atexit
import hashlib
import os
import re
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
TAG = int(time.time()) % 100000
UNAME = "px%d" % TAG
P0 = "pw-old-563"
UID = None
CID = None
TOK = None


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


def sqlinput(sql):
    """多语句必须走 -f -（-c 只回最后一条的输出）。"""
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-f", "-"],
        input=sql, capture_output=True, text=True, encoding="utf-8")
    return r.stdout + r.stderr


def h(t):
    return hashlib.sha3_256(t.encode()).hexdigest()


def mint(tok):
    ex("INSERT INTO password_resets (user_id, token_hash, expires_at) "
       "VALUES (%d, '%s', now() + interval '30 minutes')" % (UID, h(tok)))


def live():
    return int(psql("SELECT count(*) FROM password_resets WHERE user_id = %d "
                    "AND used_at IS NULL AND expires_at > now()" % UID))


def md5pw():
    return psql("SELECT md5(pass_hash) FROM users WHERE id = %d" % UID)


def login_probe(pw, tries=3):
    last = (0, {})
    for i in range(tries):
        s, r = call("POST", "/auth/login", {"username": UNAME,
                                            "password": pw})
        last = (s, r)
        if s != 429:
            break
        time.sleep(5 + i * 5)
    return last


def cleanup():
    if UID:
        for t, col in (("messages", "receiver_id"),
                       ("password_resets", "user_id"),
                       ("token_revocations", "user_id"),
                       ("login_events", "user_id"),
                       ("spark_ledger", "user_id")):
            ex("DELETE FROM %s WHERE %s = %d" % (t, col, UID))
        ex("UPDATE users SET status = 2, totp_enabled = FALSE, "
           "totp_secret = NULL WHERE id = %d" % UID)
    if CID:
        ex("DELETE FROM invites WHERE id = %s" % CID)
    if UID:
        print("   [cleanup] 令牌残留=%s 探针状态=%s 邀请=%s"
              % (psql("SELECT count(*) FROM password_resets "
                      "WHERE user_id = %d" % UID),
                 psql("SELECT status FROM users WHERE id = %d" % UID),
                 "已删" if CID else "-"))


atexit.register(cleanup)


def reg_probe():
    global UID, CID
    clear_register_rl()
    time.sleep(6)
    codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                   token=ROOT)[1].get("data") or {}).get("codes")) or []
    if not codes:
        return False
    code, CID = codes[0]["code"], codes[0]["id"]
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    s, r = call("POST", "/auth/register", {
        "username": UNAME, "email": "%s@px.local" % UNAME,
        "password": P0, "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": (int(nums[0]) + int(nums[1])
                           if len(nums) >= 2 else 0),
        "captcha_token": "", "fields": {}})
    uid = (r.get("data") or {}).get("user_id")
    if uid:
        UID = int(uid)
    print("   注册 http=%s uid=%s" % (s, UID))
    return bool(UID)


print("== PX0 探针号 ==")
ok("PX0 探针号可建成", reg_probe())
if not UID:
    summary()
    sys.exit(1)
s, r = login_probe(P0)
TOK = (r.get("data") or {}).get("token")
ok("PX0b 探针号可登（后续判据才有意义）", s == 200 and bool(TOK),
   "http=%s" % s)

print("== PX1 用掉一枚后，另一枚必须失效 ==")
T1, T2 = "px-a-%d" % TAG, "px-b-%d" % TAG
mint(T1)
mint(T2)
ok("PX1a 夹具：两枚未用令牌并存（复刻现网旧行为）", live() == 2,
   "live=%d" % live())
s, r = call("POST", "/auth/password/reset",
            {"token": T1, "new_password": "pw-new1-563"})
ok("PX1b 用 T1 重置成功", s == 200 and r.get("code") == 0,
   "http=%s %s" % (s, str(r.get("message"))[:60]))
h0 = md5pw()
s, r = call("POST", "/auth/password/reset",
            {"token": T2, "new_password": "pw-new2-563"})
ok("PX1c 另一枚旧链接必须被拒（红线：旧版 200，还能再改一次密码）",
   s == 400 and "无效或已过期" in str(r.get("message")),
   "http=%s %s" % (s, str(r.get("message"))[:60]))
ok("PX1d 被拒的旧链接没改到密码（哈希未变）", md5pw() == h0)

print("== PX2 改密后，改密前寄出的链接必须失效 ==")
# 旧镜像上 T2 会真的生效（PX1c 红）——按哈希选当前密码，避免级联假红
pw_now = "pw-new2-563" if md5pw() != h0 else "pw-new1-563"
time.sleep(1.2)     # 重置的撤销线 nbf=now，同秒登录会被判失效
s, r = login_probe(pw_now)
TOK = (r.get("data") or {}).get("token")
ok("PX2a 重置后的新密码可登录（PX1 的正控）", s == 200 and bool(TOK),
   "http=%s" % s)
T3 = "px-c-%d" % TAG
mint(T3)
ok("PX2b 夹具：改密前确有一枚待用链接", live() == 1, "live=%d" % live())
s, r = call("POST", "/me/password/change",
            {"old_password": pw_now, "new_password": "pw-new3-563"},
            token=TOK)
ok("PX2c 自助改密成功", s == 200, "http=%s %s"
   % (s, str(r.get("message"))[:60]))
h1 = md5pw()
s, r = call("POST", "/auth/password/reset",
            {"token": T3, "new_password": "pw-new4-563"})
ok("PX2d 改密后旧链接必须被拒（红线：旧版 200，夺号者能改回去）",
   s == 400 and "无效或已过期" in str(r.get("message")),
   "http=%s %s" % (s, str(r.get("message"))[:60]))
ok("PX2e 被拒的链接没改到密码（哈希未变）", md5pw() == h1)

print("== PX3 源码：两处出口的收口语义 ==")
RESET = os.path.join(REPO, "apps", "api", "src", "gaps_http", "reset.rs")
MESEC = os.path.join(REPO, "apps", "api", "src", "auth_http",
                     "me_security.rs")
src = open(RESET, encoding="utf-8", errors="replace").read()


def section(text, start, end):
    i = text.find(start)
    j = text.find(end, i + len(start)) if i >= 0 else -1
    return text[i:j] if i >= 0 and j > i else ""


# 各块缩进不同 ⇒ 续行空白不写死，按正则归一
INV_RE = re.compile(
    r"UPDATE password_resets SET used_at = now\(\)"
    r"[\\\s]+WHERE user_id = \$1 AND used_at IS NULL")


def forgot_ok(body):
    m = INV_RE.search(body)
    j = body.find("INSERT INTO password_resets")
    return bool(m) and j > m.start()


def reset_ok(body):
    return (bool(INV_RE.search(body)) and "rows_affected()" in body
            and "used == 0" in body
            and "SET used_at = now() WHERE token_hash" not in body)


fbody = section(src, "pub async fn password_forgot",
                '#[post("/auth/password/reset")]')
rbody = section(src, "pub async fn password_reset", "struct ForgotReq")
ok("PX3a 找回口先作废旧枚再铸新枚", forgot_ok(fbody))
ok("PX3b 重置口按 user_id 收口并按影响行数判重放", reset_ok(rbody))
mesrc = open(MESEC, encoding="utf-8", errors="replace").read()
mbody = section(mesrc, "pub async fn me_password_change",
                "struct PasswordChangeReq")
ok("PX3c 改密口也作废待用链接", bool(INV_RE.search(mbody)))
old_forgot = ('let token = format!("pwd_{}", uuid::Uuid::new_v4()'
              '.simple());\nINSERT INTO password_resets')
old_reset = ('"UPDATE password_resets SET used_at = now() '
             'WHERE token_hash = $1"')
ok("PX3d 承重自证：旧形状必须判红",
   forgot_ok(old_forgot) is False and reset_ok(old_reset) is False)

print("== PX4 DB 事务 A/B：语义承重 ==")
out = sqlinput(
    "BEGIN;\n"
    "INSERT INTO password_resets (user_id, token_hash, expires_at)\n"
    "VALUES (%d, 'px-ab-1', now() + interval '30 minutes'),\n"
    "       (%d, 'px-ab-2', now() + interval '30 minutes');\n"
    "UPDATE password_resets SET used_at = now()\n"
    " WHERE token_hash = 'px-ab-1';\n"
    "SELECT 'old_live=' || count(*) FROM password_resets\n"
    " WHERE user_id = %d AND used_at IS NULL AND expires_at > now();\n"
    "UPDATE password_resets SET used_at = now()\n"
    " WHERE user_id = %d AND used_at IS NULL;\n"
    "SELECT 'new_live=' || count(*) FROM password_resets\n"
    " WHERE user_id = %d AND used_at IS NULL AND expires_at > now();\n"
    "UPDATE password_resets SET used_at = now()\n"
    " WHERE user_id = %d AND used_at IS NULL;\n"
    "ROLLBACK;\n" % (UID, UID, UID, UID, UID, UID))
ok("PX4a 旧语句用掉一枚后仍留 1 枚活的（缺陷本体）",
   "old_live=1" in out, out.strip().replace("\n", " | ")[:160])
ok("PX4b 新语句后 0 枚活的", "new_live=0" in out)
ok("PX4c 重放同一收口语句影响 0 行（并发拒单的判据）",
   "UPDATE 0" in out, out.strip().replace("\n", " | ")[:160])

print("== PX-Z 收尾 ==")
cleanup()
left = psql("SELECT count(*) FROM password_resets WHERE user_id = %d" % UID)
ok("PX-Z1 令牌行已清零", left == "0", "残留=%s" % left)
st = psql("SELECT status FROM users WHERE id = %d" % UID)
ok("PX-Z2 探针号已停用", st == "2", "status=%s" % st)
if CID:
    inv = psql("SELECT count(*) FROM invites WHERE id = %s" % CID)
    ok("PX-Z3 邀请行已回收", inv == "0", "残留=%s" % inv)
summary()
