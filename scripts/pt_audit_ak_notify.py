# -*- coding: utf-8 -*-
"""批二十闸门 AK：通知中心的「未读私信」数字是不是真的。

P-29 实测：`/api/v1/me/overview` 从来不回 `unread_messages`，而通知中心
读的是 `ov?.unread_messages ?? 0` —— 于是**任何成员的未读私信卡恒显示 0**
（root 实测收件箱 56 封未读，卡片写 0）。这张卡是整个站内信的唯一聚合入口，
数字说谎等于把「你有信没看」这条链从界面上抹掉。

判据钉三件事：字段真的存在且等于发信条数；卡片数字等于它自己链接指向的
列表长度（`/messages/inbox?unread=true`）；root 的计数与 SQL 原值一致
（谓词只准有一套：unread + location=1 + 主收件箱）。
"""
import os
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402

ROOT = login()
CLEAN = []
M = "ak%d" % (int(time.time()) % 100000)
PW = "Gate#123456"


def q(sql_text):
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", sql_text],
        capture_output=True, text=True, encoding="utf-8")
    return p.stdout.strip()


def exec_sql(sql_text):
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-c", sql_text],
        capture_output=True, text=True, encoding="utf-8")
    if p.returncode:
        raise RuntimeError("psql 失败: %s" % (p.stderr or "")[:220])


print("== N0 探针成员 + 两封未读私信 ==")
call("POST", "/admin/adduser", {"username": M, "password": PW,
                               "email": "%s@ak.local" % M}, token=ROOT)
uid = int(q("SELECT id::text FROM users WHERE username = '%s'" % M))
CLEAN.append("UPDATE users SET status = 2 WHERE id = %d" % uid)
CLEAN.append("DELETE FROM messages WHERE receiver_id = %d OR sender_id = %d"
             % (uid, uid))
tok0 = login(M, PW)
call("POST", "/me/password/change", {"old_password": PW,
                                     "new_password": PW + "x"}, token=tok0)
mt = login(M, PW + "x")
sent = []
for i in (1, 2):
    s, r = call("POST", "/messages", {"to": M, "subject": "AK 探针信 %d" % i,
                                     "body": "闸门正文"}, token=ROOT)
    sent.append(s == 200 and r.get("code") == 0)
ok("N0 两封私信发出", all(sent), str(sent))
ok("N0 SQL 侧确有 2 封未读（unread + location=1 + 主收件箱）",
   q("SELECT count(*) FROM messages WHERE receiver_id = %d AND unread "
     "AND location = 1 AND folder IS NULL" % uid) == "2",
   q("SELECT count(*) FROM messages WHERE receiver_id = %d" % uid))

print("== N1 卡片数据源：/me/overview 必须真的带未读数 ==")
s, r = call("GET", "/me/overview", None, token=mt)
ov = r.get("data") or {}
ok("N1 /me/overview 有 unread_messages 字段（此前没有，页面 ?? 0 兜住）",
   "unread_messages" in ov, "keys=%d" % len(ov))
ok("N1 成员未读数 = 2", ov.get("unread_messages") == 2,
   "api=%s" % ov.get("unread_messages"))

print("== N2 卡片数字必须等于它自己链接指向的列表 ==")
s, r = call("GET", "/messages/inbox?unread=true", None, token=mt)
rows = r.get("data")
rows = rows if isinstance(rows, list) else (rows or {}).get("items") or []
ok("N2 未读列表也是 2 条（数字与列表打架就是假数字）",
   len(rows) == ov.get("unread_messages") == 2,
   "list=%d api=%s" % (len(rows), ov.get("unread_messages")))

print("== N3 管理员侧同一条谓词（root 56 封未读那种真实存量） ==")
s, r = call("GET", "/me/overview", None, token=ROOT)
root_ov = (r.get("data") or {}).get("unread_messages")
root_sql = int(q("SELECT count(*) FROM messages WHERE receiver_id = 1 "
                 "AND unread AND location = 1 AND folder IS NULL"))
ok("N3 root 的 overview 未读数与 SQL 原值一致", root_ov == root_sql,
   "api=%s sql=%d" % (root_ov, root_sql))
ok("N3 存量确实非零（判据不是靠空表蒙对）", root_sql > 0, "sql=%d" % root_sql)

print("== N5 分流到自建文件夹后：三处口径必须同一个数 ==")
# P-31 实测：/me 的角标计数原本不看 folder，把未读信移进自建夹后它仍报
# 56，而通知中心报 53——同一用户同一屏看到两个「未读私信」。现三处统一为
# 主收件箱口径（unread + location=1 + folder IS NULL）。
s, r = call("POST", "/messages/boxes", {"name": "AK 分流夹"}, token=mt)
bid = (r.get("data") or {}).get("id")
mv = call("POST", "/messages/move", {"ids": [rows[0]["id"]], "folder": bid},
          token=mt)
ok("N5 能建夹并把一封未读移进去", bool(bid) and mv[1].get("code") == 0,
   "box=%s %s" % (bid, str(mv[1])[:60]))


def three_counts():
    a = (call("GET", "/me", None, token=mt)[1].get("data") or {}).get(
        "unread_messages")
    b = (call("GET", "/me/overview", None, token=mt)[1].get("data") or {}).get(
        "unread_messages")
    rr = call("GET", "/messages/inbox?unread=true", None, token=mt)[1].get(
        "data") or []
    return a, b, len(rr)


mv_me, mv_ov, mv_list = three_counts()
ok("N5 移走一封后三个读数都是 1（角标/卡片/列表同一口径）",
   mv_me == mv_ov == mv_list == 1,
   "me=%s overview=%s list=%s" % (mv_me, mv_ov, mv_list))
call("POST", "/messages/move", {"ids": [rows[0]["id"]], "folder": None},
     token=mt)
call("POST", "/messages/boxes/delete", {"id": bid}, token=mt)
bk_me, bk_ov, bk_list = three_counts()
ok("N5 移回后三数复原为 2 且未留自建夹",
   bk_me == bk_ov == bk_list == 2,
   "me=%s overview=%s list=%s" % (bk_me, bk_ov, bk_list))

print("== N4 收尾 ==")
for stmt in reversed(CLEAN):
    exec_sql(stmt)
ok("N4 探针成员已墓碑化",
   q("SELECT count(*) FROM users WHERE email LIKE '%@ak.local' AND "
     "status < 2") == "0",
   q("SELECT count(*) FROM users WHERE email LIKE '%@ak.local' AND "
     "status < 2"))
ok("N4 探针私信已清空",
   q("SELECT count(*) FROM messages WHERE subject LIKE 'AK 探针信%'") == "0",
   q("SELECT count(*) FROM messages WHERE subject LIKE 'AK 探针信%'"))
summary()
