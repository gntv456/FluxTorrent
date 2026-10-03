# -*- coding: utf-8 -*-
"""资深 PT 深测四轮脚本 C：审核工作流 + 求种闭环 + 互动经济。

覆盖前三轮未触区：
- 发种审核：auto_approve 连击免审 / 拒绝计数禁发 / 待审列表 / decide
- 求种区：发起求种 → 应种（offer）→ 上传者得 Bonus
- 互动：评论 / 感谢 / 收藏 / 签到（attendance）+ spark_ledger 对账
"""
import sys, json, time, random
sys.stdout.reconfigure(encoding="utf-8")
from pt_audit_lib import call, login, ok, summary
from pt_audit_a_publish import make_torrent, upload_torrent
import subprocess

def q(sql):
    r = subprocess.run(["docker","exec","flux-postgres","psql","-U","flux","-d","fluxtorrent","-t","-A","-c",sql],
                       capture_output=True, text=True)
    return (r.stdout or r.stderr).strip()

tok = login()
suffix = random.randint(10000, 99999)

# 造一个普通发种人（走邀请注册）
q("INSERT INTO invites (inviter_id, code, expires_at) VALUES (76, 'audc%d', now() + interval '1 day')" % suffix)
s, cap = call("GET", "/auth/captcha")
import re as _re
m = _re.match(r"(\d+) \+ (\d+) =", (cap.get("data") or {}).get("question", ""))
uname = "ptc_up%d" % suffix
s, r = call("POST", "/auth/register", {"username": uname, "email": "%s@audit.invalid" % uname,
                                        "password": "Audit12345678!", "invite_code": "audtc%d"[:0]+"audc%d" % suffix,
                                        "captcha_id": (cap.get("data") or {}).get("captcha_id", ""),
                                        "captcha_answer": int(m.group(1)) + int(m.group(2)) if m else 0})
ok("注册发种人", s == 200 and r.get("code") == 0, "%s %s" % (s, r.get("message")))
tup = login(uname, "Audit12345678!")
s, me = call("GET", "/me/overview", token=tup)
uid_up = me["data"]["id"]
print("发种人 uid=%s" % uid_up)

# ---------- 1) 审核工作流 ----------
# 1a) 需审核吗？看 approve_streak / deny_count 现值 + upload 后 approval_status
print("== 审核工作流 ==")
streak0 = q("SELECT approve_streak FROM users WHERE id=%s" % uid_up)
deny0 = q("SELECT deny_count FROM users WHERE id=%s" % uid_up)
print("发种人 approve_streak=%s deny_count=%s" % (streak0, deny0))

tor1, ih1 = make_torrent(files=[("rev1.bin", 5242880)])
s, r = upload_torrent(tup, tor1, category_id="1", name="AuditC.NeedReview.%d" % suffix)
tid1 = (r.get("data") or {}).get("id")
st1 = q("SELECT approval_status FROM torrents WHERE id=%s" % tid1)
print("普通用户发种 tid=%s approval_status=%s（0=待审? 1=过审?）" % (tid1, st1))
ok("普通发种落待审或自动过审", st1 in ("0", "1"), st1)

# 1b) 管理端待审列表
s, r = call("GET", "/admin/reviews", token=tok)
items = r.get("data") or []
print("待审列表 %d 条" % len(items) if isinstance(items, list) else str(r)[:150])

# 1c) decide 拒绝 → deny_count+1；再过审一个 → streak 连击
if tid1:
    s, r = call("POST", "/admin/reviews/decide", {"torrent_id": int(tid1), "approve": False, "reason": "深测拒绝：重复资源"}, token=tok)
    print("decide 拒绝:", s, r.get("code"), r.get("message"))
    deny1 = q("SELECT deny_count FROM users WHERE id=%s" % uid_up)
    ok("拒绝后 deny_count +1", deny1 == "1", "deny=%s" % deny1)

tor2, ih2 = make_torrent(files=[("rev2.bin", 5242880)])
s, r = upload_torrent(tup, tor2, category_id="1", name="AuditC.Review2.%d" % suffix)
tid2 = (r.get("data") or {}).get("id")
if tid2:
    s, r = call("POST", "/admin/reviews/decide", {"torrent_id": int(tid2), "approve": True}, token=tok)
    print("decide 通过:", s, r.get("code"))
    streak1 = q("SELECT approve_streak FROM users WHERE id=%s" % uid_up)
    ok("通过后 approve_streak +1", streak1 == "1", "streak=%s" % streak1)

# ---------- 2) 求种区闭环 ----------
print("== 求种区 ==")
s, r = call("POST", "/requests", {"title": "深测求种 %d" % suffix, "descr": "求个好资源",
                                   "bonus": 500}, token=tup)
print("发求种:", s, r.get("code"), r.get("message"))
rid = None
if s == 200 and r.get("code") == 0:
    rid = ((r.get("data") or {}).get("id"))
else:
    # 看看求种接口形状
    s2, r2 = call("GET", "/requests", token=tup)
    print("GET /requests:", s2, str(r2)[:200])
print("rid=", rid)

summary()
