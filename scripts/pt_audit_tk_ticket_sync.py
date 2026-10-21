# -*- coding: utf-8 -*-
"""闸门 TK：工单 `answered` 标志与 `ticket_status` 必须同步。

缺陷本体（2026-10-21 现网实测，P-492/P-493）：
  - 工单视图「置为已答复/关闭工单」只改 ticket_status，不动 answered；
    用户在「我的工单」确认关闭后，该单 answered 仍为 0 —— 永远赖在
    管理组工作台的「未答复」列表里，谁也清不掉。
  - `staff_answer` 的准入是 `answered = 0`；关闭后重开（3→1）answered
    仍是 1 ⇒ 重开单再也答复不了（现网 400「来信不存在或已答复」），
    工作台「已答复」页也没有答复输入 —— 重开是条死路。
  - 反向：关闭 + answered=0 的历史形状还能被答复并发出私信，
    状态却停在已关闭。

修复口径（保持单一事实源）：
  ticket_status 置 2/3 ⇒ answered=1（2 同时落答复人/时间，与批量标记同义）；
  重开 3→1 ⇒ answered=0（回到待答复工位，现成答复入口即可用）；
  用户确认关闭 ⇒ answered=1；答复口拒绝 ticket_status=3 的单。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_tk_ticket_sync.py
"""
import atexit
import os
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, psql_stdin, summary  # noqa: E402

TAG = str(int(time.time()) % 100000)
UNAME = "tkt%s" % TAG
SUBJ = "TKT-AUDIT-%s" % TAG
A1 = "首答-%s" % TAG
A2 = "二答-%s" % TAG
ROOT = login()
UID = [0]
DONE = [False]


def insert_id(sql):
    """INSERT … RETURNING：-tA 仍会带 `INSERT 0 1` 标签，只取首行。"""
    return int(psql_stdin(sql).split("\n")[0].strip())


def state(tid):
    return psql_stdin(
        "SELECT ticket_status::text || '|' || answered::text || '|' || "
        "COALESCE(answered_by::text,'') || '|' || "
        "(answered_at IS NOT NULL)::text || '|' || "
        "COALESCE(answer,'<NULL>') FROM staffmessages WHERE id=%s" % tid
    ).split("|")


def unanswered_ids():
    s, r = call("GET", "/staffmessages?answered=0", token=ROOT)
    ok("工作台未答复列表可读", s == 200, "%s" % s)
    return [m.get("id") for m in ((r.get("data") or []) if s == 200 else [])]


def cleanup():
    if DONE[0]:
        return
    DONE[0] = True
    psql_stdin("DELETE FROM messages WHERE body LIKE '首答-%s%%' OR "
               "body LIKE '二答-%s%%'" % (TAG, TAG))
    psql_stdin("DELETE FROM staffmessages WHERE subject LIKE "
               "'TKT-AUDIT-%s%%'" % TAG)
    if UID[0]:
        psql_stdin("DELETE FROM message_flood WHERE user_id = %d" % UID[0])
        call("POST", "/admin/users/status",
             {"user_id": UID[0], "status": 2}, token=ROOT)
        call("DELETE", "/admin/users/%d" % UID[0], None, token=ROOT)
    left = psql_stdin("SELECT count(*) FROM staffmessages WHERE subject "
                      "LIKE 'TKT-AUDIT-%s%%'" % TAG)
    ok("TK11 收尾零残留（staffmessages=%s）" % left, left == "0", left)


atexit.register(cleanup)

try:
    # ===== 夹具：临时会员（admin/adduser 带临时密码 ⇒ 先改密才能用业务口）
    s, r = call("POST", "/admin/adduser", {
        "username": UNAME, "email": "%s@tk.local" % UNAME,
        "password": "TkPass123!"}, token=ROOT)
    got = psql_stdin("SELECT id FROM users WHERE username='%s'" % UNAME)
    UID[0] = int(got or 0)
    ok("TK0 造临时会员", s == 200 and UID[0] > 0, "http=%s uid=%s" % (s, got))
    utok = login(UNAME, "TkPass123!")
    call("POST", "/me/password/change", {
        "old_password": "TkPass123!", "new_password": "TkPass456!"},
        token=utok)
    utok = login(UNAME, "TkPass456!")

    t1 = insert_id(
        "INSERT INTO staffmessages (user_id, subject, body) VALUES "
        "(%d, '%s', '正文') RETURNING id" % (UID[0], SUBJ))
    ok("TK0b 夹具工单落地", t1 > 0, "id=%s" % t1)

    # ===== A. 工单视图置 2：answered 必须跟随（P-492）
    s, _ = call("POST", "/stafftickets/update",
                   {"id": t1, "ticket_status": 2}, token=ROOT)
    st = state(t1)
    ok("TK1 置为已答复 → answered=1 且落答复人/时间",
       s == 200 and st[0] == "2" and st[1] == "1"
       and st[2] == "1" and st[3] == "true", "|".join(st))
    ok("TK2 置为已答复后离开「未答复」列表",
       t1 not in unanswered_ids(), "id=%s" % t1)

    # ===== B. 用户确认关闭：answered 必须保持 1（旧形状会留在待办里）
    s, _ = call("POST", "/me/staffmessages/confirm", {"id": t1},
                   token=utok)
    st = state(t1)
    ok("TK3 用户确认关闭 → 3|answered=1",
       s == 200 and st[0] == "3" and st[1] == "1", "|".join(st))
    ok("TK4 关闭单不再出现在「未答复」列表",
       t1 not in unanswered_ids(), "id=%s" % t1)

    # ===== C. 重开死路：先真实答复并关闭一次，再重开（P-493）。
    # 必须用「答复过（answered=1）」的单：若复用上面 answered=0 的单，
    # 旧代码也能答复，这条判据就永远不会红。
    t3 = insert_id(
        "INSERT INTO staffmessages (user_id, subject, body) VALUES "
        "(%d, '%s-R', '正文') RETURNING id" % (UID[0], SUBJ))
    s, _ = call("POST", "/staffmessages/answer",
                {"id": t3, "answer": A1}, token=ROOT)
    st = state(t3)
    ok("TK4b 真实答复 → 2|answered=1",
       s == 200 and st[0] == "2" and st[1] == "1", "|".join(st))
    s, _ = call("POST", "/me/staffmessages/confirm", {"id": t3},
                token=utok)
    st = state(t3)
    ok("TK4c 确认关闭 → 3|answered=1",
       s == 200 and st[0] == "3" and st[1] == "1", "|".join(st))

    s, _ = call("POST", "/stafftickets/update",
                {"id": t3, "ticket_status": 1}, token=ROOT)
    st = state(t3)
    ok("TK5 重开 3→1 → answered 复位为 0",
       s == 200 and st[0] == "1" and st[1] == "0", "|".join(st))
    ok("TK5b 重开单回到「未答复」列表（现成答复入口可达）",
       t3 in unanswered_ids(), "id=%s" % t3)

    s, r = call("POST", "/staffmessages/answer",
                {"id": t3, "answer": A2}, token=ROOT)
    st = state(t3)
    n = psql_stdin("SELECT count(*) FROM messages WHERE receiver_id=%d "
                   "AND body='%s'" % (UID[0], A2))
    ok("TK6 重开后可再次答复 → 2|answered=1|新答复回写",
       s == 200 and st[0] == "2" and st[1] == "1" and st[4] == A2,
       "http=%s %s" % (s, "|".join(st)))
    ok("TK6b 重开后的答复发出私信", n == "1", "n=%s" % n)

    s, r = call("GET", "/me/staffmessages", token=utok)
    mine = [x for x in ((r.get("data") or []) if s == 200 else [])
            if x.get("id") == t3]
    ok("TK7 用户侧读到第二次答复",
       bool(mine) and mine[0].get("ticket_status") == 2
       and mine[0].get("answer") == A2, "mine=%s" % (mine[:1],))
    s, _ = call("POST", "/me/staffmessages/confirm", {"id": t3},
                token=utok)
    st = state(t3)
    ok("TK7b 第二次确认关闭 → 3|answered=1",
       s == 200 and st[0] == "3" and st[1] == "1", "|".join(st))

    # ===== D. 反向钉：已关闭单不可答复（旧形状 answered=0 也不许）
    t2 = insert_id(
        "INSERT INTO staffmessages (user_id, subject, body, ticket_status, "
        "answered) VALUES (%d, '%s-L', '正文', 3, 0) RETURNING id"
        % (UID[0], SUBJ))
    s, r = call("POST", "/staffmessages/answer",
                {"id": t2, "answer": A1}, token=ROOT)
    ok("TK8 已关闭单答复被拒（400/1002）",
       s == 400 and r.get("code") == 1002, "http=%s" % s)

    # ===== E. 只改优先级/指派不得动 answered
    s, _ = call("POST", "/stafftickets/update",
                {"id": t2, "priority": 2}, token=ROOT)
    st = state(t2)
    ok("TK9 只改优先级时 answered 原样不动",
       s == 200 and st[0] == "3" and st[1] == "0", "|".join(st))

    # ===== F. 状态机本身不回退（关闭单不许直接回 0）
    s, r = call("POST", "/stafftickets/update",
                {"id": t3, "ticket_status": 0}, token=ROOT)
    ok("TK10 关闭单回流 0 仍被拒（400/1002）",
       s == 400 and r.get("code") == 1002, "http=%s" % s)
finally:
    cleanup()

summary()
