# -*- coding: utf-8 -*-
"""闸门 MG：/torrents 列表参数链的**活体行为**核对（批一百九十，2026-10-28）。

MD 那把静态尺只能抓「标识符全仓独一份」的漏接件；筛选参数真正的验收是
**同一支 URL 换参数值打两次比结果集**（本仓 #70 那条 `?limit=100&offset=100`
与 `?limit=100` 回一模一样的 100 行＝收了不用，HTTP 层面看不出来）。
这条闸门把种子列表最常用的几档参数钉成行为断言，并用 psql 取同一份事实交叉
验证——界面上报的数与库里的数必须同源。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_mg_list_params_live.py
（纯 GET，零写入；限流按 root 档，8 次调用不会撞 429）

  MG1 前置：能取到读数（token 失效/服务不可用要当场红，不许空对空）
  MG2 恒等式：include_dead 视图总数 == 默认视图 total_estimate + hidden_dead
  MG3 排序真换序：sort=size 的首位与库里按 size 的第一名同源
  MG4 升降两档：sort=<col> 与 sort=<col>_asc 给出不同序列（`_asc` 不是装饰）
  MG5 谓词真生效：date_from=未来 ⇒ 0 条；search=不存在的词 ⇒ 0 条
  MG6 游标双向：p1→c1→p2，再 (c1,dir=prev) 必须回 p1、(c2,dir=prev) 必须回 p2
  MG7 limit 与 lenient：?limit=1 只回 1 条；?limit=abc 按既有口径回 200（AF G10）
"""
import os
import sys
from urllib.parse import quote

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, jwt_token, dig, psql_stdin, ok, summary

BASE = "/torrents?limit=5&include_dead=1"


def page(path, tok):
    s, r = call("GET", path, None, tok)
    d = (r or {}).get("data") if isinstance(r, dict) else None
    d = d or {}
    return s, [x.get("id") for x in (d.get("items") or [])], d


tok = jwt_token("root")
s0, p1, d1 = page(BASE, tok)
ok("MG1 前置：/torrents(include_dead) 取到读数且非空",
   s0 == 200 and len(p1) > 0, "http=%s 条数=%d" % (s0, len(p1)))

s_def, _pd, d_def = page("/torrents?limit=5", tok)
est = int(d_def.get("total_estimate") or 0)
hid = int(d_def.get("hidden_dead") or 0)
full = int(d1.get("total_estimate") or 0)
ok("MG2 恒等式：全量视图总数 == 默认视图(存活估计 + 被存活谓词藏起)",
   full == est + hid, "全量=%d 默认=%d + 隐藏=%d" % (full, est, hid))

top_sql = psql_stdin(
    "SELECT id FROM torrents WHERE approval_status = 1 "
    "ORDER BY size DESC NULLS LAST, id DESC LIMIT 1;").splitlines()[0].strip()
s_sz, psz, _ = page(BASE + "&sort=size", tok)
ok("MG3 sort=size 的首位与库内同序（不是界面自己排的假序）",
   psz and str(psz[0]) == top_sql,
   "接口首位=%s 库里首位=%s" % (psz[:1], top_sql))

s_asc, pasc, _ = page(BASE + "&sort=size_asc", tok)


def size_of(i):
    """取某条的 size（NULL 记 -1，按「最小的那一头」理解）。"""
    if not i:
        return None
    v = psql_stdin("SELECT COALESCE(size, -1) FROM torrents WHERE id = %s"
                   % int(i)).splitlines()[0].strip()
    return int(v) if v.lstrip("-").isdigit() else None


a_sz, d_sz = size_of(pasc and pasc[0]), size_of(psz and psz[0])
ok("MG4 升降两档真的两档：sort=size_asc 首位不得比 sort=size 首位更大",
   bool(pasc) and pasc != psz and a_sz is not None and d_sz is not None
   and a_sz <= d_sz,
   "asc 首3=%s(size=%s) desc 首3=%s(size=%s)"
   % (pasc[:3], a_sz, psz[:3], d_sz))
# MG4b＝MG4 的承重证明：`_asc` 若被人改成「解析了但不进 ORDER BY」，
# asc 那一页会退回默认序——只比「asc 首位 <= desc 首位」会被默认序蒙对，
# 所以必须再钉一条「asc 页 != 默认页」。
ok("MG4b 承重：sort=size_asc 的序列必须不同于默认排序（防 _asc 变成装饰）",
   pasc != p1, "asc 首3=%s 默认首3=%s" % (pasc[:3], p1[:3]))

s_dt, pdt, _ = page(BASE + "&date_from=2030-01-01", tok)
s_se, pse, _ = page(BASE + "&search=zzz_not_present_zz", tok)
ok("MG5 谓词生效：未来日界与不存在的关键词都必须清空结果",
   s_dt == 200 and not pdt and s_se == 200 and not pse,
   "date_from 命中=%d search 命中=%d" % (len(pdt), len(pse)))

c1 = d1.get("next_cursor")
s2, p2, d2 = page("%s&cursor=%s" % (BASE, c1), tok)
_s, b1, _d = page("%s&cursor=%s&dir=prev" % (BASE, c1), tok)
_s, b2, _d = page("%s&cursor=%s&dir=prev" % (BASE, d2.get("next_cursor")), tok)
ok("MG6 游标双向：dir=prev 必须回到上一页（不是原地再发一次正向）",
   bool(p2) and b1 == p1 and b2 == p2,
   "p2=%s prev(c1)=%s prev(c2)==p2=%s"
   % (p2[:3], b1[:3], b2 == p2))

s_l1, pl1, _ = page("/torrents?limit=1&include_dead=1", tok)
s_lb, r_lb = call("GET", "/torrents?limit=abc&include_dead=1", None, tok)
dl = (r_lb or {}).get("data") or {}
ok("MG7 limit 消费 + 坏值按既有 lenient 口径放行（AF G10 的口径不许飘）",
   s_l1 == 200 and len(pl1) == 1 and s_lb == 200
   and int(dl.get("total_estimate") or 0) > 0,
   "limit=1 回%d 条；limit=abc http=%s total=%s"
   % (len(pl1), s_lb, dl.get("total_estimate")))

# —— MG8 搜索字面量对账（活体；本轮曾据此误立案 P-458，后撤案，见下三条尺子错）——
# 文档写着「关键字（AND 模糊，%/_ 已转义）」，代码也对：`esc_like()` 把
# `%`/`_`/`\` 都换过（torrents/list.rs:13），`mk_pat` 两个模式都走它。
# 但现网 `?search=_` 回 **25 条**，而被这条谓词读的四列里**一个字面下划线都没有**
# （name/small_descr/descr 各 0，files.path 也 0；`site_terms` 空表⇒同义词支路
# 根本没参与；`?search=%` 与 `?search=1_8` 都回 0，所以也不是「整条没转义」）。
# 机制没定位，就把**「接口条数 == 库内字面命中数」**这条不变量钉住：
# 红就是有人在按文档语义搜字面量时拿到不相干的结果，绿才是真的对齐。
# 谓词必须与后端**同一把尺**：后端用 ILIKE（大小写不敏感）+ `ESCAPE chr(92)`。
# 另两条本轮实犯的尺子错，写死在这里防重犯：
#   ① **LIKE 没有字符类**！`'%[_]%'` 匹配的是字面三字符 "[_]"，不是「一个字面下划线」
#      ——我据此得出「全库 0 行含下划线」，其实 `pt_audit_alive.*` 这类名字满屏都是
#      下划线。字面量比对要用 `LIKE '$_$' ESCAPE '$'`（选 `$` 是为躲开 psql 行内反斜杠
#      当元命令的老坑）。
#   ② 要比**接口返回的 id 集合逐行验真**，不能拿 `total_estimate` 当精确数对账。
LIKE_COLS = ("t.name ILIKE {p} OR t.small_descr ILIKE {p} "
             "OR t.descr ILIKE {p} OR EXISTS (SELECT 1 FROM files f "
             "WHERE f.torrent_id = t.id AND f.path ILIKE {p})")
CASES = [  # (搜索串, 库内「字面命中」的 LIKE 表达式；ESCAPE '$' 只转义紧跟的一字符)
    ("%", "'%$%%'"),
    ("_", "'%$_%'"),
    ("1_8", "'%$1$_$8%'"),
    ("Audit", "'%Audit%'"),
    ("audit", "'%audit%'"),
]
ESC = " ESCAPE '$'"
bad = []
for raw, pat in CASES:
    s9, r9 = call("GET", "/torrents?limit=60&include_dead=1&search="
                  + quote(raw), None, tok)
    d9 = (r9 or {}).get("data") or {}
    got = [int(x.get("id")) for x in (d9.get("items") or [])]
    inlist = ",".join(str(i) for i in got) or "-1"
    where = LIKE_COLS.format(p=pat + ESC)
    not_match = int(psql_stdin(
        "SELECT count(*) FROM torrents t WHERE t.id IN (%s) "
        "AND NOT (%s);" % (inlist, where)).splitlines()[0].strip())
    truth = int(psql_stdin(
        "SELECT count(*) FROM torrents t WHERE t.approval_status = 1 AND ("
        + LIKE_COLS.format(p=pat + ESC) + ");").splitlines()[0].strip())
    flag = "[OK] " if not_match == 0 else "[红] "
    bad.append("%s%s: 返回%d条 其中不含该词的=%d 库内字面命中=%d"
               % (flag, raw, len(got), not_match, truth))
print("[读数] 搜索字面量对账（详见下一条断言）")
for b in bad:
    print("    " + b)
ok("MG8 搜索必须按字面量命中（红=用户搜到的行里混有不含该词的记录）",
   all(x.startswith("[OK] ") for x in bad), " | ".join(bad))

summary()
