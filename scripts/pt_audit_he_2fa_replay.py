# -*- coding: utf-8 -*-
"""HE 闸门：关掉 2FA 与登录必须共用「同码防重放」这把尺（批一百七十二 P-408）。

旧写法：`totp_disable` 调裸 `totp_verify`，而登录调 `totp_verify_login`
（Redis 记已消费码，TTL 3 个窗口）。⇒ 一枚码在登录里消费过，90 秒内还能拿来
永久关掉二次验证：两路口各记各的账，防重放被自己人绕过。对会话劫持者来说
「旁观到一次登录」=「可以永久移除 2FA」，正是要堵的持久化路径。

实测 A/B（探针会员，SQL 直接铺已知 base32 密钥）：
  旧镜像 :8080  登录消费 58454 → 同码 POST /me/2fa/disable = http 200，
               库里 totp_enabled=f ⇒ 洞确认可利用；
  新二进制 :8099 同一手法 http=400「验证码不正确」，totp_enabled 仍 t；
               换未消费的新码则正常关闭（正控，没误伤合法路径）。

用法：`python -u scripts/pt_audit_he_2fa_replay.py`
"""
import io
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import ok, summary  # noqa: E402

TFA = "apps/api/src/twofa_http.rs"


def cut(s):
    return "\n".join(
        ln for ln in s.split("\n")
        if not ln.strip().startswith("//"))


def bare_verify(seg):
    """这段是否仍在用裸 totp_verify（有 totp_verify_login 就不算）。"""
    if not seg:
        return None
    return ("totp_verify(&raw" in seg
            and "totp_verify_login" not in seg)


t = cut(io.open(os.path.join(ROOT, TFA), encoding="utf-8",
                 errors="replace").read())
seg = t.split("fn totp_disable")[1].split("fn ")[0] if (
    "fn totp_disable" in t) else ""
bad = bare_verify(seg)
ok("HE1 关掉 2FA 走 totp_verify_login（与登录同一把尺）",
   bool(seg) and bad is False, "读数=%s（True=仍是裸校验）" % bad)
OLD = ("let raw = BASE32_NOPAD.decode(b32)?; if !totp_verify(&raw, "
       "body.code) { return Err(..); }")
ok("HE1b 承重自证：同一谓词打旧的裸校验必须为真",
   bare_verify(OLD) is True, "打不中＝认不出旧形状，判据是橡皮章")
# 登录那一路不许被反向改坏：它必须仍在用带防重放的校验
lg = t.split("fn login_totp_check")[1].split("fn ")[0] if (
    "fn login_totp_check" in t) else ""
ok("HE2 登录侧仍走带防重放的校验（不许把两把尺统一成裸校验）",
   "totp_verify_login" in lg, "退化＝一枚码可重复用于登录，锁死 acctlock 语义")

# ---------- HE3..HE5：/me/passkey/rotate 必须复核本人（P-409 已修） ----------
# 旧形状（:8080 现网实测，探针会员，无旧密码/无 TOTP/无 body）：
#   POST /me/passkey/rotate -> http=200，返回新 passkey，库里 passkey 已换，
#   该号未撤销的 RSS token 从 1 变 0（download_keys 同口一起废）；
#   同文件 `/me/password/change` 用错旧密码 -> 400「旧密码不正确」⇒ 一个文件两把尺。
# 会话被劫持 = 攻击者拿到新的 tracker/RSS 身份、把受害者所有下载器与命名订阅
# 打成 401，且**不触发任何登出联动**（改密才抬 logout_nbf）。
# 修法：step_up（密码必对；开了 2FA 再要当前码）+ 5 次/分限流（复核本身
# 会试密码，不能变成新的 oracle）；前端拆出 passkey-rotate.tsx 带两个输入。
SEC = "apps/api/src/auth_http/me_security.rs"
sec = cut(io.open(os.path.join(ROOT, SEC), encoding="utf-8",
                  errors="replace").read())
rot = sec.split("fn rotate_passkey")[1].split("#[post")[0] if (
    "fn rotate_passkey" in sec) else ""
pw = sec.split("fn me_password_change")[1].split("#[derive")[0] if (
    "fn me_password_change" in sec) else ""


def needs_reauth(x):
    return ("step_up" in x and "rate_window" in x
            and "password" in x)


ok("HE3 轮换 passkey 先复核本人并限流（P-409）",
   bool(rot) and needs_reauth(rot),
   "缺 step_up ⇒ 会话被偷即可白拿新钥 + 废旧钥 + 撤 RSS/下载键")
OLD3 = ("async fn rotate_passkey(req: HttpRequest,\n"
        "    state: web::Data<std::sync::Arc<AppState>>,\n"
        ") -> DomainResult<impl Responder> {\n"
        "    let auth = require_auth(&req, &state).await?;\n"
        "    let pk = state.repo.update_passkey(auth.id, false).await?;\n}")
ok("HE3b 承重自证：旧的裸轮换形状必须判红",
   needs_reauth(OLD3) is False,
   "打不中＝认不出旧形状，这条判据是橡皮章")
WEB = "apps/web/components/passkey-rotate.tsx"
USERC = "apps/web/components/usercp-security.tsx"
web = io.open(os.path.join(ROOT, WEB), encoding="utf-8",
              errors="replace").read() if os.path.exists(
                  os.path.join(ROOT, WEB)) else ""
userc = io.open(os.path.join(ROOT, USERC), encoding="utf-8",
                errors="replace").read()
ok("HE3c 前端轮换入口带密码/动态码并用新组件",
   '"/api/v1/me/passkey/rotate"' in web and "password" in web
   and "PasskeyRotate" in userc,
   "旧界面是无输入裸按钮 ⇒ 后端收紧后用户会永久 400")
ok("HE4 同一文件的两把尺对照：改密确实要旧密码",
   "verify_password" in pw and "old_password" in pw,
   "若哪天把改密的旧密码校验也拿掉，这条与 HE3 一起说明全站尺子都没了")
ok("HE5 轮换仍是「破坏性 + 取利」一体（新钥回给调用方且旧钥即刻失效）",
   "update_passkey(auth.id, false)" in rot
   and 'json!({ "passkey": pk })' in sec,
   "P-409 的成因：一次调用既发新身份又废旧身份，还连带撤 RSS/下载键")

# ---------- HE6/HE7 重置密码与自助改密必须同尺（P-410） ----------
# 「忘记密码」成功 = 用户已经拿到并设置了新密码，那么装机临时密码标志位
# `must_reset_password` 就该和自助改密一样清掉、一样失效 5s 状态缓存。
# 旧写法只改 pass_hash ⇒ 走邮件重置的用户登录回来仍被回显 must_reset_password=True，
# 安装向导下一步继续吃 400（同一个标志位，两条出口两把尺）。
RESET = "apps/api/src/gaps_http/reset.rs"
rs = cut(io.open(os.path.join(ROOT, RESET), encoding="utf-8",
                 errors="replace").read())
seg = rs.split("fn password_reset")[1].split("#[derive")[0] if (
    "fn password_reset" in rs) else ""


def flag_cleared(x):
    return ("must_reset_password = false" in x
            and "user_status_cache.invalidate" in x)


ok("HE6 重置口清临时密码标志位并失效状态缓存（与改密同尺）",
   bool(seg) and flag_cleared(seg) and "token_revocations" in seg,
   "缺一样都会让走邮件重置的用户继续被「临时密码」卡住")
OLD6 = "UPDATE users SET pass_hash = $2 WHERE id = $1"
ok("HE6b 承重自证：旧的「只改哈希」写法必须判红",
   flag_cleared(OLD6) is False and "must_reset_password" not in OLD6,
   "打不中＝这条认不出旧形状")

summary()
