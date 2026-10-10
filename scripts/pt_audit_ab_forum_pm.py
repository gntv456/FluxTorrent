# -*- coding: utf-8 -*-
"""闸门 AB：私信 / 好友 / 论坛回帖通知（2026-10-10 批二实测固化）。

钉住的四处断链：
  E1 主收件箱把已归档进自定义文件夹的信也列出来（缺省应 = folder IS NULL）；
  E2 界面「拒绝」好友申请发的其实是 POST /friends，把对方转成好友（接受/拒绝反转）；
  E3 界面「接受」用的 action=accept 后端不认，一律 400；
  E4 users.notify_topic_reply 是「设置页能改、代码零读」的假开关。

跑法：
    PYTHONIOENCODING=utf-8 python scripts/pt_audit_ab_forum_pm.py
"""
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (call, clear_register_rl, login, ok,
                         summary)  # noqa: E402

ROOT = login()
CLEAN = []
TAG = int(time.time()) % 100000
S_IN = "AB闸门%d入夹" % TAG
S_RE = "AB闸门%d可回" % TAG
S_FR = "AB闸门%d好友也不拒" % TAG
S_NO = "AB闸门%d越权信" % TAG
S_YES = "AB闸门%d放行" % TAG
S_T1 = "AB闸门%d主题" % TAG
SUBJ_OK = "好友申请已通过"
SUBJ_REPLY = "主题收到回复"


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % r.stderr.strip())
    return r.stdout.strip()


def exec_sql(q):
    subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-c", q],
        capture_output=True, text=True, encoding="utf-8")


def raw_friends(tok):
    return call("GET", "/friends", token=tok)[1].get("data") or []


def friend_rows(tok):
    """形状归一后再断言语义：行是元组也要能验 accept/reject 对不对，
    否则一条形状回归会把后面所有判据一起冲掉（读起来像「功能坏了」）。"""
    out = []
    for r in raw_friends(tok):
        if isinstance(r, dict):
            out.append(r)
        elif isinstance(r, (list, tuple)) and len(r) == 2:
            out.append({"username": r[0], "list": r[1]})
    return out


def friend_pairs(x, y):
    """两人之间已是好友的行数（双向各一行）"""
    where = ("list = 'friend' AND ((user_id = %s AND friend_id = %s)"
             " OR (user_id = %s AND friend_id = %s))") % (x, y, y, x)
    return cnt("friendships", where)


def cnt(table, where):
    return int(psql("SELECT count(*) FROM %s WHERE %s" % (table, where)) or 0)


def cleanup():
    for stmt in reversed(CLEAN):
        try:
            exec_sql(stmt)
        except BaseException as e:  # noqa: BLE001
            print("   ! 收尾失败: %s -> %s" % (stmt[:60], str(e)[:70]))


def inbox(tok):
    d = call("GET", "/messages/inbox", token=tok)[1].get("data") or []
    return d if isinstance(d, list) else (d.get("rows") or [])


def new_member(tag):
    """发码 → 验证码 → 注册 → 登录（演示号密码会被随机化，不借）。

    用户名带毫秒后缀：探针号收尾只能 `status=2`（audit_log 防篡改链不许删号），
    名字因此长期占位。用同一 TAG 复跑时旧名还在库里，注册会被拒——
    表现是「成员准备失败」，看着像站点坏了，其实是自己撞自己的历史。
    失败原因直接打出来：这一条断言的判据是「四个号都能准备」，
    光给 True/False 等于没报因。
    """
    def why(msg):
        print("   成员 %s 准备失败：%s" % (tag, msg))
        return None

    r = call("POST", "/admin/invites", {"user_id": 1, "count": 1},
             token=ROOT)[1]
    codes = ((r.get("data") or {}).get("codes")) or []
    if not codes:
        return why("发码 http=%s %s" % (r.get("code"), r.get("message")))
    code, cid = codes[0]["code"], codes[0]["id"]
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if len(nums) < 2:
        return why("验证码题解析失败 %s" % (cap,))
    user = "ab%s%d" % (tag, int(time.time() * 1000) % 10000000)
    reg = {"username": user, "email": "%s@ab.local" % user,
           "password": "password123", "invite_code": code,
           "captcha_id": cap.get("captcha_id"),
           "captcha_answer": int(nums[0]) + int(nums[1]),
           "captcha_token": "", "fields": {}}
    resp = call("POST", "/auth/register", reg)
    data = resp[1].get("data") or {}
    uid = data.get("user_id")
    if not uid:
        return why("注册 http=%s %s" % (resp[0], resp[1].get("message")))
    CLEAN.append("UPDATE users SET status = 2 WHERE id = %s" % uid)
    CLEAN.append("DELETE FROM invites WHERE id = %d" % cid)
    # 连开多个探针号时登录会吃 429；令牌拿不到就重试，
    # 否则后面每个请求都是「未认证」，红的全是假案。
    for _ in range(6):
        login_r = call("POST", "/auth/login",
                       {"username": user, "password": "password123"})[1]
        tok = (login_r.get("data") or {}).get("token")
        if tok:
            return (tok, uid, user)
        time.sleep(4)
    return why("登录 6 次都拿不到 token（多半是 429 限流）")


def send(frm, to_user, subject, text, reply_to=None):
    """发信。60s 防刷是要绕过的机制、不是判据，所以每次先清 flood。"""
    exec_sql("DELETE FROM message_flood WHERE user_id = %s" % frm[1])
    body = {"to": to_user, "subject": subject, "body": text}
    if reply_to:
        body["reply_to"] = reply_to
    return call("POST", "/messages", body, token=frm[0])[1]


# 注册限流按 IP 计数（5/分钟）：几套闸门连跑会互相吃光额度，
# 先清自己造的计数，免得「成员准备失败」被读成站点故障。
clear_register_rl()
A, B = new_member("a"), new_member("b")
C, D = new_member("c"), new_member("d")
if not all([A, B, C, D]):
    ok("四个探针成员可准备", False, "%s%s%s%s" % (bool(A), bool(B), bool(C),
                                                bool(D)))
    summary()
    sys.exit(1)
AT, BT, CT, DT = (m[0] for m in (A, B, C, D))
CLEAN.append("DELETE FROM messages WHERE subject LIKE 'AB闸门%d%%'" % TAG)
CLEAN.append("DELETE FROM pmboxes WHERE user_id = %s" % B[1])
for stmt in [
    "DELETE FROM friendships WHERE user_id IN (%s,%s,%s,%s)",
    "DELETE FROM friendships WHERE friend_id IN (%s,%s,%s,%s)",
    "DELETE FROM message_flood WHERE user_id IN (%s,%s,%s,%s)",
    "DELETE FROM forum_flood WHERE user_id IN (%s,%s,%s,%s)",
]:
    CLEAN.append(stmt % (A[1], B[1], C[1], D[1]))

print("E1 信箱文件夹：移动后主收件箱不该再看见这封")
data = call("POST", "/messages/boxes", {"name": "AB夹"}, token=BT)[1]
bid = (data.get("data") or {}).get("id")
data = send(A, B[2], S_IN, "要被归档的信")
mid = (data.get("data") or {}).get("id")
ok("E1 前置：建夹 + 发信成功", bool(bid) and bool(mid),
   "bid=%s mid=%s" % (bid, mid))
s, r = call("POST", "/messages/move", {"ids": [mid], "folder": bid},
            token=BT)
ok("E1 移动进文件夹 200", s == 200, str(r)[:120])
boxes = call("GET", "/messages/boxes", token=BT)[1].get("data") or []
cnt_box = next((x.get("count") for x in boxes if x.get("id") == bid), None)
ok("E1 文件夹计数 = 1", cnt_box == 1, str(boxes)[:130])
ids = [x.get("id") for x in inbox(BT)]
ok("E1 主收件箱不再列出该信（缺省 = folder IS NULL）", mid not in ids,
   "inbox=%s mid=%s" % (ids, mid))
q = "/messages/inbox?box_id=%s" % bid
shown = call("GET", q, token=BT)[1].get("data") or []
ok("E1 正控：按文件夹查得到这封", any(x.get("id") == mid for x in shown),
   "n=%d" % len(shown))
call("POST", "/messages/move", {"ids": [mid], "folder": None}, token=BT)
ids = [x.get("id") for x in inbox(BT)]
ok("E1 移回主收件箱后又出现", mid in ids, str(ids))

print("E2 回复：主题加 Re:、正文带原信引用")
rid = (send(B, A[2], S_RE, "原信正文").get("data") or {}).get("id")
send(A, B[2], "AB闸门%d回信" % TAG, "回你", reply_to=rid)
subs = [x.get("subject") or "" for x in inbox(BT)]
ok("E2 回复主题带 Re: 前缀",
   any(x.startswith("Re: ") and S_RE in x for x in subs), str(subs)[:140])
ok("E2 正文含原信引用",
   cnt("messages", "sender_id = %s AND body LIKE '%%%%原信%%%%'" % A[1]) >= 1)
send(A, B[2], S_NO + "引用", "x", reply_to=99999999)
ok("E2 正控：reply_to 指向不存在的信不会带走别人的正文",
   cnt("messages", "subject = '%s' AND body LIKE '%%%%原信%%%%'"
       % (S_NO + "引用")) == 0)

print("E3 accept_pm 三态要真的被消费")
call("PUT", "/me/settings", {"accept_pm": "friends"}, token=AT)
r = send(B, A[2], S_NO, "不该送达")
ok("E3 =friends 时非好友被拒", r.get("code") != 0
   and cnt("messages", "subject = '%s'" % S_NO) == 0, str(r)[:120])
call("PUT", "/me/settings", {"accept_pm": "no"}, token=AT)
call("POST", "/friends", {"username": B[2]}, token=AT)
call("POST", "/friends", {"username": A[2]}, token=BT)
ok("E3 前置：两人已是好友",
   cnt("friendships", "user_id = %s AND friend_id = %s AND list = 'friend'"
       % (A[1], B[1])) == 1)
r = send(B, A[2], S_FR, "好友也不该送达")
ok("E3 =no 时连好友也拒", r.get("code") != 0, str(r)[:120])
call("PUT", "/me/settings", {"accept_pm": "yes"}, token=AT)
send(B, A[2], S_YES, "现在该送达")
ok("E3 =yes 时放行", cnt("messages", "subject = '%s'" % S_YES) == 1)

print("E4 好友申请：接受与拒绝必须是两件事")
call("POST", "/friends", {"username": C[2]}, token=DT)
rows = raw_friends(CT)
# 形状断言：必须是 {username, list} 对象（元组数组会让两个消费端读到空）
ok("E4 /friends 行形状是对象", bool(rows) and isinstance(rows[0], dict)
   and "username" in rows[0] and "list" in rows[0], str(rows)[:140])
ok("E4 前置：C 看到 incoming 申请",
   any(r.get("list") == "incoming" for r in friend_rows(CT)), str(rows)[:140])
# 先拒：必须只是撤掉申请，绝不能把对方变成好友
s, r = call("POST", "/friends/action", {"username": D[2], "action": "reject"},
            token=CT)
ok("E4 「拒绝」返回 200", s == 200, str(r)[:130])
ok("E4 正控：拒绝没把申请人变好友（曾经的反转 bug）",
   friend_pairs(C[1], D[1]) == 0)
ok("E4 拒绝后 pending 申请消失",
   cnt("friendships", "user_id = %s AND friend_id = %s AND list = 'pending'"
       % (D[1], C[1])) == 0)
s, r = call("POST", "/friends/action", {"username": D[2], "action": "accept"},
            token=CT)
ok("E4 无申请可处理时 accept 明确报错",
   s == 400 and "没有待处理的好友申请" in (r.get("message") or ""), str(r)[:130])
# 再申请一次并接受
call("POST", "/friends", {"username": C[2]}, token=DT)
s, r = call("POST", "/friends/action", {"username": D[2], "action": "accept"},
            token=CT)
ok("E4 界面「接受」用的 action=accept 被后端接受", s == 200, str(r)[:130])
ok("E4 接受后双向成为好友", friend_pairs(C[1], D[1]) == 2)
ok("E4 接受会给对方发系统信",
   cnt("messages", "receiver_id = %s AND subject = '%s'"
       % (D[1], SUBJ_OK)) == 1)

print("E5 论坛回帖通知 + notify_topic_reply 偏好闸门")
boards = call("GET", "/forums/boards", token=AT)[1].get("data") or []
fid = boards[0]["id"] if boards else None
r = call("POST", "/forums/topics", {"forum_id": fid, "title": S_T1,
                                   "body": "主题正文"}, token=AT)[1]
d = r.get("data") or {}
tid = d.get("topic_id") or d.get("id")
ok("E5 前置：能发帖", bool(fid) and bool(tid), str(r)[:130])
if tid:
    CLEAN.append("DELETE FROM posts WHERE topic_id = %s" % tid)
    CLEAN.append("DELETE FROM topics WHERE id = %s" % tid)
    exec_sql("DELETE FROM forum_flood WHERE user_id = %s" % B[1])
    s, r = call("POST", "/forums/topics/%s/reply" % tid, {"body": "回帖一"},
               token=BT)
    ok("E5 回帖 200", s == 200, str(r)[:120])
    first = cnt("messages", "receiver_id = %s AND subject = '%s'"
                % (A[1], SUBJ_REPLY))
    ok("E5 楼主收到「主题收到回复」", first == 1, "n=%d" % first)
    call("PUT", "/me/settings", {"notify_topic_reply": False}, token=AT)
    exec_sql("DELETE FROM forum_flood WHERE user_id = %s" % C[1])
    s, r = call("POST", "/forums/topics/%s/reply" % tid, {"body": "回帖二"},
               token=CT)
    ok("E5 正控：第二次回帖本身成功", s == 200, str(r)[:120])
    after = cnt("messages", "receiver_id = %s AND subject = '%s'"
                % (A[1], SUBJ_REPLY))
    ok("E5 关掉偏好后不再发通知（假开关已接上）", after == first,
       "before=%d after=%d" % (first, after))
    call("PUT", "/me/settings", {"notify_topic_reply": True}, token=AT)
    exec_sql("DELETE FROM forum_flood WHERE user_id = %s" % D[1])
    call("POST", "/forums/topics/%s/reply" % tid, {"body": "回帖三"}, token=DT)
    ok("E5 正控：改回偏好后又能收到（闸门不是恒关）",
       cnt("messages", "receiver_id = %s AND subject = '%s'"
           % (A[1], SUBJ_REPLY)) > after)

try:
    summary()
finally:
    cleanup()
