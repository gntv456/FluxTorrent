"""受限版块读口闸门（TX）。

不变量：版块的 `minclassread` 必须**同时**挡住「按版块列主题」「主题详情」
「全站搜索」「最近主题」「侧栏热帖榜/最新回复榜」——只要有一个旁路口不判
读档，受限内容就会从搜索/聚合面漏出去（版主配置受限版块时以为
「已经关起来了」）。

现网原本没有任何 minclassread>1 的版块，这一案造不出对象，所以本闸门自带
夹具、自带清理：**建一个 read=5 的版块 + 一条只存在于它的主题（标题带本轮
唯一 ASCII 短语）→ 会员与职员各打一遍 → 删回去并回读**。职员必须看得见
（正控，否则「会员看不见」可能是「这条口本来不出货」的假绿），会员必须看不见。

短语必须 ASCII：本仓 pg_trgm 对中文不给分，中文查询恒空会把「漏」读成「正常」。

侧栏两榜（`/forums/side` 的今日热帖榜、最新回复榜）都是 top5 且**按回复数/
回帖 id 排**，零回复的新主题永远进不去 ⇒ 光建主题打这个口只能得到
「两边都看不见」的假绿。本闸门因此按「窗口内现网最高回复数 + 1」给夹具主题
铺回帖，把它顶到榜首，职员看得见才算正控成立。夹具回帖一律 `user_id IS NULL`：
worker 的成就扫描按 `p.user_id = u.id` 统计发帖数，NULL 永不匹配 ⇒ 探针行
不会被分钟级 tick 结算成任何人的成就。
"""
import json
import os
import re
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, dig, ok, summary  # noqa: E402
from ci_reg_common import jwt, psql  # noqa: E402

MARK = "tx-gate-%d" % int(time.time())
PHRASE = "ZebraQuix%s" % str(int(time.time()))[-6:]
NAME = "闸门夹具-受限版块-" + MARK
DOORS = [
    ("/forums/search?q=%s", "全站搜索"),
    ("/forums/recent?limit=200", "最近主题"),
    ("/forums/{fid}/topics", "按版块列主题"),
    ("/forums/topics/{tid}", "主题详情"),
    # 版块清单：本轮实测职员侧含夹具 fid、会员侧不含 ⇒ 正控成立，收进判据
    ("/forums/boards", "版块清单"),
]

member = int(psql("SELECT id FROM users WHERE class_id < 5 AND status < 2 "
                  "AND id <> 1 ORDER BY id DESC LIMIT 1") or 0)
ok("TX0 夹具读者取到（class<5 的正常会员）", bool(member), "member=%s" % member)
if not member:
    summary()
mcls = int(psql("SELECT class_id FROM users WHERE id=%d" % member) or 1)
tm = jwt(member, cls=mcls)
ts = jwt(1, cls=int(psql("SELECT class_id FROM users WHERE id=1")))

# 静态先钉：版块读档的执法点在哪几处（被摘掉就红，不等活体）
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
src = os.path.join(ROOT, "apps", "api", "src", "community_http")


def body(rel):
    t = open(os.path.join(src, rel), encoding="utf-8", errors="replace").read()
    return "\n".join(l.split("//")[0] for l in t.split("\n"))


forums_b, post_b = body("forums.rs"), body("post_read.rs")
ok("TX1 版块列表口仍按读档过滤（minclassread 在册）",
   "minclassread" in forums_b or "can_read" in forums_b)
ok("TX2 主题读口仍按读档过滤", "can_read" in post_b or "minclassread" in post_b)

fid = tid = 0
POSTS0 = psql("SELECT count(*) FROM posts")
try:
    st, b = call("POST", "/admin/forums",
                 {"name": NAME, "descr": "闸门夹具，跑完即删",
                  "minclassread": 5, "minclasswrite": 5, "minclasscreate": 5},
                 token=ts)
    fid = int(psql("SELECT id FROM forums WHERE name='%s'" % NAME) or 0)
    ok("TX3 夹具版块建成（fid=%s http=%s code=%s msg=%s）"
       % (fid, st, b.get("code"), str(b.get("message"))[:50]), bool(fid))
    if not fid:
        summary()
    st, b = call("POST", "/forums/topics",
                 {"forum_id": fid, "title": PHRASE + " 受限读探针",
                  "body": "正文只存在于受限版块：" + PHRASE}, token=ts)
    tid = int(psql("SELECT id FROM topics WHERE title LIKE '%%%s%%' "
                   "ORDER BY id DESC LIMIT 1" % PHRASE) or 0)
    ok("TX4 夹具主题建成（tid=%s http=%s code=%s）" % (tid, st,
       b.get("code")), bool(tid))
    if not tid:
        summary()

    print("== TX5 活体：四道口，职员必须看得见、会员必须看不见 ==")

    def sees(blob):
        return (PHRASE in blob
                or ('"forum_id": %d' % fid) in blob
                or re.search(r'\bid": %d\b' % fid, blob) is not None)

    for tpl, label in DOORS:
        p = tpl % PHRASE if "%s" in tpl else tpl.format(fid=fid, tid=tid)
        m_st, m_b = call("GET", p, token=tm)
        s_st, s_b = call("GET", p, token=ts)
        mt = json.dumps(m_b, ensure_ascii=False)
        stx = json.dumps(s_b, ensure_ascii=False)
        m_see, s_see = sees(mt), sees(stx)
        if not s_see:
            ok("TX5 %s：正控不成立（职员也看不见⇒这条没被 "
               "exercised，不许算已验）" % label, False,
               "会员 http=%s 职员 http=%s len=%d"
               % (m_st, s_st, len(stx)))
        else:
            ok("TX5 %s：职员看得见、会员必须看不见（会员看见=%s）"
               % (label, m_see), not m_see,
               "会员 http=%s code=%s 命中短语=%s"
               % (m_st, m_b.get("code"), m_see))

    # ---- 侧栏两榜：先给夹具主题铺回帖顶进 top5，再逐榜打 A/B ----
    top = int(psql(
        "SELECT greatest(max(r), 0) FROM (SELECT greatest("
        "  (SELECT count(*) FROM posts p WHERE p.topic_id = t.id) - 1, 0)"
        " AS r FROM topics t JOIN forums f ON f.id = t.forum_id"
        " WHERE f.minclassread <= 99 AND t.id <> %d"
        "   AND COALESCE(t.last_post_at, t.created_at)"
        "       >= now() - make_interval(days => 7)) x" % tid) or 0)
    need = min(top + 1, 20)
    psql("INSERT INTO posts (id, topic_id, user_id, body, body_text)"
         " SELECT nextval('posts_id_seq'), %d, NULL,"
         " '%s 夹具回帖', '%s 夹具回帖' FROM generate_series(1, %d)"
         % (tid, PHRASE, PHRASE, need))
    laid = int(psql("SELECT count(*) FROM posts WHERE topic_id=%d"
                    " AND user_id IS NULL" % tid) or 0)
    ok("TX6 夹具回帖铺齐（窗口内最高回复数=%s⇒补 %s 条，实际 %s 条）"
       % (top, need, laid), laid == need)

    side_s = call("GET", "/forums/side", token=ts)
    side_m = call("GET", "/forums/side", token=tm)

    def rows(env, key):
        return dig(env[1], "data", key) or []

    def hit_idx(env, key):
        for i, r in enumerate(rows(env, key)):
            if PHRASE in json.dumps(r, ensure_ascii=False):
                return i
        return -1

    ok("TX7 会员侧栏两榜不是空榜（热帖 %s 行、最新回复 %s 行⇒"
       "「看不见夹具」不是「整块没货」）"
       % (len(rows(side_m, "hot")), len(rows(side_m, "latest_replies"))),
       len(rows(side_m, "hot")) > 0 and len(rows(side_m, "latest_replies")) > 0,
       "会员 http=%s code=%s" % (side_m[0], side_m[1].get("code")))

    for key, label in (("hot", "今日热帖榜"),
                       ("latest_replies", "最新回复榜")):
        rank, mrank = hit_idx(side_s, key), hit_idx(side_m, key)
        if rank < 0:
            ok("TX8 侧栏%s：正控不成立（职员 top%s 里也没有夹具⇒这条口"
               "没被 exercised，不许算已验）" % (label, len(rows(side_s, key))),
               False, "职员 http=%s code=%s 会员 http=%s"
               % (side_s[0], side_s[1].get("code"), side_m[0]))
        else:
            ok("TX8 侧栏%s：职员看得见（第 %s 位）、会员必须看不见"
               % (label, rank + 1), mrank < 0,
               "会员 http=%s code=%s 命中第 %s 位；职员榜=%s 会员榜=%s"
               % (side_m[0], side_m[1].get("code"), mrank,
                  [r.get("id") or r.get("topic_id") for r in rows(side_s, key)],
                  [r.get("id") or r.get("topic_id") for r in rows(side_m, key)]))

finally:
    if tid:
        psql("DELETE FROM posts WHERE topic_id=%d" % tid)
        psql("DELETE FROM topics WHERE id=%d" % tid)
    if fid:
        psql("DELETE FROM forums WHERE id=%d" % fid)
    left_t = "0" if not tid else psql(
        "SELECT count(*) FROM topics WHERE id=%d" % tid)
    left_f = "0" if not fid else psql(
        "SELECT count(*) FROM forums WHERE id=%d" % fid)
    left_p = "0" if not tid else psql(
        "SELECT count(*) FROM posts WHERE topic_id=%d" % tid)
    left_body = psql("SELECT count(*) FROM posts WHERE body LIKE '%%%s%%'"
                     % PHRASE)
    ok("TX9 残料清零（主题=%s 版块=%s 回帖=%s 带短语的帖=%s；"
       "夹具前后现网 posts=%s→%s）"
       % (left_t, left_f, left_p, left_body, posts_before,
          psql("SELECT count(*) FROM posts")),
       left_t == "0" and left_f == "0" and left_p == "0"
       and left_body == "0")

summary()
