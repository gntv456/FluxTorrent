# -*- coding: utf-8 -*-
"""闸门 BB：举报与处置链（批三十八）。

举报是版主每天要用的口，也是成员「被欺负之后唯一的路」，此前**零常驻闸门
覆盖**（只有 0101 时代的 e2e 脚本碰过，而那些脚本早已跑不起来）。这条链横跨
三个面：成员写口（/reports、/subtitles/{id}/report）、版主队列
（/admin/reports）、处置与回信（resolve → 私信举报人 → 字幕奖惩动账）。

判据一律「真发请求 → 真读库 + 真读举报人收件箱」。SQL 字面量全用 ASCII
（中文只从库里**读**出来比对，不往 psql 参数里塞），中文理由通过 API JSON 提交。
  S0 地基    两个探针成员（举报人 / 被举报人兼字幕上传者）+ 探针评论 +
             探针字幕（一条正常、一条**已软删**）+ 现成 topic
  S1 写口卫生  非法 ref_type / 空理由 / 目标不存在 / 未过审种 / 重复举报去重；
             **理由长度按「字」而不是按字节**（文案写 1-500 字，200 个汉字
             实际是 600 字节 ⇒ 现网当场 400，中文用户只能写 166 字）
  S2 限流    同一成员对不同目标连发 12 条 ⇒ 队列洪水无拦截；正控换**另一个号**
             证档不是把所有人一起堵死
  S3 队列    成员读版主队列必须被拒；非法 status 400；pending ⊆ all；
             **ref_label 必须落在写口校验的同一 ID 空间**（forum 写口验
             topics，队列却按 posts 取正文 ⇒ 版主看到空或张冠李戴）
  S4 认领处置 claim 互斥、重复认领 400、备注长度上限、resolve 后
             **举报人收件箱真能读到那条结果**（PM 插入此前被 `let _ =` 吞，
             写失败版主毫无察觉）、重复处置 400 且状态机真的走到 1
  S5 字幕奖惩 act → 举报者 +50 / 上传者 −100 / 字幕软删三件都落，
             dismiss 不动账但同样回信；重复处置不二次动账
  S6 半写    对已软删字幕点「举报坏字幕」：INSERT 与 UPDATE 不在同一事务 ⇒
             404 之后留下孤儿举报行
  S7 收尾    探针 reports/messages/comments/subtitles/流水 全清、探针号墓碑、
             未动任何站点共享设定

跑法（仓库根目录，需本地栈在跑）：
    PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bb_reports.py
"""
import os
import re
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (dig,   # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
N1 = "bb%d" % TAG        # 举报人
N2 = "bb%da" % TAG       # 被举报人 + 字幕上传者
REP_IDS = []
MSG_IDS = []
CMT_IDS = []
SUB_IDS = []
LED_IDS = []
UID1 = None
UID2 = None


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % (r.stderr or "")[:200])
    return r.stdout.strip()


def note(msg):
    print("   %s" % (msg,))


def clear_rl(pattern):
    """清掉本闸自己打出来的限流计数（只碰点名的那族键）。S2 的洪水判据要
    一个干净的窗口才能说「第 n 次才被拦」，而 S3 之后还要继续正常举报——
    不清就整节红在限速上（AV 同一手法）。"""
    cmd = ('redis-cli -a "$REDIS_PASSWORD" --no-auth-warning --scan '
           '--pattern "%s" | xargs -r redis-cli -a "$REDIS_PASSWORD" '
           '--no-auth-warning del' % pattern)
    subprocess.run(["docker", "exec", "flux-redis", "sh", "-c", cmd],
                   capture_output=True, text=True, encoding="utf-8")


def ids_of(csv):
    return [int(x) for x in csv.split() if x.strip().isdigit()]


def cleanup():
    psql("DELETE FROM reports WHERE id IN (%s)"
         % (",".join(map(str, REP_IDS)) or "-1"))
    psql("DELETE FROM messages WHERE id IN (%s)"
         % (",".join(map(str, MSG_IDS)) or "-1"))
    psql("DELETE FROM spark_ledger WHERE id IN (%s)"
         % (",".join(map(str, LED_IDS)) or "-1"))
    psql("DELETE FROM comments WHERE id IN (%s)"
         % (",".join(map(str, CMT_IDS)) or "-1"))
    psql("DELETE FROM subtitles WHERE id IN (%s)"
         % (",".join(map(str, SUB_IDS)) or "-1"))
    psql("UPDATE users SET status = 2 WHERE id IN (%s)"
         % (",".join([str(x) for x in (UID1, UID2) if x]) or "-1"))
    left = psql("SELECT count(*) FROM reports WHERE id IN (%s)"
                % (",".join(map(str, REP_IDS)) or "-1"))
    note("[cleanup] 探针 reports 残留 %s/%d；探针 %s/%s 已墓碑；评论 %d /"
         " 字幕 %d / 私信 %d / 流水 %d 已删"
         % (left, len(REP_IDS), UID1, UID2, len(CMT_IDS), len(SUB_IDS),
            len(MSG_IDS), len(LED_IDS)))


import atexit  # noqa: E402
atexit.register(cleanup)


def register(name):
    clear_register_rl()
    time.sleep(6)
    codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                   token=ROOT)[1].get("data") or {}).get("codes")) or []
    if not codes:
        return None
    code, cid = codes[0]["code"], codes[0]["id"]
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = [int(x) for x in re.findall(r"-?\d+", str(cap.get("question")))]
    call("POST", "/auth/register", {
        "username": name, "email": "%s@bb.local" % name,
        "password": "password123", "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": (nums[0] + nums[1]) if len(nums) > 1 else 1,
        "captcha_token": "", "fields": {}})
    uid = psql("SELECT id FROM users WHERE username = '%s'" % name)
    psql("DELETE FROM invites WHERE id = %d" % cid)
    return int(uid) if uid.isdigit() else None


def inbox(token):
    s, r = call("GET", "/messages/inbox", token=token)
    return (r or {}).get("data") or []


print("== S0 地基：两个探针成员 + 探针评论/字幕 ==")
UID1, UID2 = register(N1), register(N2)
if not (UID1 and UID2):
    ok("探针成员注册成功（uid=%s/%s）——地基不成立就不往下量" % (UID1, UID2),
       False, "注册拿不到号，后续判据全部作废")
    sys.exit(1)
s, rr = call("POST", "/auth/login", {"username": N1,
                                     "password": "password123"})
T1 = ((rr or {}).get("data") or {}).get("token")
s, rr2 = call("POST", "/auth/login", {"username": N2,
                                      "password": "password123"})
T2 = ((rr2 or {}).get("data") or {}).get("token")
TID = psql("SELECT id FROM torrents WHERE approval_status = 1 ORDER BY id"
           " DESC LIMIT 1")
# 选一条**同 id 也有 post** 的 topic：这样队列若走错 ID 空间，取到的是
# 另一条真实正文（张冠李戴），而不是恰好 NULL
TOPIC = psql("SELECT t.id FROM topics t JOIN posts p ON p.id = t.id"
             " ORDER BY t.id LIMIT 1")
if not TOPIC.isdigit():
    # 没有「同 id 也有 post」的 topic 时退而取任意一条：那种情况下队列走错
    # ID 空间会取到 NULL，同样是 P-127 的现场
    TOPIC = psql("SELECT id FROM topics ORDER BY id LIMIT 1")
TOPIC_NAME = psql("SELECT title FROM topics WHERE id = %s" % TOPIC)
POST_BODY = psql("SELECT body FROM posts WHERE id = %s" % TOPIC)
CMT = psql("INSERT INTO comments (torrent_id, user_id, body)"
           " VALUES (%s, %s, 'BB-probe-comment-%s') RETURNING id"
           % (TID, UID1, TAG))
SUB_OK = psql("INSERT INTO subtitles (torrent_id, user_id, title, file_ref,"
              " lang_id, created_at, status, verified, bad_reports)"
              " VALUES (%s, %s, 'bb-ok-%s', 'bb-probe.srt', 1, now(), 1,"
              " true, 0) RETURNING id" % (TID, UID2, TAG))
SUB_DEL = psql("INSERT INTO subtitles (torrent_id, user_id, title, file_ref,"
               " lang_id, created_at, status, verified, bad_reports,"
               " deleted_at) VALUES (%s, %s, 'bb-del-%s', 'bb-probe.srt', 1,"
               " now(), 1, true, 0, now()) RETURNING id" % (TID, UID2, TAG))
CMT_IDS += ids_of(CMT)
SUB_IDS += ids_of(SUB_OK) + ids_of(SUB_DEL)
psql("UPDATE users SET spark_balance = 500 WHERE id = %s" % UID2)
ok("探针成员/评论/字幕/topic 就位（uid=%s/%s torrent=%s topic=%s 字幕=%s/%s）"
   % (UID1, UID2, TID, TOPIC, SUB_OK, SUB_DEL),
   bool(UID1) and bool(UID2) and T1 and T2 and CMT.isdigit()
   and SUB_OK.isdigit() and SUB_DEL.isdigit() and TOPIC.isdigit(),
   "t1=%s t2=%s topic=%s cmt=%s" % (bool(T1), bool(T2), TOPIC, CMT[:40]))
if not (UID1 and UID2 and T1 and T2):
    sys.exit(1)
note("[read-only] 站点设定一个字都不写；topic#%s 标题=%r，同 id 的 post 正文"
     "=%r" % (TOPIC, TOPIC_NAME[:24], (POST_BODY or "")[:24]))

print("== S1 写口卫生（POST /reports）==")
s, r = call("POST", "/reports", {"ref_type": "planet", "ref_id": 1,
                                 "reason": "BB-probe"}, token=T1)
msg = str((r or {}).get("message", ""))
ok("S1 非法 ref_type 必须 400 且点到「举报对象」（不许写成 != 200）",
   s == 400 and "举报对象" in msg, "%s %s" % (s, msg[:70]))
s, r = call("POST", "/reports", {"ref_type": "torrent", "ref_id": int(TID),
                                 "reason": "   "}, token=T1)
ok("S1 空理由必须 400 且点到「理由」",
   s == 400 and "理由" in str((r or {}).get("message", "")),
   "%s %s" % (s, str((r or {}).get("message", ""))[:70]))
zh = "汉字" * 200
s, r = call("POST", "/reports", {"ref_type": "torrent", "ref_id": int(TID),
                                 "reason": zh}, token=T1)
rid = psql("SELECT max(id) FROM reports WHERE reporter_id = %s AND"
           " ref_type = 'torrent'" % UID1)
REP_IDS += ids_of(rid)
ok("S1 200 个汉字（=600 字节）的理由必须**收下**：文案写「1-500 字」而代码"
   "数 str::len() 的字节 ⇒ 中文实际只配写 166 字（P-128）",
   s == 200 and (r or {}).get("code") == 0,
   "%s %s" % (s, str((r or {}).get("message", ""))[:90]))
s, r = call("POST", "/reports", {"ref_type": "torrent", "ref_id": 99999999,
                                 "reason": "BB-probe"}, token=T1)
ok("S1 目标不存在必须 404（code=1004/NotFound，不是 200）",
   s == 404 and (r or {}).get("code") != 0, "%s %s" % (s, str(r)[:70]))
un = psql("SELECT id FROM torrents WHERE approval_status <> 1 ORDER BY id"
          " DESC LIMIT 1")
if un.isdigit():
    s, r = call("POST", "/reports", {"ref_type": "torrent",
                                     "ref_id": int(un), "reason": "BB-unpub"},
                token=T1)
    ok("S1 未过审种 #%s 也要拒（举报口不替看不见的种开工单）" % un,
       (r or {}).get("code") != 0, "%s %s" % (s, str(r)[:70]))
s, r = call("POST", "/reports", {"ref_type": "comment", "ref_id": int(CMT),
                                 "reason": "BB-cmt-1"}, token=T1)
first = (r or {}).get("data") or {}
CMT_RID = int(first.get("id") or 0)
s, r = call("POST", "/reports", {"ref_type": "comment", "ref_id": int(CMT),
                                 "reason": "BB-cmt-2"}, token=T1)
dup = (r or {}).get("data") or {}
REP_IDS += ids_of(str(first.get("id") or ""))
n_open = int(psql("SELECT count(*) FROM reports WHERE ref_type = 'comment'"
                  " AND ref_id = %s AND status IN (0,2)" % CMT))
ok("S1 同一目标重复举报：第一次落 id、第二次标 duplicate 且队列仍 1 行"
   "（实测 %d 行）" % n_open,
   bool(first.get("id")) and dup.get("duplicate") is True and n_open == 1,
   "%s %s rows=%d" % (first, dup, n_open))

print("== S2 限流：一个人灌队列 ==")
storm = []
for i in range(12):
    cid = psql("INSERT INTO comments (torrent_id, user_id, body)"
               " VALUES (%s, %s, 'BB-flood-%s-%s') RETURNING id"
               % (TID, UID1, TAG, i))
    CMT_IDS += ids_of(cid)
    s, r = call("POST", "/reports", {"ref_type": "comment", "ref_id": int(cid),
                                     "reason": "BB-flood %d" % i}, token=T1)
    storm.append(s)
REP_IDS += ids_of(psql("SELECT id FROM reports WHERE reason LIKE"
                       " 'BB-flood%'"))
ok("S2 连发 12 条必须撞上限流档（红线：全 200 ⇒ 版主队列可被一人灌满，"
   "P-130）实测 %s" % (storm,), storm.count(429) >= 2 and storm[-1] == 429,
   "%s" % (storm,))
s, r = call("POST", "/reports", {"ref_type": "user", "ref_id": int(UID2),
                                 "reason": "BB-user-probe"}, token=T2)
rid2 = psql("SELECT max(id) FROM reports WHERE reason = 'BB-user-probe'")
REP_IDS += ids_of(rid2)
ok("S2 正控：洪水只冻住**那个号**——换一个成员的第一条照样 200"
   "（红线写法：把档做成全局，别人举报无门）",
   (r or {}).get("code") == 0, "%s %s" % (s, str(r)[:80]))
s2, r2 = call("POST", "/reports", {"ref_type": "subtitle",
                                   "ref_id": int(SUB_OK),
                                   "reason": "BB-sub-axis"}, token=None)
ok("S2 正控：未登录打举报口必须 401（限流不是把匿名也放行）",
   s2 == 401 and (r2 or {}).get("code") != 0, "%s %s" % (s2, str(r2)[:70]))
# 后面各节还要正常举报：清掉本轮自己打出来的 rep 计数（只碰这一族键）
clear_rl("rl:rep:%d:*" % UID1)
clear_rl("rl:rep:%d:*" % UID2)
# 被举报人自己那条由 UID2 发的也要能收尾
rid2b = psql("SELECT max(id) FROM reports WHERE ref_type = 'user' AND"
             " ref_id = %s" % UID2)
REP_IDS += ids_of(rid2b)

print("== S3 队列权限与 ref_label ==")
s, r = call("GET", "/admin/reports?status=pending", token=T1)
ok("S3 成员读版主队列必须被拒且给出权限判据（401/403 + code≠0，"
   "不是 200 空数组）", s in (401, 403) and (r or {}).get("code") not in
   (0, None), "%s %s" % (s, str(r)[:80]))
s, r = call("GET", "/admin/reports?status=bogus", token=ROOT)
ok("S3 非法 status 必须 400 且点到四档",
   s == 400 and "pending" in str((r or {}).get("message", "")),
   "%s %s" % (s, str((r or {}).get("message", ""))[:80]))
s, q = call("GET", "/admin/reports?status=pending", token=ROOT)
rows = (q or {}).get("data") or []
s, q2 = call("GET", "/admin/reports?status=all", token=ROOT)
allrows = (q2 or {}).get("data") or []
pend = {x["id"] for x in rows}
ok("S3 队列可读（pending %d 条）且 all ⊇ pending（%d 条）"
   % (len(rows), len(allrows)), bool(pend) and len(allrows) >= len(pend),
   "%d %d" % (len(rows), len(allrows)))
s, r = call("POST", "/reports", {"ref_type": "forum", "ref_id": int(TOPIC),
                                 "reason": "BB-forum"}, token=T1)
frid = ((r or {}).get("data") or {}).get("id")
REP_IDS += ids_of(str(frid or ""))
s, q3 = call("GET", "/admin/reports?status=all", token=ROOT)
qrows = (q3 or {}).get("data") or []
lab = [x.get("ref_label") for x in qrows
       if x.get("ref_type") == "forum" and x.get("ref_id") == int(TOPIC)]
got_lab = lab[0] if lab else None
ok("S3 forum 的 ref_label 必须等于被举报 **topic** 的标题（实测 %r；topic"
   "=%r / 同 id 的 post 正文=%r）——写口验 topics、队列读 posts 是两个 ID"
   " 空间（P-127）" % (got_lab, TOPIC_NAME[:20], (POST_BODY or "")[:20]),
   got_lab == TOPIC_NAME, "%r != %r" % (got_lab, TOPIC_NAME))
ulab = [x.get("ref_label") for x in qrows
        if x.get("ref_type") == "user" and x.get("ref_id") == int(UID2)]
ok("S3 user 的 ref_label 是被举报人的用户名（实测 %r）" % (ulab,),
   bool(ulab) and ulab[0] == N2, "%s" % (ulab,))

print("== S4 认领 → 处置 → 回信 ==")
target = int(frid or 0)
s, r = call("POST", "/admin/reports/claim", {"report_id": target}, token=ROOT)
ok("S4 认领成功（200 + claimed）", s == 200 and (r or {}).get("code") == 0,
   "%s %s" % (s, str(r)[:80]))
s, r = call("POST", "/admin/reports/claim", {"report_id": target}, token=ROOT)
ok("S4 重复认领必须 400 且点到「已被认领」",
   s == 400 and "已被认领" in str((r or {}).get("message", "")),
   "%s %s" % (s, str((r or {}).get("message", ""))[:70]))
st = psql("SELECT status || '/' || COALESCE(claimed_by::text,'-') FROM reports"
          " WHERE id = %d" % target)
ok("S4 认领是状态机变更（status=2 + claimed_by 落库，实测 %s）" % st,
   st.startswith("2/"), "%s" % st)
s, r = call("POST", "/admin/reports/resolve", {"report_id": target,
                                               "action": "act",
                                               "note": "x" * 5000},
            token=ROOT)
ok("S4 处置备注超长必须 400 且点到「备注」长度上限（红线：原文不筛直接进"
   "私信正文，一条 5000 字的备注把举报人私信撑满，P-133）",
   s == 400 and "备注" in str((r or {}).get("message", "")),
   "%s %s" % (s, str((r or {}).get("message", ""))[:80]))
ok("S4 超长备注那条**没有**被当成已结案（status 仍是 2）",
   psql("SELECT status FROM reports WHERE id = %d" % target) == "2")
s, r = call("POST", "/admin/reports/resolve", {"report_id": target,
                                               "action": "act",
                                               "note": "BB-note-ok"},
            token=ROOT)
ok("S4 处置成功（200 + resolved + pm_sent=true）",
   s == 200 and (r or {}).get("code") == 0
   and dig(r or {}, "data", "pm_sent") is True,
   "%s %s" % (s, str(r)[:80]))
items = inbox(T1)
got = [m for m in items if "举报" in (m.get("subject") or "")
       and "BB-note-ok" in (m.get("body") or "")]
MSG_IDS += ids_of(str(got[0]["id"]) if got else "")
ok("S4 举报人**收件箱真能读到**这条结果且带管理备注（读到 %d 条）——PM"
   " 插入此前是 `let _ =`，写失败版主毫无察觉（P-135）" % len(got),
   bool(got) and got[0].get("unread") is True, "%d/%d 封"
   % (len(got), len(items)))
s, r = call("POST", "/admin/reports/resolve", {"report_id": target},
            token=ROOT)
ok("S4 重复处置必须 400 且点到「已处理」",
   s == 400 and "已处理" in str((r or {}).get("message", "")),
   "%s %s" % (s, str((r or {}).get("message", ""))[:70]))
ok("S4 结案后 status=1 且 handled_by/handled_at 落库",
   psql("SELECT status || '/' || COALESCE(handled_by::text,'-') || '/' ||"
        " (handled_at IS NOT NULL)::int FROM reports WHERE id = %d"
        % target) == "1/1/1")

print("== S5 字幕举报奖惩闭环 ==")
sub_rep = int(psql("SELECT max(id) FROM reports WHERE reason ="
                   " 'BB-sub-axis'") or 0)
if sub_rep == 0:
    # 匿名被拒那条不会建行，改用登录态重发一次（S2 的正控不产生行）
    s, r = call("POST", "/reports", {"ref_type": "subtitle",
                                     "ref_id": int(SUB_OK),
                                     "reason": "BB-sub-axis"}, token=T1)
    sub_rep = int(((r or {}).get("data") or {}).get("id") or 0)
REP_IDS.append(sub_rep)
bal1 = int(psql("SELECT spark_balance FROM users WHERE id = %s" % UID1))
bal2 = int(psql("SELECT spark_balance FROM users WHERE id = %s" % UID2))
s, r = call("POST", "/admin/reports/resolve", {"report_id": sub_rep,
                                               "action": "act"}, token=ROOT)
ok("S5 字幕举报成立：处置 200", s == 200 and (r or {}).get("code") == 0,
   "%s %s" % (s, str(r)[:90]))
got1 = int(psql("SELECT spark_balance FROM users WHERE id = %s" % UID1))
got2 = int(psql("SELECT spark_balance FROM users WHERE id = %s" % UID2))
led1 = psql("SELECT count(*) FROM spark_ledger WHERE user_id = %s AND"
            " kind = 'subtitle_report'" % UID1)
led2 = psql("SELECT count(*) FROM spark_ledger WHERE user_id = %s AND"
            " kind = 'subtitle_violation'" % UID2)
LED_IDS += ids_of(psql("SELECT id FROM spark_ledger WHERE user_id IN (%s,%s)"
                       " AND kind IN ('subtitle_report',"
                       " 'subtitle_violation')" % (UID1, UID2)))
ok("S5 举报者 +50（%d→%d）、上传者 −100（%d→%d）、两条流水各 1 条"
   % (bal1, got1, bal2, got2),
   got1 - bal1 == 50 and bal2 - got2 == 100 and led1 == "1" and led2 == "1",
   "up=%d/%d dn=%d/%d led=%s/%s" % (bal1, got1, bal2, got2, led1, led2))
ok("S5 字幕被软删且记 moderator",
   psql("SELECT (deleted_at IS NOT NULL)::int || '/' ||"
        " COALESCE(moderated_by::text,'-') FROM subtitles WHERE id = %s"
        % SUB_OK) == "1/1")
s, r = call("POST", "/admin/reports/resolve", {"report_id": sub_rep,
                                               "action": "act"}, token=ROOT)
again = psql("SELECT count(*) FROM spark_ledger WHERE user_id = %s AND"
             " kind = 'subtitle_report'" % UID1)
ok("S5 重复处置被拒（http=%s）且不二次动账（流水仍 %s 条）" % (s, again),
   s == 400 and again == "1", "%s %s" % (s, again))
dm = CMT_RID
b1 = int(psql("SELECT spark_balance FROM users WHERE id = %s" % UID1))
s, r = call("POST", "/admin/reports/resolve", {"report_id": dm,
                                               "action": "dismiss"},
            token=ROOT)
b2 = int(psql("SELECT spark_balance FROM users WHERE id = %s" % UID1))
ok("S5 dismiss 不动账（%d→%d）但仍回信" % (b1, b2),
   b1 == b2 and s == 200 and (r or {}).get("code") == 0
   and dig(r or {}, "data", "pm_sent") is True,
   "%s %s" % (s, str(r)[:70]))
items2 = inbox(T1)
got2m = [m for m in items2 if "举报" in (m.get("subject") or "")
         and "驳回" in (m.get("body") or "")]
MSG_IDS += ids_of(str(got2m[0]["id"]) if got2m else "")
ok("S5 dismiss 的回信同样进了收件箱（读到 %d 条）" % len(got2m),
   bool(got2m), "%s" % ([m.get("subject") for m in got2m],))

print("== S6 已软删字幕：两条写口都不许留半笔 ==")
bad0 = int(psql("SELECT count(*) FROM reports WHERE ref_type = 'subtitle'"
                " AND ref_id = %s" % SUB_DEL))
s, r = call("POST", "/subtitles/%s/report" % SUB_DEL, {}, token=T1)
bad1 = int(psql("SELECT count(*) FROM reports WHERE ref_type = 'subtitle'"
                " AND ref_id = %s" % SUB_DEL))
REP_IDS += ids_of(psql("SELECT id FROM reports WHERE ref_type = 'subtitle'"
                       " AND ref_id = %s" % SUB_DEL))
br = psql("SELECT bad_reports FROM subtitles WHERE id = %s" % SUB_DEL)
ok("S6 对已软删字幕点举报必须整笔不做：拒掉之后**不许留下孤儿举报行**"
   "（实测新增 %d 行、bad_reports=%s）——INSERT 与 UPDATE 不同事务（P-131）"
   % (bad1 - bad0, br), (s == 404 or (r or {}).get("code") != 0)
   and bad1 == bad0, "http=%s rows %d->%d code=%s"
   % (s, bad0, bad1, (r or {}).get("code")))
s, r = call("POST", "/reports", {"ref_type": "subtitle",
                                 "ref_id": int(SUB_DEL),
                                 "reason": "BB-del-sub"}, token=T1)
bad2 = int(psql("SELECT count(*) FROM reports WHERE ref_type = 'subtitle'"
                " AND ref_id = %s" % SUB_DEL))
ok("S6 另一条写口 /reports 也不该给已软删字幕开工单（实测新增 %d 行）"
   % (bad2 - bad1), (r or {}).get("code") != 0 and bad2 == bad1,
   "http=%s rows %d->%d" % (s, bad1, bad2))

print("== S7 收尾 ==")
# 删除必须在断言**之前**显式做完：放在 atexit 的 cleanup 里数就永远「还没清」，
# 上一版这两条红是自己造的（假红，不是站点残料）
for sql in (
    "DELETE FROM messages WHERE id IN (%s)"
    % (",".join(map(str, MSG_IDS)) or "-1"),
    "DELETE FROM spark_ledger WHERE id IN (%s)"
    % (",".join(map(str, LED_IDS)) or "-1"),
    "DELETE FROM comments WHERE id IN (%s)"
    % (",".join(map(str, CMT_IDS)) or "-1"),
    "DELETE FROM subtitles WHERE id IN (%s)"
    % (",".join(map(str, SUB_IDS)) or "-1"),
    "UPDATE users SET status = 2 WHERE id IN (%s)"
    % (",".join([str(x) for x in (UID1, UID2) if x]) or "-1"),
):
    psql(sql)
psql("DELETE FROM reports WHERE id IN (%s)"
     % (",".join(map(str, REP_IDS)) or "-1"))
ok("S7 探针 reports 清零（造过 %d 条）" % len(REP_IDS),
   psql("SELECT count(*) FROM reports WHERE id IN (%s)"
        % (",".join(map(str, REP_IDS)) or "-1")) == "0")
ok("S7 探针私信/评论/字幕/流水不留残料",
   psql("SELECT count(*) FROM messages WHERE id IN (%s)"
        % (",".join(map(str, MSG_IDS)) or "-1")) == "0"
   and psql("SELECT count(*) FROM subtitles WHERE id IN (%s)"
            % (",".join(map(str, SUB_IDS)) or "-1")) == "0"
   and psql("SELECT count(*) FROM comments WHERE id IN (%s)"
            % (",".join(map(str, CMT_IDS)) or "-1")) == "0"
   and psql("SELECT count(*) FROM spark_ledger WHERE id IN (%s)"
            % (",".join(map(str, LED_IDS)) or "-1")) == "0")
ok("S7 探针号已墓碑（%s=%s / %s=%s）"
   % (N1, psql("SELECT status FROM users WHERE id = %s" % UID1),
      N2, psql("SELECT status FROM users WHERE id = %s" % UID2)),
   psql("SELECT status FROM users WHERE id = %s" % UID1) == "2"
   and psql("SELECT status FROM users WHERE id = %s" % UID2) == "2")
summary()
