# -*- coding: utf-8 -*-
"""闸门 AZ：账号恢复与认证链端到端（批三十四）。

本闸自己起一个 SMTP 诱捕容器，**读用户真收到的邮件正文**来判链路——找回密码
与邮箱激活这类链，接口 200 不代表用户能用，只有落进信箱的那串字符才算数。

钉八件事（红线现场见审计文档批三十四）：
  S1 P-100 重置邮件基址取错变量：站点对外地址的实际口径是 PUBLIC_SITE_URL
     （rss/donate/sitemap/web lib 都用它），而 reset.rs 拼网页链接却读
     PUBLIC_API_URL——现网只配了前者 ⇒ 即使 SMTP 完全可用也一律拒发
     （红线：500 + 信箱 0 封）。修法后必须真投递，且链接以站点基址开头。
  S1 P-102 配置不可用被说成系统故障：同一场景返回 500/1000「内部错误」，
     用户与运维都看不出是「站点没配对外地址」。修法：业务码 + 可读文案。
  S1 正控：取回令牌 → 重置成功 → 旧 token 立刻失效（撤销线）→ 新密码可登录
     → 同 token 重放被拒（一次性）。
  S2 P-101 激活邮件印的是相对路径 `/auth/email/verify?token=…`：邮件客户端里
     根本不成链接，而站内落地页其实是 `/email-verify`。红线抓到正文确实如此。
     修法后正文须是 `{站点基址}/email-verify?token=…`，并且用正文里那枚 token
     真能激活、激活后能登录（整条链闭合）。
  S3 P-103 未激活自救断链：登录被拦时文案让用户「在登录页用注册邮箱重新发送
     激活邮件」，但登录页没有这个入口（resend 表单只在 /email-verify 页，而
     那页只能由那封不可点的邮件抵达）。本闸钉 API 侧：resend 必须真再投一封。
  S2 P-108 更要命的一层：`/email-verify` 不在站点准入白名单里（middleware 的
     PUBLIC_PATHS 收了 /reset、/forgot，独独漏了它）——未激活用户本来就登不进
     （guard_login 拦），访问激活页却被 307 回 /login，等于「链接就算修好也
     走不到」。红线：匿名 GET /email-verify → 307 → /login?next=/email-verify。
  S4 P-104 死锁前置校验缺失：registration_mode=email_verify（设置中心确有此项）
     而 SMTP 不可用时，注册照样建号成功 ⇒ 用户永远收不到激活信、又永远登不进
     （红线实测：200 + email_verified_at NULL + 登录 400 拦死）。修法：注册
     前置拒绝并给出可执行文案，且不建号；forgot 同场景不得再铸无效令牌。
  S5 P-105 /auth/password/reset 零匿名限流：同族（pklogin begin/finish、
     passkeys finish、password/forgot）都有 rate_window，唯独它漏了。
  S6 P-106 otpauth URI 标签用用户号：验证器 App 里显示「站点:42」而不是用户名，
     多账号用户分不清哪个码属于哪个号。

共享设置：registration_mode / smtp_host / smtp_port / smtp_from / encryption
五键串行改、快照原值写回并逐键回读比对；探针号墓碑、验证/重置令牌行、
2FA 密钥、邀请与流水自己收尾；诱捕容器只删本闸起的那个。

跑法（仓库根目录，需 docker 可用）：
    PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_az_auth_recovery.py
"""
import atexit
import base64
import json
import os
import re
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
PIN = {}
UIDS = []
INVITES = []
SINK_NAME = "flux-audit-smtp-sink"
SINK_DIR = os.path.join(os.environ.get("TEMP", "/tmp"), "flux_audit_smtp")
SINK_FILE = os.path.join(SINK_DIR, "sink_out.txt")
SINK_STARTED = False
SITE_BASE = os.environ.get("AZ_SITE_BASE", "http://localhost:3000")
NEW_PW = "Reset#2026pw"

SINK_SRC = """import socket, threading
out = %r
lock = threading.Lock()

def handle(c):
    c.sendall(b"220 sink ESMTP\\r\\n")
    buf = b""
    data = False
    c.settimeout(60)
    try:
        while True:
            ch = c.recv(4096)
            if not ch:
                break
            if data:
                buf += ch
                if b"\\r\\n.\\r\\n" in buf or b"\\n.\\n" in buf:
                    with lock:
                        with open(out, "ab") as f:
                            f.write(b"===== MSG =====\\n")
                            f.write(buf)
                    buf = b""
                    data = False
                    c.sendall(b"250 queued\\r\\n")
                continue
            up = ch.strip().upper()
            if up.startswith(b"DATA"):
                data = True
                c.sendall(b"354 go\\r\\n")
            elif up.startswith(b"QUIT"):
                c.sendall(b"221 bye\\r\\n")
                break
            elif up.startswith(b"EHLO") or up.startswith(b"HELO"):
                c.sendall(b"250-sink\\r\\n250 ok\\r\\n")
            else:
                c.sendall(b"250 ok\\r\\n")
    except Exception:
        pass
    finally:
        try:
            c.close()
        except Exception:
            pass

s = socket.socket()
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind(("0.0.0.0", 2525))
s.listen(8)
print("sink up", flush=True)
while True:
    threading.Thread(target=handle, args=(s.accept()[0],),
                     daemon=True).start()
""" % SINK_DIR


def q(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % r.stderr.strip()[:200])
    return r.stdout.strip()


def ex(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-c", sql],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode:
        print("   ! SQL 失败: %s" % (r.stderr or "").strip()[:150])
    return r.returncode


def qi(sql, default=0):
    try:
        v = q(sql)
        return int(v) if v else default
    except Exception:  # noqa: BLE001
        return default


def setting(name):
    return q("SELECT COALESCE((SELECT value FROM site_settings WHERE name = "
             "'%s'), '__ABSENT__')" % name)


def setv(name, val):
    if setting(name) == "__ABSENT__":
        ex("INSERT INTO site_settings (name, value) VALUES ('%s', '%s')"
           % (name, val))
    else:
        ex("UPDATE site_settings SET value = '%s' WHERE name = '%s'"
           % (val, name))
    got = setting(name)
    print("   [set] %s = %s（回读 %s）" % (name, val, got))


def brief(x):
    return str(x)[:200]


def rej(name, s, r, code, kw):
    msg = str(r.get("message", "")) if isinstance(r, dict) else str(r)
    return ok(name, s == code and kw in msg, "%s %s %s" % (s, msg, brief(r)))


def cleanup():
    global SINK_STARTED
    for k in PIN:
        if PIN[k] == "__ABSENT__":
            ex("DELETE FROM site_settings WHERE name = '%s'" % k)
        else:
            ex("UPDATE site_settings SET value = '%s' WHERE name = '%s'"
               % (PIN[k], k))
    bad = [k for k in PIN if setting(k) != PIN[k]]
    for uid in UIDS:
        for t in ("email_verifications", "password_resets", "spark_ledger",
                  "messages"):
            col = "receiver_id" if t == "messages" else "user_id"
            ex("DELETE FROM %s WHERE %s = %d" % (t, col, uid))
        ex("UPDATE users SET totp_secret = NULL, totp_enabled = FALSE "
           "WHERE id = %d" % uid)
        ex("UPDATE users SET status = 2 WHERE id = %d" % uid)
    for cid in INVITES:
        ex("DELETE FROM invites WHERE id = %d" % cid)
    if SINK_STARTED:
        subprocess.run(["docker", "rm", "-f", SINK_NAME], capture_output=True)
        SINK_STARTED = False
    print("   [cleanup] 五键复原失配=%s；探针号=%s 已墓碑；邀请行=%d 条回收；"
          "诱捕容器已删" % (bad or "无", UIDS, len(INVITES)))


atexit.register(cleanup)


def net_name():
    r = subprocess.run(
        ["docker", "inspect", "flux-api", "--format",
         "{{range $k,$v := .NetworkSettings.Networks}}{{$k}}{{end}}"],
        capture_output=True, text=True, encoding="utf-8")
    return (r.stdout or "").strip() or "docker_default"


def start_sink():
    global SINK_STARTED
    os.makedirs(SINK_DIR, exist_ok=True)
    if os.path.exists(SINK_FILE):
        os.remove(SINK_FILE)
    with open(os.path.join(SINK_DIR, "sink.py"), "w", encoding="utf-8") as f:
        f.write(SINK_SRC)
    chk = subprocess.run(["docker", "ps", "-a", "--format", "{{.Names}}"],
                         capture_output=True, text=True, encoding="utf-8")
    if SINK_NAME in (chk.stdout or "").split():
        subprocess.run(["docker", "rm", "-f", SINK_NAME], capture_output=True)
    r = subprocess.run(
        ["docker", "run", "-d", "--name", SINK_NAME, "--network", net_name(),
         "-v", "%s:/sink" % SINK_DIR, "python:3.12-alpine",
         "python", "/sink/sink.py"],
        capture_output=True, text=True, encoding="utf-8")
    SINK_STARTED = r.returncode == 0
    print("   [sink] 启动 %s（网=%s）" % ("OK" if r.returncode == 0
                                        else "FAIL " + (r.stderr or "")[:150],
                                        net_name()))
    time.sleep(3)


def mails():
    """读诱捕到的邮件，返回可搜索的正文列表（base64/quoted-printable 都解开）。"""
    if not os.path.exists(SINK_FILE):
        return []
    raw = open(SINK_FILE, "rb").read().decode("utf-8", "replace")
    out = []
    for blk in raw.split("===== MSG =====")[1:]:
        head, _, body = blk.partition("\r\n\r\n")
        body = body.strip()
        enc = "base64" if "base64" in head.lower() else ""
        text = body
        if enc:
            try:
                text = base64.b64decode(
                    re.sub(r"\s+", "", body.split("===== ")[0])).decode(
                        "utf-8", "replace")
            except Exception:  # noqa: BLE001
                text = body
        out.append({"head": head, "body": text, "raw": blk})
    return out


def find_link(needle):
    for m in mails():
        hit = re.search(r"(\S*)%s\S*token=([0-9a-zA-Z_-]+)" % re.escape(needle),
                        m["body"])
        if hit:
            return hit.group(1), hit.group(2)
    return None, None


def new_member(role, invite=True):
    clear_register_rl()
    time.sleep(6)
    code = ""
    if invite:
        codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                       token=ROOT)[1].get("data") or {}).get("codes")) or []
        if not codes:
            return None
        code = codes[0]["code"]
        INVITES.append(codes[0]["id"])
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if len(nums) < 2:
        return None
    user = "az%s%d" % (role, TAG)
    s, rr = call("POST", "/auth/register", {
        "username": user, "email": "%s@az.local" % user,
        "password": "password123", "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}})
    uid = (rr.get("data") or {}).get("user_id")
    if not uid:
        print("   ! 注册失败 %s: %s" % (user, brief(rr)))
        return None
    UIDS.append(uid)
    tok = None
    for _ in range(8):
        tok = (call("POST", "/auth/login",
                   {"username": user, "password": "password123"})[1].get(
                       "data") or {}).get("token")
        if tok:
            break
        time.sleep(6)
    return (tok, uid, user) if tok else None


WEB_ORIGIN = os.environ.get("AZ_WEB_ORIGIN", "http://localhost:3000")


def web_probe(path):
    """匿名访问站点页面，返回 (状态码, Location)——故意不跟随重定向：
    准入闸门把人 307 回 /login 正是要抓的现象（P-108）。"""
    import urllib.error
    import urllib.request

    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, *a, **k):
            return None

    op = urllib.request.build_opener(NoRedirect)
    try:
        with op.open(WEB_ORIGIN + path, timeout=10) as r:
            return r.status, r.headers.get("location") or ""
    except urllib.error.HTTPError as e:
        return e.code, e.headers.get("location") or ""
    except Exception:  # noqa: BLE001
        return 0, ""


def open_register(role):
    """email_verify 模式下开放注册：只建号，不登录（未激活本就登不进）。"""
    clear_register_rl()
    time.sleep(6)
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if len(nums) < 2:
        return None
    user = "az%s%d" % (role, TAG)
    email = "%s@az.local" % user
    s, rr = call("POST", "/auth/register", {
        "username": user, "email": email, "password": "password123",
        "invite_code": "", "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}})
    uid = (rr.get("data") or {}).get("user_id")
    if uid:
        UIDS.append(uid)
    return {"status": s, "uid": uid, "user": user, "email": email,
            "resp": rr}


print("== S0 地基：诱捕容器 + 五键快照 + 探针成员 ==")
for k in ("registration_mode", "smtp_host", "smtp_port", "smtp_from",
          "encryption"):
    PIN[k] = setting(k)
start_sink()
ok("诱捕 SMTP 容器可用（信箱是判据的唯一来源）", SINK_STARTED)
setv("smtp_host", SINK_NAME)
setv("smtp_port", "2525")
setv("smtp_from", "audit@site.local")
setv("encryption", "none")
setv("registration_mode", "invite_only")
A = new_member("a")
B = new_member("b")
ok("两个探针成员可准备", bool(A and B))
if not (A and B):
    sys.exit(1)
AT, AID, AUSER = A
BT, BID, BUSER = B
AEMAIL = "%s@az.local" % AUSER
BEMAIL = "%s@az.local" % BUSER
# 基础设施正控：api 容器必须真能连到诱捕容器（连不上则后面全是假红）
rc = subprocess.run(
    ["docker", "exec", "flux-api", "sh", "-c",
     "wget -T 3 -O /dev/null http://%s:2525/ 2>&1 | head -1" % SINK_NAME],
    capture_output=True, text=True, encoding="utf-8")
txt = (rc.stdout or "") + (rc.stderr or "")
ok("api 容器到诱捕 SMTP 网络可达（红线：连不上就全是假红）",
   "Connection refused" not in txt and "bad address" not in txt, txt[:120])

print("== S1 找回密码全链（P-100 基址 / P-102 错误语义）==")
n0 = len(mails())
s, r = call("POST", "/auth/password/forgot", {"email": AEMAIL}, token=None)
ok("S1 P-100 forgot 返回 200 且真投递（红线：500 + 信箱 0 封）",
   s == 200 and r.get("code") == 0 and len(mails()) > n0,
   "%s %s" % (s, brief(r)))
href, tok1 = find_link("/reset")
ok("S1 P-100 重置链接以站点基址开头（红线：读 PUBLIC_API_URL 拼不出）",
   bool(tok1) and bool(href) and href.startswith(SITE_BASE),
   "href=%s" % href)
old_tok = (call("POST", "/auth/login", {"username": AUSER,
                                         "password": "password123"})[1].get(
                                           "data") or {}).get("token")
s, r = call("POST", "/auth/password/reset",
            {"token": tok1 or "x", "new_password": NEW_PW})
ok("S1 用邮件里的 token 重置成功", s == 200 and r.get("code") == 0,
   "%s %s" % (s, brief(r)))
s2, r2 = call("GET", "/me", token=old_tok)
ok("S1 改密后旧会话立即可死（撤销线）", s2 == 401 and r2.get("code") == 2001,
   "%s %s" % (s2, brief(r2)))
s3, r3 = call("POST", "/auth/login",
              {"username": AUSER, "password": NEW_PW})
ok("S1 新密码可登录", s3 == 200 and (r3.get("data") or {}).get("token"),
   "%s %s" % (s3, brief(r3)))
rej("S1 同 token 重放被拒（一次性）", *call(
    "POST", "/auth/password/reset",
    {"token": tok1 or "x", "new_password": "Another#2026pw"}),
    400, "无效或已过期")

print("== S2 邮箱激活链（P-101 相对路径 / P-103 resend / P-108 准入）==")
setv("registration_mode", "email_verify")
st_w, loc_w = web_probe("/email-verify")
ok("S2 P-108 激活页必须匿名可达（红线：准入闸门把未登录者 307 回 /login，"
   "而未激活用户恰恰登不进——链接再好也走不到）",
   st_w == 200 and "/login" not in loc_w, "%s → %s" % (st_w, loc_w))
n1 = len(mails())
V = open_register("v")
ok("S2 email_verify 模式下开放注册可建号", bool(V and V["uid"]),
   brief(V))
if V and V["uid"]:
    VUSER, VEMAIL = V["user"], V["email"]
    href2, vtok = find_link("/email-verify")
    ok("S2 P-101 激活邮件里是 `{站点基址}/email-verify?token=` 的可点链接"
       "（红线：正文只有 /auth/email/verify?token=）",
       len(mails()) > n1 and bool(vtok) and bool(href2)
       and href2.startswith(SITE_BASE),
       "封数=%d href=%s" % (len(mails()) - n1, href2))
    rej("S2 未激活时登录被拦（文案要指向 resend 入口）", *call(
        "POST", "/auth/login", {"username": VUSER, "password": "password123"}),
        400, "激活")
    n2 = len(mails())
    s, r = call("POST", "/auth/email/resend", {"email": VEMAIL})
    ok("S2 P-103 resend 真再投一封（登录页新入口走的就是这条口）",
       s == 200 and len(mails()) > n2, "%s 封数 %d→%d" % (s, n2, len(mails())))
    s, r = call("GET", "/auth/email/verify?token=%s" % vtok)
    ok("S2 用正文里的 token 激活成功", s == 200 and r.get("code") == 0,
       "%s %s" % (s, brief(r)))
    s, r = call("POST", "/auth/login",
                {"username": VUSER, "password": "password123"})
    ok("S2 激活后可正常登录（整条链闭合）",
       s == 200 and (r.get("data") or {}).get("token"), "%s %s" % (s, brief(r)))

print("== S3/S4 无邮件服务时的两条前置（P-104 死锁 / 令牌不空铸）==")
setv("smtp_from", "")
setv("registration_mode", "email_verify")
V3 = open_register("d")
DUID = qi("SELECT count(*) FROM users WHERE username = 'azd%d'" % TAG)
st3 = (V3 or {}).get("status")
ok("S3 P-104 邮件不可用时 email_verify 模式注册应被明确拒绝（红线：200 建号"
   "后永久卡死）",
   st3 is not None and st3 != 200 and st3 < 500 and DUID == 0,
   "status=%s 建号=%d msg=%s" % (st3, DUID, brief((V3 or {}).get("resp"))))
tok0 = qi("SELECT count(*) FROM password_resets")
s, r = call("POST", "/auth/password/forgot", {"email": BEMAIL})
ok("S4 P-102/P-104 forgot 在无邮件能力时给业务码而非 500",
   s < 500 and s != 200, "%s %s" % (s, brief(r)))
ok("S4 P-104 拒发时不得先铸令牌（红线：每次尝试都留一行无效 token）",
   qi("SELECT count(*) FROM password_resets") == tok0,
   "%d → %d" % (tok0, qi("SELECT count(*) FROM password_resets")))

print("== S5 P-105 /auth/password/reset 匿名限流（同族漏网）==")
seen = []
for i in range(12):
    s, r = call("POST", "/auth/password/reset",
                {"token": "bogus_%d" % i, "new_password": "Whatever#2026"})
    seen.append(s)
ok("S5 连续 12 次错 token 后必须出现 429（红线：全是 400，无上限）",
   429 in seen, "状态序列=%s" % seen)

print("== S6 P-106 otpauth 标签用用户名 ==")
s, r = call("POST", "/me/2fa/setup", {}, token=BT)
d = r.get("data") or {}
uri = d.get("otpauth_uri") or ""
# 期望 otpauth://totp/{站点}:{username}?secret=…；红线是 {站点}:{uid}
label = uri.split("?")[0].split("//")[1] if "?" in uri else ""
ok("S6 otpauth 标签里是用户名（红线：只有用户号 %d）" % BID,
   s == 200 and BUSER in label and str(BID) not in label,
   "status=%s label=%s" % (s, label[:110]))

print()
summary()
