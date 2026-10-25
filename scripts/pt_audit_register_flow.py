# -*- coding: utf-8 -*-
"""注册链路闸门：邀请码 + 算术验证码 → 建号 → 登录 → 收尾。

覆盖一条普通 PT 用户天天要走、但其它闸门没覆盖的路：
  1. 管理员发一张邀请码（admin/users/adjust 的 invite_grant 是
     「直发有效邀请」，不走 quota_extra、不占周配额）；
  2. 自研算术验证码（captcha_provider=none 时也仍是必填的防机闸，
     答错/不给都要被拒）；
  3. 用码注册：码被一次性消费（invites.status=1 + used_by）；
  4. 同一个码不能注册第二个人；
  5. 新号能登录（临时密码闸门要有明确答复，不许 500）；
  6. 收尾走产品删除链（墓碑行是预期残留），并把注册限额的临时
     放宽还原、把本次注册在 reg_ip_signups 里的占位行清掉。
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, psql_stdin, ok, summary  # noqa: E402

ROOT = login()
TS = int(time.time()) % 100000
NEWU = "rg%d" % TS
INVITER = "in%d" % TS
PW = "Gate#123456"


def q(sql_text):
    return psql_stdin(sql_text)


def set_limit(value):
    q("UPDATE site_settings SET value = '%s' "
      "WHERE name = 'reg_ip_limit'" % value)


def captcha():
    """取一张算术验证码并解出来。"""
    s, r = call("GET", "/auth/captcha", None)
    cap = (r.get("data") or {})
    text = str(cap.get("question") or "")
    try:
        left, right = text.split("+")
        ans = int(left.strip()) + int(right.split("=")[0].strip())
    except (ValueError, IndexError):
        ans = -1
    return cap.get("captcha_id"), ans, s


def cleanup(uid, inv_id, names):
    for name in names:
        u = q("SELECT id FROM users WHERE username = '%s'" % name).strip()
        if u:
            q("UPDATE users SET status = 2 WHERE id = %s" % u)
            call("DELETE", "/admin/users/%s" % u, None, token=ROOT)
    if uid:
        q("DELETE FROM reg_ip_signups WHERE user_id = '%s'" % uid)
    if inv_id:
        q("DELETE FROM invites WHERE inviter_id = '%s'" % inv_id)


print("== S1 前提：注册模式、验证码驱动、同 IP 注册限额 ==")
mode = q("SELECT COALESCE(value,'invite_only') FROM site_settings "
         "WHERE name = 'registration_mode'")
ok("S1 注册模式读得到", mode != "", "mode=%s" % mode)
provider = q("SELECT COALESCE(value,'none') FROM site_settings "
             "WHERE name = 'captcha_provider'")
ok("S1 验证码驱动读得到", provider != "", "provider=%s" % provider)
lim_old = q("SELECT COALESCE(value,'0') FROM site_settings "
            "WHERE name = 'reg_ip_limit'")
print("   同 IP 限额原值:", lim_old)

inv_id = ""
uid = ""
try:
    # 闸门要连开两个号（正控 + 负控各一次），先把同 IP 限额放宽，
    # 收尾时原样还原——不留任何设置漂移。
    set_limit("50")

    print("== S2 管理员发邀请码 ==")
    call("POST", "/admin/adduser", {"username": INVITER, "password": PW,
                                    "email": "%s@rg.local" % INVITER},
         token=ROOT)
    inv_id = q("SELECT id FROM users WHERE username = '%s'" % INVITER).strip()
    ok("S2 邀请人建号", inv_id != "", "uid=%s" % inv_id)
    extra_before = q("SELECT COALESCE(quota_extra::text,'0') FROM users "
                     "WHERE id = %d" % int(inv_id))
    s, r = call("POST", "/admin/users/adjust",
                {"user_id": int(inv_id), "invite_grant": 1,
                 "note": "register flow probe"}, token=ROOT)
    ok("S2 发码受理", s == 200, "http=%s %s" % (s, str(r)[:110]))
    code = q("SELECT code FROM invites WHERE inviter_id = %d "
             "AND used_by IS NULL ORDER BY id DESC LIMIT 1"
             % int(inv_id)).strip()
    ok("S2 码落库", code != "", "code=%s" % code[:10])
    extra_after = q("SELECT COALESCE(quota_extra::text,'0') FROM users "
                    "WHERE id = %d" % int(inv_id))
    ok("S2 直发邀请码不动 quota_extra（也不占周配额）",
       extra_after == extra_before,
       "%s → %s" % (extra_before, extra_after))

    print("== S3 注册：验证码 + 邀请码 ==")
    cid, ans, cs = captcha()
    ok("S3 取到算术验证码", bool(cid) and ans > 0,
       "http=%s ans=%s" % (cs, ans))
    s, r = call("POST", "/auth/register",
                {"username": NEWU, "email": "%s@rg.local" % NEWU,
                 "password": PW, "invite_code": code,
                 "captcha_id": cid, "captcha_answer": ans})
    ok("S3 注册受理", s == 200, "http=%s %s" % (s, str(r)[:150]))
    uid = q("SELECT id FROM users WHERE username = '%s'" % NEWU).strip()
    ok("S3 新号落库", uid != "", "uid=%s" % uid)
    row = q("SELECT status||'|'||COALESCE(used_by::text,'-') FROM invites "
            "WHERE code = '%s'" % code)
    ok("S3 码被一次性消费（status=1 且 used_by 对得上）",
       row.split("|")[0] == "1" and row.split("|")[1] == uid,
       "row=%s uid=%s" % (row, uid))
    ok("S3 同 IP 注册占位有记账",
       q("SELECT count(*) FROM reg_ip_signups WHERE user_id = %s" % uid) == "1",
       "")

    print("== S4 负控：同一个码不能注册第二个人 ==")
    cid2, ans2, _ = captcha()
    s, r = call("POST", "/auth/register",
                {"username": NEWU + "b", "email": "%sb@rg.local" % NEWU,
                 "password": PW, "invite_code": code,
                 "captcha_id": cid2, "captcha_answer": ans2})
    ok("S4 旧码被拒", s != 200, "http=%s %s" % (s, str(r)[:110]))
    ok("S4 第二个号没有落库",
       q("SELECT count(*) FROM users WHERE username = '%sb'" % NEWU) == "0", "")

    print("== S4b 负控：过期码 / 已回收的码都不许过 ==")
    exp_code = "exp" + str(TS)
    q("INSERT INTO invites (inviter_id, code, status, expires_at) "
      "VALUES (%d, '%s', 0, now() - interval '1 day')"
      % (int(inv_id), exp_code))
    cid4, ans4, _ = captcha()
    s, r = call("POST", "/auth/register",
                {"username": NEWU + "e", "email": "%se@rg.local" % NEWU,
                 "password": PW, "invite_code": exp_code,
                 "captcha_id": cid4, "captcha_answer": ans4})
    msg = str((r or {}).get("message") or "")
    ok("S4b 过期码被拒（且理由是码本身）", s != 200 and "邀请" in msg,
       "http=%s msg=%s" % (s, msg[:90]))
    rev_code = "rev" + str(TS)
    q("INSERT INTO invites (inviter_id, code, status, expires_at) "
      "VALUES (%d, '%s', 2, now() + interval '30 days')"
      % (int(inv_id), rev_code))
    cid5, ans5, _ = captcha()
    s, r = call("POST", "/auth/register",
                {"username": NEWU + "f", "email": "%sf@rg.local" % NEWU,
                 "password": PW, "invite_code": rev_code,
                 "captcha_id": cid5, "captcha_answer": ans5})
    msg = str((r or {}).get("message") or "")
    ok("S4b 已回收的码被拒（且理由是码本身）", s != 200 and "邀请" in msg,
       "http=%s msg=%s" % (s, msg[:90]))
    ok("S4b 负控期间没建出号",
       q("SELECT count(*) FROM users WHERE username LIKE 'rg%d%%'" % TS) == "1",
       "")
    print("== S5 验证码负控：不给 / 答错都不许过 ==")
    s, r = call("POST", "/auth/register",
                {"username": NEWU + "c", "email": "%sc@rg.local" % NEWU,
                 "password": PW, "invite_code": code})
    ok("S5 无验证码被拒", s != 200, "http=%s %s" % (s, str(r)[:110]))
    cid3, ans3, _ = captcha()
    s, r = call("POST", "/auth/register",
                {"username": NEWU + "d", "email": "%sd@rg.local" % NEWU,
                 "password": PW, "invite_code": code,
                 "captcha_id": cid3, "captcha_answer": ans3 + 1})
    ok("S5 验证码答错被拒", s != 200, "http=%s %s" % (s, str(r)[:110]))
    ok("S5 负控期间没建出号",
       q("SELECT count(*) FROM users WHERE username LIKE 'rg%d%%'" % TS) == "1",
       "")

    print("== S6 新号登录 ==")
    s, r = call("POST", "/auth/login", {"username": NEWU, "password": PW})
    ok("S6 登录有明确答复（不许 500）", s in (200, 400),
       "http=%s %s" % (s, str(r)[:110]))
finally:
    cleanup(uid, inv_id, [NEWU, INVITER])
    set_limit(lim_old or "2")
    now = q("SELECT COALESCE(value,'(缺省)') FROM site_settings "
            "WHERE name = 'reg_ip_limit'")
    ok("S7 同 IP 注册限额已还原", now == (lim_old or "2"),
       "%s → %s" % (lim_old, now))
    ok("S7 探针号已墓碑（status=3）",
       q("SELECT count(*) FROM users WHERE id IN (%s, %s) AND status = 3"
         % (uid or 0, inv_id or 0)) == "2", "")
    ok("S7 邀请码已清",
       q("SELECT count(*) FROM invites WHERE inviter_id = %d"
         % int(inv_id or 0)) == "0", "")
    ok("S7 注册占位行已清",
       q("SELECT count(*) FROM reg_ip_signups WHERE user_id = %s"
         % (uid or 0)) == "0", "")
summary()
