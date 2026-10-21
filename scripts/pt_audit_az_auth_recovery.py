# -*- coding: utf-8 -*-
"""闸门 AZ：账号恢复与认证链端到端（批三十四）。

本闸自己起一个 SMTP 诱捕容器，**读用户真收到的邮件正文**来判链路——找回密码
与邮箱激活这类链，接口 200 不代表用户能用，只有落进信箱的那串字符才算数。
信箱取自 `docker logs`（容器 stdout）而不是 bind mount 里的文件：宿主是
Windows，容器内不存在那条宿主路径，写盘异常会被诱捕脚本的 `except: pass`
静静吞掉 ⇒ 表现为「一封都没抓到」的假红。

钉九件事（红线现场见审计文档批三十四）：
  S1 P-100 重置邮件基址取错变量：站点对外地址的实际口径是 PUBLIC_SITE_URL
     （rss/donate/sitemap/web lib 都用它），而 reset.rs 拼网页链接却读
     PUBLIC_API_URL——现网只配了前者 ⇒ 即使 SMTP 完全可用也一律拒发
     （红线：500 + 信箱 0 封）。修法后必须真投递，且链接以站点基址开头。
  S1 P-102 配置不可用被说成系统故障：同一场景返回 500/1000「内部错误」，
     用户与运维都看不出是「站点没配对外地址」。修法：业务码 + 可读文案。
  S1 正控：取回令牌 → 重置成功 → 旧 token 立刻失效（撤销线）→ 新密码可登录
     → 同 token 重放被拒（一次性）。
  S2 P-101 激活邮件印的是相对路径 `/auth/email/verify?token=…`：邮件客户端里
     根本不成链接，而站内落地页其实是 `/email-verify`。该条红是**源码层实证**
     （红线那轮信箱通道还没通电，见上），修法后改由真邮件正文断言：
     正文须是 `{站点基址}/email-verify?token=…`，并且用正文里那枚 token
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
  S7 P-109 resend 是「邮件能力前置」的第三条漏网出口（forgot/register 已在前
     面收口）：SMTP 没配时它照样作废旧链接、铸新链接、回 200 sent:true，前端
     如实显示「激活邮件已发出」。红线实测（站点态 smtp_from 为空）：A 信箱里
     那枚仍有效的链接被置过期、库里多一行永不达的新令牌、HTTP 200。修法：与
     forgot 同口径回 503/4103，且在碰任何令牌之前判。
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
from pt_smtp_sink import (  # noqa: E402
    SINK_NAME, find_link, link_note, mails, reachable, selftest,
    sink_addr, start_sink, stop_sink, wait_mails)

ROOT = login()
TAG = int(time.time()) % 100000
PIN = {}
UIDS = []
INVITES = []
SITE_BASE = os.environ.get("AZ_SITE_BASE", "http://localhost:3000")
NEW_PW = "Reset#2026pw"


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
    stop_sink()
    print("   [cleanup] 五键复原失配=%s；探针号=%s 已墓碑；邀请行=%d 条回收；"
          "诱捕容器已删" % (bad or "无", UIDS, len(INVITES)))


atexit.register(cleanup)


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
ok("诱捕 SMTP 容器可用（信箱是判据的唯一来源）", start_sink())
# 地址跟 API 所在选：容器内用容器名，宿主侧进程用发布端口。
# 批一百五十三：硬写容器名跟宿主侧进程登录失败，
# 表现为 forgot 返回 200 但信箱零封——冒充产品红。
SINK_HOST, SINK_PORT = sink_addr()
setv("smtp_host", SINK_HOST)
setv("smtp_port", str(SINK_PORT))
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
cond_rc, detail_rc = reachable()
ok("api 容器到诱捕 SMTP 网络可达（红线：连不上就全是假红）",
   cond_rc, detail_rc)
cond_st, detail_st = selftest()
ok("信箱通电正控：自己发一封必须能原样读回（缺它则「0 封」不可信）",
   cond_st, detail_st)

# P-302：信箱不可用时早退。S1/S2 共 11 条断言以「读回邮件」为唯一证据，
# 前置红掉还照跑就会产出 14 行红账，把根因埋进噪音里（本机拉不到
# python:3.12-alpine 时属环境账）。红只留上面 3 条，未验收仍回非零码。
if __import__("pt_audit_lib").FAILURES:
    print("\n[BLOCKED] 诱捕 SMTP 信箱不可用：S1/S2 的邮件断言全部跳过。")
    print("   这不是产品红——根因＝上面三条前置（环境恢复后自然复跑）。")
    summary()
    raise SystemExit(1)     # atexit 注册的 cleanup() 照常复原五键设定

print("== S1 找回密码全链（P-100 基址 / P-102 错误语义）==")
n0 = len(mails())
s, r = call("POST", "/auth/password/forgot", {"email": AEMAIL}, token=None)
ok("S1 P-100 forgot 返回 200 且真投递（红线：500 + 信箱 0 封）",
   s == 200 and r.get("code") == 0 and wait_mails(n0),
   "%s %s 新增封数=%d" % (s, brief(r), len(mails()) - n0))
href, tok1 = find_link("/reset")
link_note("S1 重置", href, tok1)
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
    got = wait_mails(n1)
    href2, vtok = find_link("/email-verify")
    link_note("S2 激活", href2, vtok)
    ok("S2 P-101 激活邮件里是 `{站点基址}/email-verify?token=` 的可点链接"
       "（红线：正文只有 /auth/email/verify?token=）",
       got and bool(vtok) and bool(href2) and href2.startswith(SITE_BASE),
       "封数=%d href=%s" % (len(mails()) - n1, href2))
    rej("S2 未激活时登录被拦（文案要指向 resend 入口）", *call(
        "POST", "/auth/login", {"username": VUSER, "password": "password123"}),
        400, "激活")
    n2 = len(mails())
    vtok_old = vtok
    s, r = call("POST", "/auth/email/resend", {"email": VEMAIL})
    ok("S2 P-103 resend 真再投一封（登录页新入口走的就是这条口）",
       s == 200 and wait_mails(n2),
       "%s 封数 %d→%d" % (s, n2, len(mails())))
    # 重发前会把该用户未用的验证令牌一律置过期（email_verify.rs:74）——
    # 同一个人手里不能并存两条可用链接，这是防爆破的口径，本闸如实钉住
    rej("S2 重发后旧链接作废（不得双活）",
        *call("GET", "/auth/email/verify?token=%s" % vtok_old), 400, "过期")
    href3, vtok = find_link("/email-verify")
    ok("S2 最新一封里仍是 `{站点基址}` 开头的可点链接",
       bool(vtok) and bool(href3) and href3.startswith(SITE_BASE),
       "href=%s" % href3)
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
   "后永久卡死）——契约 503/4103",
   st3 == 503 and ((V3 or {}).get("resp") or {}).get("code") == 4103
   and DUID == 0,
   "status=%s 建号=%d msg=%s" % (st3, DUID, brief((V3 or {}).get("resp"))))
tok0 = qi("SELECT count(*) FROM password_resets")
s, r = call("POST", "/auth/password/forgot", {"email": BEMAIL})
ok("S4 P-102/P-104 forgot 在无邮件能力时如实回业务码（契约 503/4103+文案，"
   "红线：500 内部错误）",
   s == 503 and r.get("code") == 4103 and "邮件" in str(r.get("message")),
   "%s %s" % (s, brief(r)))
ok("S4 P-104 拒发时不得先铸令牌（红线：每次尝试都留一行无效 token）",
   qi("SELECT count(*) FROM password_resets") == tok0,
   "%d → %d" % (tok0, qi("SELECT count(*) FROM password_resets")))

print("== S7 P-109 resend 的能力前置（无邮件服务时连令牌都不许碰）==")
# 先给 A 一枚「用户信箱里还躺着」的有效激活链接，再在 SMTP 不可用的站点态
# 下调 resend：红线是它照发作废旧枚 + 铸新枚 + 回 sent:true（自救口反而
# 把手里唯一可用链接打死）
ex("INSERT INTO email_verifications (user_id, token_hash, expires_at) "
   "VALUES (%d, 'az-s7-live', now() + interval '48 hours')" % AID)
row0 = qi("SELECT count(*) FROM email_verifications WHERE user_id = %d" % AID)
live0 = qi("SELECT count(*) FROM email_verifications WHERE user_id = %d "
           "AND expires_at > now()" % AID)
tk0 = q("SELECT COALESCE(string_agg(token_hash, ','), '-') "
        "FROM email_verifications WHERE user_id = %d "
        "AND expires_at > now()" % AID)
ok("S7 前置成立：A 手里确有一枚未过期链接（否则下面全是空判）",
   row0 >= 1 and live0 >= 1, "rows=%d live=%d" % (row0, live0))
s, r = call("POST", "/auth/email/resend", {"email": AEMAIL})
ok("S7 P-109 无邮件能力时 resend 如实回 503/4103（红线：200 sent:true 假承诺）",
   s == 503 and r.get("code") == 4103, "%s %s" % (s, brief(r)))
row1 = qi("SELECT count(*) FROM email_verifications WHERE user_id = %d" % AID)
live1 = qi("SELECT count(*) FROM email_verifications WHERE user_id = %d "
           "AND expires_at > now()" % AID)
tk1 = q("SELECT COALESCE(string_agg(token_hash, ','), '-') "
        "FROM email_verifications WHERE user_id = %d "
        "AND expires_at > now()" % AID)
ok("S7 P-109 拒发不得作废旧链接、不得铸新链接（红线：%d/%d 被改成作废+多一行）"
   % (row0, live0), row1 == row0 and live1 == live0,
   "rows %d→%d；live %d→%d" % (row0, row1, live0, live1))
ok("S7 P-109 信箱里那枚链接必须还是同一枚（红线：live 数不变但被换成永不达的新枚）",
   tk1 == tk0, "%s… → %s…" % (tk0[:12], tk1[:12]))

# P-111：resend 自己的档——模块注释与界面文案都写「1 分钟一次」，代码却走
# 通用 throttle()（=登录档 5 次/分）。档不对，用户连点就把可用链接连杀几枚。
seens = []
for _ in range(3):
    seens.append(call("POST", "/auth/email/resend", {"email": BEMAIL})[0])
ok("S7b P-111 resend 同邮箱同 IP 连发：第 2 次起必须 429（红线：通用档"
   " 5 次/分，三次全是 503）", seens[1:] == [429, 429],
   "状态序列=%s" % seens)

print("== S5 P-105 /auth/password/reset 匿名限流（同族漏网）==")
seen = []
for i in range(12):
    s, r = call("POST", "/auth/password/reset",
                {"token": "bogus_%d" % i, "new_password": "Whatever#2026"})
    seen.append(s)
ok("S5 连续 12 次错 token 后必须出现 429（红线：全是 400，无上限）",
   429 in seen, "状态序列=%s" % seen)

print("== S6 P-106 otpauth 标签用用户名 ==")
s, r = call("POST", "/me/2fa/setup", {"password": "password123"},
            token=BT)
d = r.get("data") or {}
uri = d.get("otpauth_uri") or ""
# 期望 otpauth://totp/{站点}:{username}?secret=…；红线是 {站点}:{uid}
label = uri.split("?")[0].split("//")[1] if "?" in uri else ""
ok("S6 otpauth 标签里是用户名（红线：只有用户号 %d）" % BID,
   s == 200 and BUSER in label and str(BID) not in label,
   "status=%s label=%s" % (s, label[:110]))

print()
summary()
