# -*- coding: utf-8 -*-
"""闸门 IV：资源页「作品聚合」视图（GET /works 与 GET /works/{key}）。

这一族此前**零常驻判据**（`scripts/` 里连 `works` 这个词都没出现过），而它是
影视站的门面：`/torrents` 的 group 模式（`torrents-works.tsx`）与
`/works/[id]` 详情页都靠它取数。作品层不新建实体——归属口径写在 SQL 里
（优先 `group_id`，其次 imdb/tmdb/douban 外部 id，最后归一化标题 md5），
所以「接口回的数字」与「SQL 复算的数字」一旦分叉，用户在卡片上看到的
「3 个版本」点进去只有 2 个，且不会有任何报错。

判据刻意**不复制**那份归一化表达式，而是从源码里把 `WK` 常量原文抠出来
拼进复算 SQL——实现改了判据跟着改，不会漂成一个过期的第二把尺。

  IV1 列表可用且形状完整（items/total/limit/page，每项有 key/torrents/top_id）；
  IV2 逐张卡复算：种子数 / 体积 / max(id) 与 SQL 一致（取前 5 个作品）；
  IV3 匿名不可读（两处都要登录，与 /torrents 同口径）；
  IV4 不存在的作品键 ⇒ 404（不许演成 200 + data:null，也不许 500）；
  IV5 关键字搜索（优先取作品名里的中文串，显式 quote 走一遍非 ASCII 通路）：
      直接拼 URL 会被 Git Bash 按 GBK 百分号编码，中文搜索恒空＝假红）；
  IV6 排序白名单：sort=torrents 时返回的种子数单调不增；乱写的 sort 回落默认
      而不是 500（白名单之外的一切串都不该进 SQL）；
  IV7 卡片里挂的种子行全部是过审种（approval_status=1）——聚合口不许比
      列表口宽，把待审/被删的种随作品带出去；
  IV8 前端接线在册：`/torrents` 的 group 模式与 `/works/[id]` 两处调用点
      还在（后端改字段时这头要有读者）；
  IV9 分页不重叠，且极端页码不 500（`page_offset` 助手防负 offset）；
  IV10 作品键归一化：前导括号/空格/点号/尾括号同键、纯标签名回退非空、
      空 IMDb 不落裸 i（WK 表达式由源码提取后直接复算）。

跑法：PYTHONIOENCODING=utf-8 python scripts/pt_audit_iv_works_view.py
"""
import os
import re
import sys
import urllib.parse

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (call, ok, psql_stdin, summary)  # noqa: E402
from pt_audit_lib import login  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SQL_FILE = os.path.join(ROOTDIR, "apps", "api", "src", "torrent_http",
                        "works_sql.rs")
KEY_RE = re.compile(r"^[gind][0-9A-Za-z._|:-]{0,80}$")


def sval(q):
    return psql_stdin(q).strip()


def wk_expr():
    """从源码里抠出归一化作品键表达式（对 `t` 别名求值）。"""
    t = open(SQL_FILE, encoding="utf-8", errors="replace").read()
    m = re.search(r'const WK: &str = r#"(.*?)"#;', t, re.S)
    assert m, "works_sql.rs 里找不到 WK 常量——表达式被改名/搬走了，先修判据"
    return m.group(1).strip()


WK = wk_expr()
tok = login()
print("== IV1 列表形状 ==")
s, r = call("GET", "/works?limit=5", token=tok)
data = (r or {}).get("data") or {}
items = data.get("items") or []
bad_shape = [w.get("key") for w in items
             if not (w.get("key") and isinstance(w.get("torrents"), int)
                     and w.get("top_id") and w.get("category_id") is not None)]
ok("IV1 GET /works 200 且 items=%d / total=%s，每项 key+torrents+top_id 齐"
   % (len(items), data.get("total")),
   s == 200 and len(items) >= 1 and not bad_shape and data.get("limit") == 5,
   "http=%s code=%s 缺字段=%s" % (s, (r or {}).get("code"), bad_shape[:3]))

print("== IV2 卡片数字与 SQL 复算一致 ==")
mismatch, checked = [], 0
for w in items[:5]:
    k = w.get("key") or ""
    if not KEY_RE.match(k):
        mismatch.append("键形不合法 %r" % k)
        continue
    row = sval("SELECT count(*)||'/'||coalesce(sum(t.size),0)||'/'||"
               "max(t.id) FROM torrents t WHERE t.approval_status = 1 "
               "AND (%s) = '%s';" % (WK, k))
    c, sz, mx = (row.split("/") + ["", "", ""])[:3]
    checked += 1
    if (int(c) != w["torrents"] or int(mx) != w["top_id"]
            or int(sz) != w["size"]):
        mismatch.append("%s 接口 %s/%s/%s vs SQL %s" % (
            k, w["torrents"], w["size"], w["top_id"], row))
ctl = sval("SELECT count(*) FROM torrents t WHERE t.approval_status = 1 "
           "AND (%s) = 'nzzzz-not-a-work';" % WK)
ok("IV2 复算了 %d 部作品的种子数/体积/max(id)，分叉 %d 处；正控：喂一个假键"
   "复算须为 0（实=%s，证明这条 SQL 真在筛）" % (checked, len(mismatch), ctl),
   checked >= 3 and not mismatch and ctl == "0",
   " | ".join(mismatch[:3]) or "逐张一致")

print("== IV3 匿名不可读 ==")
s3, r3 = call("GET", "/works?limit=1")
s3b, r3b = call("GET", "/works/g1")
ok("IV3 匿名列表 http=%s、匿名详情 http=%s（都要 401，资源元数据不对外）"
   % (s3, s3b), s3 == 401 and s3b == 401, str(r3)[:90])

print("== IV4 不存在的作品键 ==")
fake = "n" + "0" * 32
s4, r4 = call("GET", "/works/" + fake, token=tok)
d4 = (r4 or {}).get("data")
ok("IV4 不存在键 ⇒ 404 且 code=1004（实测 http=%s code=%s，data 是 %s）"
   % (s4, (r4 or {}).get("code"), "null" if d4 is None else type(d4).__name__),
   s4 == 404 and (r4 or {}).get("code") == 1004,
   str(r4)[:110])

print("== IV5 中文关键字 ==")
# 关键字优先取作品名里的中文串（走一遍非 ASCII 的 URL 通路：Windows 下直接拼
# 会被 Git Bash 按 GBK 百分号编码，中文搜索恒空＝假红，所以这里必须显式
# urllib.parse.quote）；没有中文就退回作品名第一段。不靠 SQL 里挑，避免
# 「挑出来的串恰好没人含」这种判据自证式空跑。
cand = (items[0].get("name") or "").strip()
cjk = None
for w in items:
    m = re.search(r"[\u4e00-\u9fff]{2,6}", w.get("name") or "")
    if m:
        cjk = m.group(0)
        break
word = cjk or re.split(r"[\s\.|｜\[\(（【]", cand)[0][:6]
qs = urllib.parse.quote(word)
s5, r5 = call("GET", "/works?q=%s&limit=5" % qs, token=tok)
d5 = ((r5 or {}).get("data") or {}).get("items") or []
like = "%%%s%%" % word.replace("'", "''")
# 作品 q 现在与列表同源（默认范围 0 = 标题/简介/文件），
# 判据也按同一组字段复算，不能只认 name。
match = ("(t.name ILIKE '%s' OR COALESCE(t.small_descr, '') ILIKE '%s' "
         "OR COALESCE(t.descr, '') ILIKE '%s' OR EXISTS(SELECT 1 FROM files f "
         "WHERE f.torrent_id = t.id AND f.path ILIKE '%s'))"
         % (like, like, like, like))
expect = sval("SELECT count(DISTINCT (%s)) FROM torrents t WHERE "
              "t.approval_status = 1 AND %s;" % (WK, match))
leak = [w.get("key") for w in d5
        if sval("SELECT count(*) FROM torrents t WHERE t.approval_status=1 "
                "AND (%s) = '%s' AND %s"
                % (WK, w.get("key"), match)) == "0"]
# 正控：把关键字拉长成没人含的串，必须命中 0（否则 IV5「返回啥都算对」）
ctl5 = call("GET", "/works?q=%s&limit=5"
            % urllib.parse.quote(word + "绝不可能出现的后缀zzq"), token=tok)
ctl_items = ((ctl5[1] or {}).get("data") or {}).get("items") or []
ok("IV5 关键字「%s」：接口回 %d 部、SQL 说该串命中 %s 部作品、无越界项"
   "（越界=%s）；正控：不存在的关键字命中 %d 部（须 0）"
   % (word, len(d5), expect, leak[:3], len(ctl_items)),
   s5 == 200 and len(d5) >= 1 and int(expect) >= 1 and not leak
   and len(ctl_items) == 0,
   "http=%s code=%s total=%s" % (s5, (r5 or {}).get("code"),
                                 ((r5 or {}).get("data") or {}).get("total")))

print("== IV6 排序白名单 ==")
s6, r6 = call("GET", "/works?limit=20&sort=torrents", token=tok)
seq = [w.get("torrents") for w in ((r6 or {}).get("data") or
                                   {}).get("items") or []]
desc_ok = all(seq[i] >= seq[i + 1] for i in range(len(seq) - 1))
s6b, r6b = call("GET", "/works?limit=3&sort=;drop%20table%20users--",
               token=tok)
ok("IV6 sort=torrents 返回种子数单调不增（%s）；非法 sort 不 500 也不注入"
   "（http=%s code=%s）" % (desc_ok, s6b, (r6b or {}).get("code")),
   len(seq) >= 3 and desc_ok and s6b == 200,
   str(r6b)[:90])

print("== IV7 卡片里的种子行都过审 ==")
ids = [sd.get("id") for w in items for sd in (w.get("seeds") or [])
       if sd.get("id") is not None]
lst = ",".join(str(int(i)) for i in ids)
notapproved = sval("SELECT count(*) FROM torrents WHERE id IN (%s) "
                   "AND approval_status <> 1" % (lst or "0"))
ok("IV7 本轮卡片挂了 %d 条种子行，其中未过审/被删的 %s 条（须 0）"
   % (len(ids), notapproved), len(ids) >= 1 and notapproved == "0",
   "聚合口比列表口宽＝把待审种随作品带出去")

print("== IV8 前端读者在册 ==")
readers = []
for rel in [os.path.join("apps", "web", "app", "(main)", "torrents",
                         "_parts", "torrents-works.tsx"),
            os.path.join("apps", "web", "app", "(main)", "works", "[id]",
                         "page.tsx")]:
    p = os.path.join(ROOTDIR, rel)
    txt = open(p, encoding="utf-8", errors="replace").read() if \
        os.path.exists(p) else ""
    if "/api/v1/works" in txt:
        readers.append(rel.replace("\\", "/"))
ok("IV8 两个读者都在调 /api/v1/works：%s" % readers, len(readers) == 2,
   "少了哪个＝后端字段变了而那页还在按旧形状渲染，或直接成了死口")

print("== IV9 分页不重叠、极端页码不打 500 ==")
p1 = call("GET", "/works?limit=5&page=1", token=tok)
p2 = call("GET", "/works?limit=5&page=2", token=tok)
k1 = [w.get("key") for w in ((p1 or [None, {}])[1].get("data") or
                             {}).get("items") or []]
k2 = [w.get("key") for w in ((p2 or [None, {}])[1].get("data") or
                             {}).get("items") or []]
overlap = sorted(set(k1) & set(k2))
# 极端页码：旧式手写 offset 会绕成负数 ⇒ PG「OFFSET must not be negative」500
pbig = call("GET", "/works?limit=100&page=999999", token=tok)
ok("IV9 page1/page2 无重叠键（重叠=%s，各 %d/%d 条），page=999999 不 500"
   "（http=%s）" % (overlap[:3], len(k1), len(k2), pbig[0]),
   not overlap and pbig[0] == 200,
   str(pbig[1])[:90])

print("== IV10 作品键归一化 ==")
def keys_for(names):
    rows = ",".join("('%s')" % n.replace("'", "''") for n in names)
    sql = ("WITH t AS (SELECT v.name, NULL::bigint AS group_id, "
           "NULL::text AS imdb_id, '{}'::jsonb AS media_info "
           "FROM (VALUES %s) v(name)) "
           "SELECT string_agg((%s), ',') FROM t;" % (rows, WK))
    return sval(sql).split(",")
same = keys_for([
    "【官种】Movie 2024 1080p",
    "Movie.2024.1080p.WEB-DL",
    "Movie 2024 1080p WEB-DL",
    "Movie (2024) [1080p]",
])
bracket = keys_for(["【官种】"])[0]
imdb_sql = ("WITH t AS (SELECT v.name, NULL::bigint AS group_id, v.imdb_id, "
            "'{}'::jsonb AS media_info FROM (VALUES ('Movie', NULL::text), "
            "('Movie2', ''), ('Movie3', 'TT1234567')) "
            "v(name, imdb_id)) SELECT count(*) FILTER "
            "(WHERE (" + WK + ") LIKE 'i%') || '/' || count(*) FROM t;")
imdb_cnt = sval(imdb_sql)
ok("IV10 前导括号/空格/点号/尾括号归一为同一键（%s）；纯标签名回退非空键"
   "（%s，len=%d）；空 IMDb 不落裸 i（%s）"
   % (same, bracket, len(bracket), imdb_cnt),
   len(set(same)) == 1 and len(same) == 4
   and bracket.startswith("n") and len(bracket) == 33
   and imdb_cnt == "1/3",
   "same=%s bracket=%s imdb=%s" % (same[:3], bracket, imdb_cnt))

summary()
