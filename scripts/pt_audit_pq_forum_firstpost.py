# -*- coding: utf-8 -*-
"""闸门 PQ：论坛首帖与 protected 版块窗口（2026-10-21 批二百三十三）。

钉两件实测缺陷：
  P-494 `DELETE /forums/posts/{id}` 过去不区分首帖。topics 没有
        first_post_id，仓里多处拿 min(id) 当首帖（protected 正文保护、
        搜索片段、最新回复、我的回帖、删主题对账）。版主删掉一楼后二楼
        顶成一楼，主题变成「有标题没正文」，楼层号与回帖统计全跟着漂。
        现在首帖删除回 400/1002 并指向「删主题」（DomainError::Validation
        在 errors.rs 里映射的是 400，OpenAPI 注解写的 422 不作数）；
        普通回帖照常可删。
  P-495 protected 版块「只公开一楼」按本页下标放行。详情默认只取最近
        200 楼，长主题的第一条其实是回帖；旧码把每页的下标 0 原文送出，
        `?before=` 还能指定窗口复现。现在只认真实首帖 min(id)。

红证（旧镜像）：P-495 默认窗口泄漏 1 条回帖正文；P-494 删首帖 http=200
且详情只剩一楼。夹具：探针分区/版块/低权号/两张主题（1 楼 + 205 回帖），
跑完删净并回读零残留。
"""
import atexit
import os
import re
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
LUID = None
CID = None


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % r.stderr.strip()[:200])
    return r.stdout.strip()


def exec_sql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-c", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        print("   ! SQL 失败: %s" % (r.stderr or "").strip()[:150])
    return r.returncode


def cleanup():
    """幂等：按本轮专属前缀/id 删净，中途崩了 atexit 也会跑。"""
    if LUID:
        exec_sql("DELETE FROM topic_reads WHERE user_id = %s" % LUID)
        exec_sql("DELETE FROM follows WHERE user_id = %s" % LUID)
    exec_sql("DELETE FROM topic_reads WHERE topic_id IN "
             "(SELECT id FROM topics WHERE title LIKE 'PQgate-%')")
    exec_sql("DELETE FROM topics WHERE title LIKE 'PQgate-%'")
    exec_sql("DELETE FROM forums WHERE name LIKE 'PQgate-board-%'")
    exec_sql("DELETE FROM forum_categories WHERE name LIKE 'PQgate-cat-%'")
    if LUID:
        exec_sql("UPDATE users SET status = 2 WHERE id = %s" % LUID)
    if CID:
        exec_sql("DELETE FROM invites WHERE id = %s" % CID)


atexit.register(cleanup)


def login_backoff(user, tries=10):
    for _ in range(tries):
        s, r = call("POST", "/auth/login",
                    {"username": user, "password": "password123"})
        tok = (r.get("data") or {}).get("token")
        if tok:
            return tok
        time.sleep(8 if r.get("code") == 1015 or s == 429 else 3)
    return None


def new_member():
    """注册 24h/2 IP：先清自己探针号留下的额度再按节拍建号。"""
    global LUID, CID
    clear_register_rl()
    time.sleep(6)
    codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                   token=ROOT)[1].get("data") or {}).get("codes")) or []
    if not codes:
        return None
    code, CID = codes[0]["code"], codes[0]["id"]
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if len(nums) < 2:
        return None
    user = "pq%s%d" % ("m", TAG)
    s, rr = call("POST", "/auth/register", {
        "username": user, "email": "%s@pq.local" % user,
        "password": "password123", "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}})
    uid = (rr.get("data") or {}).get("user_id")
    if not uid:
        print("   ! 注册失败: %s %s" % (s, str(rr.get("message"))[:100]))
        return None
    LUID = int(uid)
    tok = login_backoff(user)
    if not tok:
        return None
    return (tok, LUID, user)


def mk_topic(forum_id, title, bodies):
    tid = psql("INSERT INTO topics (forum_id, user_id, title) VALUES "
               "(%s, 1, '%s') RETURNING id" % (forum_id, title))
    vals = ",".join(
        "(nextval('posts_id_seq'), %s, 1, '%s', '%s')" % (tid, b, b)
        for b in bodies)
    exec_sql("INSERT INTO posts (id, topic_id, user_id, body, body_text) "
             "VALUES " + vals)
    return tid


def detail(tok, tid, before=None):
    path = "/forums/topics/%s" % tid
    if before:
        path += "?before=%s" % before
    s, r = call("GET", path, token=tok)
    return s, (r.get("data") or {})


def leaked_bodies(posts):
    return [p for p in posts
            if str(p.get("body") or "").startswith("PQLONG")]


print("== PQ0 前置：探针分区/版块/低权号/主题 ==")
# 上一次中途崩了留下的残料（本闸门独有前缀，不碰别人的数据）
cleanup()
exec_sql("INSERT INTO forum_categories (name, sort, visible) "
         "VALUES ('PQgate-cat-%d', 900, TRUE)" % TAG)
CAT = psql("SELECT id FROM forum_categories "
           "WHERE name = 'PQgate-cat-%d'" % TAG)
exec_sql("INSERT INTO forums (name, descr, min_class, minclassread, "
         "minclasswrite, minclasscreate, protected, category_id, sort) "
         "VALUES ('PQgate-board-%d', 'probe board', 0, 0, 0, 0, TRUE, %s, 900)"
         % (TAG, CAT))
FID = psql("SELECT id FROM forums WHERE name = 'PQgate-board-%d'" % TAG)
LOW = new_member()
LT = LOW[0] if LOW else None
LONG_BODIES = ["PQLONG%d-%03d 正文" % (TAG, i) for i in range(206)]
T1 = mk_topic(FID, "PQgate-long-%d" % TAG, LONG_BODIES)
T2 = mk_topic(FID, "PQgate-del-%d" % TAG,
              ["PQDEL%d-首帖正文" % TAG, "PQDEL%d-回帖正文" % TAG])
FIRST1 = int(psql("SELECT min(id) FROM posts WHERE topic_id = %s" % T1))
SECOND1 = int(psql("SELECT id FROM posts WHERE topic_id = %s "
                   "ORDER BY id OFFSET 1 LIMIT 1" % T1))
FIRST2 = int(psql("SELECT min(id) FROM posts WHERE topic_id = %s" % T2))
REPLY2 = int(psql("SELECT max(id) FROM posts WHERE topic_id = %s" % T2))
ok("PQ0 前置齐备（分区/版块/低权号/两张主题）",
   bool(CAT and FID and LT and T1 and T2)
   and REPLY2 > FIRST2 and SECOND1 > FIRST1,
   "CAT=%s FID=%s LOW=%s T1=%s T2=%s" % (CAT, FID, bool(LT), T1, T2))
if not (CAT and FID and LT and T1 and T2):
    cleanup()
    summary()

print("== PQ-A P-494：首帖不能按回帖删 ==")
s, r = call("DELETE", "/forums/posts/%s" % FIRST2, token=LT)
ok("PQ-A1 普通成员删首帖仍被 403 挡住（正控）", s == 403,
   "http=%s msg=%s" % (s, str(r.get("message"))[:60]))
s, r = call("DELETE", "/forums/posts/%s" % FIRST2, token=ROOT)
msg = str(r.get("message") or "")
ok("PQ-A2 版主删首帖必须 400/1002（一楼不能按回帖删）",
   s == 400 and r.get("code") == 1002 and "首帖" in msg,
   "http=%s code=%s msg=%s" % (s, r.get("code"), msg[:60]))
s, d = detail(ROOT, T2)
posts = d.get("posts") or []
ok("PQ-A3 拒绝之后主题仍完整（一楼与回帖都在）",
   s == 200 and len(posts) == 2
   and any(int(p.get("id") or 0) == FIRST2 for p in posts),
   "http=%s posts=%d" % (s, len(posts)))
s, r = call("DELETE", "/forums/posts/%s" % REPLY2, token=ROOT)
ok("PQ-A4 普通回帖照常可删（正控）", s == 200,
   "http=%s msg=%s" % (s, str(r.get("message"))[:60]))
s, d = detail(ROOT, T2)
ok("PQ-A5 删回帖后只剩一楼", s == 200 and len(d.get("posts") or []) == 1,
   "http=%s posts=%d" % (s, len(d.get("posts") or [])))
s, r = call("DELETE", "/forums/topics/%s" % T2, token=ROOT)
ok("PQ-A6 提示的逃生口「删主题」真的可用", s == 200,
   "http=%s msg=%s" % (s, str(r.get("message"))[:60]))

print("== PQ-B P-495：protected 长主题的窗口放行 ==")
s, d = detail(LT, T1)
posts = d.get("posts") or []
ok("PQ-B1 地基：低权号能读到长主题的 200 楼窗口",
   s == 200 and len(posts) == 200, "http=%s posts=%d" % (s, len(posts)))
leaked = leaked_bodies(posts)
ok("PQ-B2 protected 长主题默认窗口不得漏出回帖正文",
   s == 200 and len(posts) == 200 and not leaked,
   "泄漏=%d 首条=%s" % (len(leaked),
                        str(leaked[0].get("body"))[:60] if leaked else "-"))
s, d = detail(LT, T1, before=SECOND1)
posts = d.get("posts") or []
ok("PQ-B3 正控：翻到第一页仍能看到一楼公开正文",
   s == 200 and len(posts) == 1
   and str(posts[0].get("body") or "").startswith("PQLONG"),
   "http=%s posts=%d body=%s" % (s, len(posts),
                                 str((posts[0] if posts else {}).get("body"))
                                 [:60]))
s, d = detail(ROOT, T1)
posts = d.get("posts") or []
ok("PQ-B4 正控：staff 读 protected 长主题正文全可见",
   s == 200 and len(leaked_bodies(posts)) == 200,
   "http=%s visible=%d/%d" % (s, len(leaked_bodies(posts)), len(posts)))

print("== PQ-C 收尾 ==")
cleanup()
left_t = psql("SELECT count(*) FROM topics WHERE title LIKE 'PQgate-%'")
left_p = psql("SELECT count(*) FROM posts WHERE body LIKE 'PQLONG%' "
              "OR body LIKE 'PQDEL%'")
left_f = psql("SELECT count(*) FROM forums WHERE name LIKE 'PQgate-board-%'")
left_c = psql("SELECT count(*) FROM forum_categories "
              "WHERE name LIKE 'PQgate-cat-%'")
left_r = psql("SELECT count(*) FROM topic_reads WHERE user_id = %s" % LUID)
status = psql("SELECT status FROM users WHERE id = %s" % LUID)
ok("PQ-C1 收尾：探针内容/版块/分区/已读标记零残留，探针号已停用",
   left_t == left_p == left_f == left_c == left_r == "0"
   and status == "2",
   "topics=%s posts=%s forums=%s cats=%s reads=%s status=%s"
   % (left_t, left_p, left_f, left_c, left_r, status))
summary()
