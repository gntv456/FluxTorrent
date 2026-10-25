"""受限版块读口闸门（TX）。

不变量：版块的 `minclassread` 必须**同时**挡住「按版块列主题」「主题详情」
「全站搜索」「最近主题」——只要有一个旁路口不判读档，受限内容就会从搜索/
聚合面漏出去（版主配置受限版块时以为「已经关起来了」）。

现网原本没有任何 minclassread>1 的版块，这一案造不出对象，所以本闸门自带
夹具、自带清理：**建一个 read=5 的版块 + 一条只存在于它的主题（标题带本轮
唯一 ASCII 短语）→ 会员与职员各打一遍 → 删回去并回读**。职员必须看得见
（正控，否则「会员看不见」可能是「这条口本来不出货」的假绿），会员必须看不见。

短语必须 ASCII：本仓 pg_trgm 对中文不给分，中文查询恒空会把「漏」读成「正常」。
"""
import os
import re
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, ok, summary  # noqa: E402
from ci_reg_common import jwt, psql  # noqa: E402

MARK = "tx-gate-%d" % int(time.time())
PHRASE = "ZebraQuix%s" % str(int(time.time()))[-6:]
NAME = "闸门夹具-受限版块-" + MARK
DOORS = [
    ("/forums/search?q=%s", "全站搜索"),
    ("/forums/recent?limit=200", "最近主题"),
    ("/forums/{fid}/topics", "按版块列主题"),
    ("/forums/topics/{tid}", "主题详情"),
]

member = int(psql("SELECT id FROM users WHERE class_id < 5 AND status < 2 "
                  "AND id <> 1 ORDER BY id DESC LIMIT 1") or 0)
ok("TX0 夹具读者取到（class<5 的正常会员）", bool(member), "member=%s" % member)
if not member:
    summary()
tm = jwt(member, cls=int(psql("SELECT class_id FROM users WHERE id=%d" % member)))
ts = jwt(1, cls=int(psql("SELECT class_id FROM users WHERE id=1")))

# 静态先钉：版块读档的执法点在哪几处（被摘掉就红，不等活体）
src = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                   "apps", "api", "src", "community_http")


def body(rel):
    t = open(os.path.join(src, rel), encoding="utf-8", errors="replace").read()
    return "\n".join(l.split("//")[0] for l in t.split("\n"))


forums_b, post_b = body("forums.rs"), body("post_read.rs")
ok("TX1 版块列表口仍按读档过滤（minclassread 在册）",
   "minclassread" in forums_b or "can_read" in forums_b)
ok("TX2 主题读口仍按读档过滤", "can_read" in post_b or "minclassread" in post_b)

fid = tid = 0
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
    ok("TX4 夹具主题建成（tid=%s http=%s code=%s）" % (tid, st, b.get("code")),
       bool(tid))
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
        mt, stx = str(m_b), str(s_b)
        m_see, s_see = sees(mt), sees(stx)
        if not s_see:
            ok("TX5 %s：正控不成立（职员也看不见⇒这条没被 exercised，"
               "不许算已验）" % label, False,
               "会员 http=%s 职员 http=%s len=%d" % (m_st, s_st, len(stx)))
        else:
            ok("TX5 %s：职员看得见、会员必须看不见（会员看见=%s）" % (label, m_see),
               not m_see, "会员 http=%s code=%s 命中短语=%s" % (
                   m_st, m_b.get("code"), m_see))
finally:
    if tid:
        psql("DELETE FROM posts WHERE topic_id=%d" % tid)
        psql("DELETE FROM topics WHERE id=%d" % tid)
    if fid:
        psql("DELETE FROM forums WHERE id=%d" % fid)
    left_t = psql("SELECT count(*) FROM topics WHERE id=%d" % tid) if tid else "0"
    left_f = psql("SELECT count(*) FROM forums WHERE id=%d" % fid) if fid else "0"
    ok("TX6 残料清零（主题=%s 版块=%s；现网总量 版块=%s 主题=%s）"
       % (left_t, left_f, psql("SELECT count(*) FROM forums"),
          psql("SELECT count(*) FROM topics")),
       left_t == "0" and left_f == "0")

summary()
