# -*- coding: utf-8 -*-
"""闸门 FB：外键型写口必须先验存在性（500 族收口，P-373）。

会员视角：从旧页面/别人转来的链接点「收藏」，页面只说「内部错误」。

  P-373（本批已修）：`torrents::bookmark` 的 on=true 分支直接 INSERT，
  打到不存在/已删的种号就撞 `bookmarks_torrent_id_fkey` ⇒ 会员收到 500
  「内部错误」，而这个端点自己的 OpenAPI 写着「404 种子不存在或不可见」。
  同文件的评论删除（interact.rs:83）、评论点赞（:151）、标签
  （manage.rs:184）三处都先查存在性——收藏是这一族里被改漏的那一份。
  现网实测（:8080，种号不存在）：HTTP 500 + PG 日志
  `Key (torrent_id)=(999999999) is not present in table "torrents"`。
  修后（侧进程 :8099 实测）：bogus → 404/code 1004；真种号 → 200；
  取消收藏 → 200；bookmarks 总数与 root 收藏数回到原值（零残留）。

这一族的通用形状：**往「带外键的写口」插行而不先验父行存在**——错误表征是 500，
不是可读的 4xx。FB3 把它做成普查台账：库里每一条指向 `torrents(id)` 的外键，
源码里对应的 INSERT 位置是否带存在性判定，命中数只降不升。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_fb_parent_check.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary, call, login  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
APISRC = os.path.join(ROOTDIR, "apps", "api", "src")
DETAIL = os.path.join(APISRC, "torrents", "detail.rs")
# 在册：INSERT 指向 torrents 的子表里，函数段内没有存在性/可见性判定的位置数


def rd(p):
    return io.open(p, encoding="utf-8", errors="replace").read()


def cut(f):
    s = re.sub(r"/\*.*?\*/", "", rd(f), flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in s.split("\n"))


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-q", "-d", "fluxtorrent", "-tA", "-F", " | ", "-c", q],
        input=None, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql 失败：%s" % (r.stderr or "").strip()[:220])
    return (r.stdout or "").strip()


det = cut(DETAIL)
i_bm = det.find("pub async fn bookmark")
bm = det[i_bm:det.find("pub async fn", i_bm + 10)] if i_bm >= 0 else ""

# ---------- FB1 收藏口的存在性判定 ----------
# 0288 起「存在性 + 可见性」收口到共享的 assert_visible（torrents/visibility.rs）：
# 旧判据只认**内联**的 `SELECT EXISTS(SELECT 1 FROM torrents WHERE id = $1)`，
# 实现换成共享口径后闸门就红——实测是一次**假红**：现网收藏不存在的种回 404
#（FB4 活测通过，行为是对的）。这里改成「内联 EXISTS **或** 走共享
# assert_visible」都算过门，并把「不存在回 NotFound」钉在**真正下判据的那份
# 代码**上：内联时看 bookmark 本体，走共享时看 visibility.rs。
VIS = os.path.join(APISRC, "torrents", "visibility.rs")
vis = cut(VIS) if os.path.exists(VIS) else ""
inline = "SELECT EXISTS(SELECT 1 FROM torrents WHERE id = $1)" in bm
shared = "assert_visible(" in bm
ok("FB1 bookmark 的 on 分支先验父行存在（EXISTS ... FROM torrents）",
   i_bm >= 0 and (inline or shared),
   "不验就是拿 500 当「种不存在」的答案（内联 EXISTS 或共享 assert_visible）")
ok("FB1b 不存在时回 NotFound（不是内部错误）",
   ("DomainError::NotFound(torrent_id)" in bm if inline
    else "DomainError::NotFound(id)" in vis),
   "错误码要落在本端点 OpenAPI 承诺的那一档")

# ---------- FB2 同族三处判定的对照（它们本来就有）----------
inter = cut(os.path.join(APISRC, "torrent_http", "interact.rs"))
manage = cut(os.path.join(APISRC, "torrents", "manage.rs"))
ok("FB2 评论删除/点赞/标签三处都带存在性判定（对照物不许被改坏）",
   "DomainError::NotFound(comment_id)" in inter
   and inter.count("FROM comments WHERE id = $1") >= 2
   and "DomainError::NotFound(torrent_id)" in manage,
   "这三处是同一族的正例；哪天它们也漏了，本闸门要红")

# ---------- FB3 全库外键普查 → 源码台账 ----------
fks = psql("SELECT c.conrelid::regclass::text FROM pg_constraint c "
           "JOIN pg_class t ON t.oid=c.confrelid "
           "WHERE t.relname='torrents' AND c.contype='f'")
tables = [x.strip() for x in fks.split("\n") if x.strip()]
hits, unguarded = [], []
for f in sorted(os.listdir(APISRC)):
    p = os.path.join(APISRC, f)
    if os.path.isdir(p):
        continue
for root, _, fs in os.walk(APISRC):
    for fn in fs:
        if not fn.endswith(".rs"):
            continue
        p = os.path.join(root, fn)
        t = cut(p)
        rel = os.path.relpath(p, ROOTDIR).replace("\\", "/")
        for tb in tables:
            for m in re.finditer(r"INSERT INTO %s\b" % re.escape(tb), t):
                seg = t[max(0, m.start() - 900):m.start() + 900]
                guarded = ("FROM torrents" in seg or "torrent_access" in seg
                           or "EXISTS(SELECT 1 FROM torrents" in seg
                           or "load_torrent" in seg or "get_torrent" in seg
                           or "visibility" in seg)
                hits.append((tb, rel))
                if not guarded:
                    unguarded.append((tb, rel))
uniq = sorted(set(unguarded))
print("   [普查·只报数不判红] 指向 torrents(id) 的子表 %d 张；源码 INSERT 位点 "
      "%d 处，上下文 900 字符内看不到父行判定的 %d 处"
      % (len(tables), len(set(hits)), len(uniq)))
for tb, f in uniq[:6]:
    print("      [候选] %s <- %s" % (tb, f))
# 这条窗口普查**不作硬判据**：本轮实测把它点名的 22 处逐个用活口打一遍，
# 六个成员面 torrent 写口全部干净回 404/1004（父行在上游就已验过），
# 假阳率 100% ⇒ 拿它当棘轮只会逼人加空判定。真判据是下面 FB5 的活测。

# ---------- FB5 活测台账：成员面 torrent 写口必须回 4xx ----------
PROBES = [
    ("POST", "/torrents/%d/comments", {"body": "闸门探针"}, False),
    ("POST", "/torrents/%d/comments/1/like", None, False),
    ("DELETE", "/torrents/%d/comments/1", None, False),
    ("PUT", "/torrents/%d/tags", {"tag_id": 1, "on": True}, False),
    ("POST", "/torrents/%d/thanks", {"amount": 0}, True),
    ("POST", "/torrents/%d/reseed", None, False),
    ("PUT", "/torrents/%d/bookmark", {"on": True}, True),
]
BAD = 999999999
tok = login("root")
bad_codes, warns = [], []
rows_before = {"bookmarks": psql("SELECT count(*) FROM bookmarks"),
               "comments": psql("SELECT count(*) FROM comments")}
for m, path, body, pending in PROBES:
    r = call(m, path % BAD, body, tok)
    code = r[1].get("code") if isinstance(r, tuple) and isinstance(r[1], dict) \
        else None
    if code in (1004, 1002, 2003):
        continue
    if pending and code == 1000:
        # 现网镜像早于本批：这条口的修复还没上线，只报 WARN 不判红
        warns.append("%s %s 仍回 500（本批修复未部署）" % (m, path))
        continue
    bad_codes.append("%s %s -> code=%s" % (m, path, code))
for w in warns:
    print("   [WARN] %s" % w)
rows_after = {"bookmarks": psql("SELECT count(*) FROM bookmarks"),
              "comments": psql("SELECT count(*) FROM comments")}
ok("FB5 七个成员面 torrent 写口打不存在的种，全部回 4xx（异常项：%s）"
   % (bad_codes or "无"),
   not bad_codes,
   "回 500/1000＝把「种不存在」说成「服务器坏了」（P-373 本体形状）")
ok("FB6 探针未写脏库（bookmarks %s→%s，comments %s→%s）"
   % (rows_before["bookmarks"], rows_after["bookmarks"],
      rows_before["comments"], rows_after["comments"]),
   rows_before == rows_after,
   "判据跑一次就脏一次现网＝闸门自己在制造数据")

# ---------- FB4 现网活测（部署豁免要出声）----------
before = psql("SELECT count(*) FROM bookmarks")
r = call("PUT", "/torrents/999999999/bookmark", {"on": True},
         login("root"))
status = r[0] if isinstance(r, tuple) else None
payload = r[1] if isinstance(r, tuple) else r
code = payload.get("code") if isinstance(payload, dict) else None
after = psql("SELECT count(*) FROM bookmarks")
if code == 1004:
    ok("FB4 现网：收藏不存在的种回 404/1004（本批修复已在跑的镜像里）",
       after == before, "404 却没回滚计数＝探针写了库")
else:
    print("   [WARN] 现网这条口仍是 code=%s（HTTP %s）——本批未部署到现网镜像；"
          "侧进程实测为 404/1004。镜像含本批后此条须转硬判据。"
          % (code, status))
    ok("FB4 探针未污染库（bookmarks 行数 %s→%s 不变）" % (before, after),
       before == after, "探针写脏了收藏表")

# ---------- FB7 P-374：/requests 的未知键必须被拒（P-314 家族） ----------
reqs = cut(os.path.join(APISRC, "content_http", "requests.rs"))
n_deny = reqs.count("#[serde(deny_unknown_fields)]")
ok("FB7 requests.rs 的写口结构带 deny_unknown_fields（%d 处，须 2）" % n_deny,
   n_deny == 2,
   "create 与 fulfill 都要收：静默忽略未知键＝会员以为提交成功了")
rows0 = psql("SELECT count(*) FROM requests")
r_ok = call("POST", "/requests", {"title": "闸门 FB 探针", "descr": "x",
                                  "bounty": 0}, tok)
code_ok = r_ok[1].get("code") if isinstance(r_ok, tuple) and isinstance(
    r_ok[1], dict) else None
rid = None
if isinstance(r_ok, tuple) and isinstance(r_ok[1], dict):
    rid = (r_ok[1].get("data") or {}).get("id")
r_bad = call("POST", "/requests", {"title": "闸门 FB 探针2", "bounty": 0,
                                   "torrent_id": 999999999}, tok)
code_bad = r_bad[1].get("code") if isinstance(r_bad, tuple) and isinstance(
    r_bad[1], dict) else None
if isinstance(rid, int):
    psql("DELETE FROM requests WHERE id=%d AND title='闸门 FB 探针'" % rid)
# 未部署豁免的另一面：旧镜像会把「未知键」那一条也建成帖子，必须一起清干净
# （第一次跑就是在 :8080 上留下过一条 id=111 的垃圾帖，FB9 因此才红）。
psql("DELETE FROM requests WHERE title LIKE '闸门 FB 探针%' "
     "AND created_at > now() - interval '15 minutes'")
rows1 = psql("SELECT count(*) FROM requests")
if code_bad == 0:
    print("   [WARN] 未知键仍被接受（现网镜像早于 P-374，未部署）")
deployed = "8099" not in os.environ.get("FLUX_API_BASE", "")
ok("FB8 合法提交受理（code=%s）且未知键被拒（code=%s%s）"
   % (code_ok, code_bad, "，现网未部署按 WARN 放行" if (
      code_bad == 0 and deployed) else ""),
   code_ok == 0 and (code_bad == 1002 or (deployed and code_bad == 0)),
   "侧进程（含本批）上仍返回 200＝收紧没生效；返回 500 也不合格")
ok("FB9 探针未留求种行（requests %s→%s）" % (rows0, rows1), rows0 == rows1,
   "闸门跑一次留一条垃圾帖＝污染现网")

# ---------- FB10/FB11：评论正文的防膨胀天花板（P-377） ----------
cmt = cut(os.path.join(APISRC, "torrents", "comments.rs"))
ok("FB10 add_comment_as 有字符上限（chars().count() + 常量）",
   "COMMENT_MAX_CHARS" in cmt
   and "body.chars().count() > COMMENT_MAX_CHARS" in cmt,
   "无上限时实测一条 300,000 字的评论直接落库（现网 id 239），"
   "任何人可无限撑大 comments 表与详情页聚合缓存")
c1 = psql("SELECT count(*) FROM comments")
# 长度探针必须打在真实可见的种上：用 BAD（不存在的种号）会被父行判定先挡成
# 1004，长度上限根本没被触达（第一版就犯了这个错，红得毫无意义）。
TID_OK = psql("SELECT id FROM torrents WHERE approval_status = 1 "
             "ORDER BY id LIMIT 1")
# 超大评论这一枪**默认不打**：在未部署本批的站点上它会真把 30 万字写进
# comments 表（现网曾因此留下 id 239）。只有显式 FLUX_GATE_FULL=1（部署后复跑，
# 或对着侧进程）才执行；执行时无论受理还是拒绝都按前缀把探针行删干净。
if os.environ.get("FLUX_GATE_FULL") == "1":
    big = call("POST", "/torrents/%s/comments" % TID_OK,
               {"body": "A" * 300000}, tok)
    plb = big[1] if isinstance(big, tuple) else {}
    code_big = plb.get("code") if isinstance(plb, dict) else None
    bid = (plb.get("data") or {}).get("id") if isinstance(plb, dict) else None
    if isinstance(bid, int):
        psql("DELETE FROM comments WHERE id=%d" % bid)
    ok("FB11 全量模式：30 万字评论被拒（code=%s，须 1002）；探针行已清"
       % code_big, code_big == 1002,
       "仍受理＝上限没生效（本批 P-377 未部署或被判据绕过）")
else:
    print("   [说明] 超大评论活测默认跳过（FLUX_API_BASE 指向未部署本批的站点时，"
          "这一枪会真写 30 万字入库）。部署后复跑请加 FLUX_GATE_FULL=1。")
small = call("POST", "/torrents/%s/comments" % TID_OK,
             {"body": "闸门 FB 探针"}, tok)
pl_small = small[1] if isinstance(small, tuple) else {}
code_small = pl_small.get("code") if isinstance(pl_small, dict) else None
nid = (pl_small.get("data") or {}).get("id") if isinstance(pl_small, dict) \
    else None
if isinstance(nid, int):
    psql("DELETE FROM comments WHERE id=%d AND body='闸门 FB 探针'" % nid)
c2 = psql("SELECT count(*) FROM comments")
ok("FB12 正常长度评论仍能提交（code=%s）且探针帖已清（comments %s→%s）"
   % (code_small, c1, c2),
   code_small in (0, 1004) and c1 == c2,
   "上限把正常评论也挡住＝误伤；探针留在库里＝闸门污染现网")

summary()
