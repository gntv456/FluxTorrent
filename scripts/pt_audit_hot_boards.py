# -*- coding: utf-8 -*-
"""常驻闸门：0424 内容热榜（P1-1）+ 作品订阅（P1-2）的口径

2026-10-27 深测在这两个新功能里抓到五个形状，全部当场修完，本闸门钉住：

  HB1 三榜都要「有活种 + 榜值>0」
      首版 today/week 只算 val 不筛 val ⇒ 今天没人完成下载时，榜被 val=0 的
      老种（最低 id）填满（现网实测：旧口径 10 行里 9 行 val=0，榜首是
      id=40 的回归种）；首版 free 用 `t.seeders >= 0`（恒真）⇒ 0 做种的死种
      照样上榜，点进去只有「无种子」。
  HB2 热榜行必须链到详情真实路由 `/torrent/{id}`
      首版写成 `/torrents/{id}`（实测 404，`/torrent/{id}` 才 200）
      ⇒ 榜上每一行点进去都是死链。
  HB3 订阅关键词必须是**归一化标题**，不是某颗种的全名
      本站种名形如「片名｜4K HDR｜类型…」，作品内各版本只在 ｜ 之后不同；
      用全名做 ILIKE ⇒ 同作品另一版本与以后新集数永不命中（实测旧词命中
      1/2、新词 2/2），「订阅新作」形同虚设。
  HB4 订阅口必须走作品键单源（valid_work_key + WK 表达式）
      首版用 `trim_start_matches(['g','i','m','d'])` 猜键：n 键永远匹配不到
      （实测 GET 作品 200、POST 订阅 404），m/d 数字键还会与 group_id 撞车。
  HB5 详情口回读 watching + 订阅口挂 wishlist 模块闸
      否则刷新后按钮退回「订阅新作」（用户以为没订上会反复点），
      且模块关闭时仍写得进愿望单（写了管不掉）。

判据本体一律**从源码正则取出**（Rust 里的 SQL 串与 format! 模板），直接在
现网库上跑——对「镜像还没重建」不敏感，验的是代码本体而不是旧进程。
HB1e/HB3b 是同轮正控：拿旧形状跑同一条数据量出缺陷读数，证明判据承重。
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import ok, psql_stdin, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WEB = os.path.join(ROOT, "apps", "web")


def rd(*parts):
    return open(os.path.join(ROOT, *parts), encoding="utf-8",
                errors="replace").read()


def read_abs(p):
    return open(p, encoding="utf-8", errors="replace").read()


def unquote(raw):
    """Rust 字面量 → 纯文本（吃行续接，剥引号/转义）。"""
    raw = re.sub(r"\\\r?\n\s*", "", raw.strip())
    if raw.startswith('r#"') and raw.endswith('"#'):
        raw = raw[3:-2]
    elif raw.startswith('"') and raw.endswith('"'):
        raw = raw[1:-1]
    return raw.replace('\\"', '"')


def const_str(src, name):
    m = re.search(r"const %s: &str =\s*(.*?);\s*\n" % name, src, re.S)
    return unquote(m.group(1)) if m else ""


def fmt_tpl(src, var):
    m = re.search(r"let %s = format!\(\s*\"(.*?)\"" % var, src, re.S)
    if m:
        return unquote('"%s"' % m.group(1))
    return tpl_after(src, "let %s" % var)


def tpl_after(src, marker):
    """取 marker 之后第一个字符串字面量：仓里多是
    `let x: T = sqlx::query_scalar(&format!("..."))` 的写法，
    正则只认 `let x = format!(` 会整条判据打空（首版就栽在这）。
    """
    i = src.find(marker)
    if i < 0:
        return ""
    m = re.search(r'"((?:[^"\\]|\\.)*)"', src[i:i + 1600], re.S)
    return unquote('"%s"' % m.group(1)) if m else ""


def macro_body(src, name):
    m = re.search(r"macro_rules! %s \{\s*\(\) => \{\s*(.*?)\s*\}\s*;" % name,
                  src, re.S)
    return unquote(m.group(1)) if m else ""


def fn_str(src, name):
    """`fn name() -> &'static str { r#"..."# }` 的字符串本体。

    2026-10-29 重构后 works_sql 的 title_expr 是函数不是宏；只认宏会让
    HB0 整支打空（首版就栽在这）。
    """
    m = re.search(r"fn %s\(\)\s*->\s*&'static str\s*\{(.*?)\n\}" % name,
                  src, re.S)
    return unquote(m.group(1).strip()) if m else ""


def lit(s):
    return "'" + s.replace("'", "''") + "'"


def fill(tpl, *parts):
    out = tpl
    for p in parts:
        out = out.replace("{}", p, 1)
    return out


def rows(sql, cols="z.id, z.val, z.seeders"):
    out = []
    for ln in psql_stdin("SELECT %s FROM (%s) z;" % (cols, sql)).splitlines():
        p = [x.strip() for x in ln.split("|")]
        if p and p[0].isdigit():
            out.append(tuple(p))
    return out


HOT = rd("apps", "api", "src", "content_http", "top_torrents.rs")
WATCH = rd("apps", "api", "src", "torrent_http", "works_watch.rs")
WSQ = rd("apps", "api", "src", "torrent_http", "works_sql.rs")
WORKS = rd("apps", "api", "src", "torrent_http", "works.rs")
COMP = rd("apps", "web", "components", "top-torrents.tsx")
BTN = rd("apps", "web", "components", "work-watch-button.tsx")
PAGE = rd("apps", "web", "app", "(main)", "works", "[id]", "page.tsx")
DTYPES = rd("packages", "domain-types", "src", "index.ts")

HEAD = const_str(HOT, "LIST_SQL_HEAD")
VIS = const_str(HOT, "VIS")
TODAY = const_str(HOT, "TODAY_VAL")
WEEK = const_str(HOT, "WEEK_VAL")
TITLE = fn_str(WSQ, "title_expr") or macro_body(WSQ, "title_expr")
WK = const_str(WSQ, "WK")
Q_TODAY = fill(fmt_tpl(HOT, "today_q"), HEAD, TODAY, VIS)
Q_WEEK = fill(fmt_tpl(HOT, "week_q"), HEAD, WEEK, VIS)
T_FREE = fmt_tpl(HOT, "free_q")
Q_FREE = fill(T_FREE, HEAD, VIS)
# 2026-10-29：订阅取词/回读都收敛到 works.rs 的 keyword_for 与详情口，
# 不再各写一份 SQL；锚点跟着实现搬家。
W_TPL = tpl_after(WORKS, "pub(super) async fn keyword_for")
V_TPL = tpl_after(WORKS, "let watching: bool = match keyword")

print("== HB0 源码取件（取不到＝判据打空，绝不静默跳过）==")
PIECES = dict(HEAD=HEAD, VIS=VIS, TODAY=TODAY, WEEK=WEEK, TITLE=TITLE, WK=WK,
              watch=W_TPL, watching=V_TPL)
ok("HB0 八段本体都从源码取到", all(PIECES.values()),
   " ".join("%s=%d" % (k, len(v)) for k, v in PIECES.items()))
if not all(PIECES.values()):
    summary()
    sys.exit(1)

print("== HB1 三榜形状 + 现网读数 ==")
ok("HB1a 共用谓词只收活种（不再写 seeders >= 0）",
   "t.seeders > 0" in VIS and "t.seeders >= 0" not in VIS, VIS[:100])
ok("HB1b today/week 外层筛 val>0（否则零值行灌榜）",
   "q.val > 0" in fmt_tpl(HOT, "today_q")
   and "q.val > 0" in fmt_tpl(HOT, "week_q"))
bal = {"today": Q_TODAY, "week": Q_WEEK, "free": Q_FREE}


def try_sql(sql):
    """跑一条 SQL，出错返回 None（让 PG 判语法，不数文本括号——
    正则串里 `\\(` `\\)` 与 `('free','x2free')` 会让文本计数天然不配平）。
    """
    try:
        return psql_stdin(sql)
    except RuntimeError:
        return None


parsed = {k: try_sql("EXPLAIN " + v) is not None for k, v in bal.items()}
parsed["WK"] = try_sql("SELECT count(*) FROM torrents t WHERE ("
                       + WK + ") IS NOT NULL;") is not None
parsed["TITLE"] = try_sql("SELECT count(*) FROM torrents t WHERE ("
                          + TITLE + ") IS NOT NULL;") is not None
ok("HB1c 三条榜 SQL 与 WK/TITLE 片段都能被 PG 解析（少括号即红）",
   all(parsed.values()), "解析失败=%s" % [k for k, v in parsed.items() if not v])
bd = rows(Q_TODAY) + rows(Q_WEEK) + rows(Q_FREE)
ok("HB1d 三榜逐行 val>0 且 seeders>0",
   all(int(v) > 0 and int(s) > 0 for _, v, s in bd),
   "违规=%s" % [(i, v, s) for i, v, s in bd if int(v) <= 0 or int(s) <= 0])
LEGACY = ("SELECT t.id, " + TODAY + " AS val, t.seeders FROM torrents t "
          "WHERE t.approval_status = 1 AND t.seeders >= 0 "
          "ORDER BY val DESC, t.id LIMIT 10")
lg = rows(LEGACY)
zero = [i for i, v, _ in lg if int(v) == 0]
deadn = len([1 for _, _, s in lg if int(s) == 0])
print("   [正控读数] 旧口径 10 行：val=0 共 %d 行、0 做种共 %d 行；新口径 %d 行"
      % (len(zero), deadn, len(rows(Q_TODAY))))
ok("HB1e 正控：旧口径真会灌进 val=0 或死种（判据不空转）",
   bool(zero) or deadn > 0, "库里若已无此类种，这条会红＝仍然承重")

print("== HB2 榜单行链接与全仓详情路由 ==")
ok("HB2a 热榜行链向 /torrent/{id}（单数才是详情路由）",
   "/torrent/${t.id}" in COMP.replace("`", ""))
bad = []
pat = re.compile(r"(href|push)\(\s*[`\"']/torrents/\$\{")
for dp, dirs, fs in os.walk(WEB):
    dirs[:] = [d for d in dirs if d not in ("node_modules", ".next")]
    for fn in fs:
        if fn.endswith((".tsx", ".ts")) and pat.search(read_abs(
                os.path.join(dp, fn))):
            bad.append(os.path.relpath(os.path.join(dp, fn), ROOT))
ok("HB2b 全仓不再出现 /torrents/${id} 型详情死链", not bad,
   "命中=%s" % [b.replace("\\", "/") for b in bad])

print("== HB3/HB4 作品订阅：键单源 + 归一化标题 ==")
ok("HB4a 订阅口用 valid_work_key 白名单（不再 trim_start_matches 猜键）",
   "valid_work_key" in WATCH and "trim_start_matches" not in WATCH)
ok("HB4b 订阅口按作品键单源取种（WK = $1）",
   "AND {wk} = $1" in W_TPL and "group_id::text = $1" not in W_TPL)
ok("HB3a 关键词取归一化标题（title_expr），空才回落全名",
   "COALESCE(NULLIF({title}, ''), t.name)" in W_TPL
   and "title = works_sql::title_expr()" in WORKS)
ok("HB4c 键白名单只定义一处、详情与订阅两处都调它",
   WORKS.count("fn valid_work_key") == 1
   and WORKS.count("valid_work_key(&key)") == 1
   and "super::works::valid_work_key(&key)" in WATCH,
   "定义=%d 详情=%d 订阅=%d"
   % (WORKS.count("fn valid_work_key"), WORKS.count("valid_work_key(&key)"),
      WATCH.count("valid_work_key(&key)")))
ok("HB5a 订阅口挂 wishlist 模块闸",
   "require_module(crate::modules::key::WISHLIST)" in WATCH)
ok("HB5b 详情口回读 watching、契约与按钮吃初值",
   '"watching": watching' in WORKS and "watching: boolean" in DTYPES
   and "initialWatching" in BTN and "initialWatching={data.watching}" in PAGE)


def watch_sql(key):
    return (W_TPL.replace("{title}", TITLE, 1).replace("{wk}", WK, 1)
            .replace("$1", lit(key)))


keys = {}
for k in psql_stdin("SELECT DISTINCT (%s) FROM torrents t "
                    "WHERE t.approval_status = 1 LIMIT 500;" % WK).splitlines():
    if re.match(r"^[gimdn][A-Za-z0-9]{1,40}$", k):
        keys.setdefault(k[0], k)
print("   [键样本] %s" % keys)
emptyk = [p for p, k in sorted(keys.items())
          if psql_stdin(watch_sql(k)) == ""]
ok("HB4d 逐键跑订阅口取词都能取到（旧实现 n 键直接 404）",
   bool(keys) and not emptyk, "样本=%s 取不到=%s" % (list(keys), emptyk))

rep = psql_stdin(
    "WITH s AS (SELECT t.name, (" + WK + ") AS wk FROM torrents t "
    "WHERE t.approval_status = 1), g AS (SELECT wk FROM s GROUP BY wk "
    "HAVING count(*) > 1 AND max(name) LIKE '%｜%' LIMIT 1) "
    "SELECT name FROM s WHERE wk = (SELECT wk FROM g) ORDER BY name DESC "
    "LIMIT 1;")
if not rep:
    print("   WARN 现网没有「同作品多版本且带｜副题」样本：HB3b 本轮无据可验")
else:
    norm = psql_stdin("SELECT " + TITLE.replace("t.name", "q.name")
                      + " FROM (SELECT " + lit(rep) + " AS name) q;")
    ab = psql_stdin(
        "SELECT (SELECT count(*) FROM torrents t WHERE t.approval_status = 1 "
        "AND t.name ILIKE '%' || " + lit(norm) + " || '%') AS new_hit, "
        "(SELECT count(*) FROM torrents t WHERE t.approval_status = 1 "
        "AND t.name ILIKE '%' || " + lit(rep) + " || '%') AS old_hit;")
    nums = [int(x) for x in ab.split("|") if x.strip().isdigit()]
    print("   [HB3b 正控读数] 全名=%r 归一化=%r 新词命中=%s 旧词命中=%s"
          % (rep[:34], norm[:34], nums[0] if nums else "?",
             nums[1] if len(nums) > 1 else "?"))
    ok("HB3b 归一化标题命中严格多于全名（旧词验不到同作品另一版本）",
       len(nums) == 2 and nums[0] > nums[1] and norm != rep, ab[:120])

print("== HB5 订阅状态回读（事务内 A/B，收尾 ROLLBACK）==")
probe_key = keys.get("n") or (sorted(keys.values())[0] if keys else "")
probe_name = psql_stdin(watch_sql(probe_key)) if probe_key else ""
if not probe_name:
    ok("HB5c 探针作品可用", False, "key=%s 取不到代表名" % probe_key)
else:
    pnorm = psql_stdin("SELECT " + TITLE.replace("t.name", "q.name")
                       + " FROM (SELECT " + lit(probe_name) + " AS name) q;")
    exists = (V_TPL.replace("{title}", TITLE, 1).replace("$1", "1::bigint")
              .replace("$2", lit(probe_name)))
    out = psql_stdin(
        "BEGIN;\n"
        "DELETE FROM wishlist WHERE user_id = 1 AND keyword = "
        + lit(pnorm) + ";\n"
        "SELECT 'OFF|' || (" + exists + ");\n"
        "INSERT INTO wishlist (user_id, keyword) VALUES (1, " + lit(pnorm)
        + ");\n"
        "SELECT 'ON|' || (" + exists + ");\n"
        "ROLLBACK;\n"
        "SELECT 'RESIDUE|' || (SELECT count(*) FROM wishlist WHERE user_id = 1"
        " AND keyword = " + lit(pnorm) + ");\n")
    print("   [HB5 读数] %s" % out.replace("\n", "  "))
    ok("HB5c 未订→f、写入归一化词后→t、回滚零残留",
       "OFF|f" in out and "ON|t" in out and "RESIDUE|0" in out, out[:160])

summary()
