# -*- coding: utf-8 -*-
"""CY 闸门：分页越界 / 计数谎报 / 列类型宽度（P-333，2026-10-19）。

三条独立缺陷，全部先有 PG 级真凭（现网 :8080 容器日志原文）：

A) 成员「我的种子」整页 500（GET /me/torrentlist）
   `internal error error=... decoding column "pending": mismatched types;
    Rust type i64 (as SQL type INT8) is not compatible with SQL type INT4`
   根因：stats SQL 的非 uploads 分支写 `0 AS pending` —— 字面 0 在 PG 里是 int4，
   而 ListStats 按 i64 读。uploads 分支用 count(*)（int8）所以不炸 ⇒
   **默认 kind（做种中）与 completed 两支恒定 500**，前端
   components/torrent-list-client.tsx 每次进页面都打这一口。
   修法：字面量写 `0::bigint`。

B) 管理端「系统日志」的总数是谎话（GET /admin/syslog）
   PG：`bind message supplies 4 parameters, but prepared statement "sqlx_s_54"
        requires 5`，语句正是那条 `SELECT count(*) FROM runtime_logs WHERE ...`
   根因：where_sql 是「列表与计数同一谓词」共用的一段，里面引用 $5（instance）；
   计数口按 $1..$4 顺序 bind ⇒ 参数对不上，而 `.unwrap_or(0)` 把错误吞成 0 ⇒
   列表有 50 行、界面「共 0 条」、页码算不出来（失败演成空数据）。
   修法：计数口按 $1..$5 补齐 $3/$4 占位（列表的 LIMIT/OFFSET 用不到也要占位）。

C) 极端页码打成 500（OFFSET 绕负）
   PG：`ERROR: OFFSET must not be negative`，实测 /artists、/subtitles、/preserve
   在 `?page=9223372036854775807` 下全部 500；根因是手写 `(page - 1) * per`
   溢出。dto.rs:26 的 page_window 注释早就写了「旧式写法在极端参数下会绕负」，
   但仓里还有一批口是手写的。
   修法：新增 `dto::page_offset(page, per)`（page 夹 1..=1e6、per 由调用方自己夹，
   乘积 saturating），19 处手写点全部改走它。

判据：静态普查（A/B/C 各自的形状）+ 活体（旧镜像红 = 负控，侧进程绿 = 修复）。
静态正控钉在 76f8adc5（本批修复的前一个提交），必须报出全部三类形状。
"""
import glob
import os
import re
import subprocess
import sys

sys.path.insert(0, "scripts")
from pt_audit_lib import call, login, ok, dig, FAILURES  # noqa: E402

API = "apps/api/src"
CTRL_REV = "76f8adc5"
# C 类：手写 offset 的形状（`(… - 1) * 变量`），dto.rs 的助手本体除外
HANDOFF_RE = re.compile(
    r"\(\s*[\w.]+(?:\.unwrap_or\(1\))?(?:\.max\(1\)|"
    r"\.clamp\([^)]*\))?\s*-\s*1\s*\)\s*\*\s*[\w]+"
)
ALLOW = {
    # compat 层把负数 cast 成 usize ⇒ 变成「跳过海量行」＝空页，不打 500
    "compat_http/nexusphp.rs": "已核实：负 offset 绕成巨大 usize，退化为空页不是 500",
    # 非分页公式（等级→经验值），正则只是撞上了同样的形状
    "games_http/pets.rs": "不是分页：等级换经验值的算式",
}
# 活体只读探针用的口
PAGin = "9223372036854775807"
# 活体只读探针用的口：(路由, 这支是否真的有 page 参数)
# /me/bookmarks 只有 limit（后端截 100，data-content.ts:115 的注释自己写着
# 「后端按 limit 截断且没有分页」）⇒ 对它断言「换页要变空页」是判据写错，
# 该口另列为「上限无翻页」的待裁项，不进 C 类账。
LIST_EPS = [("/users", True), ("/artists", True), ("/forums/mine", True),
            ("/forums/1/topics", True), ("/subtitles", True),
            ("/me/orders", True), ("/me/spark/ledger", True),
            ("/ban-log", True), ("/me/hr", True), ("/me/torrentlist", True),
            ("/me/bookmarks", False), ("/invites/invitees", True),
            ("/invites", True), ("/preserve", True), ("/admin/syslog", True),
            ("/admin/locations", True),
            ("/admin/settings/history?name=site_type", True)]


def sh(args):
    return subprocess.run(args, capture_output=True, encoding="utf-8",
                          errors="replace")


def read(rel, rev=None):
    if rev:
        o = sh(["git", "show", "%s:%s" % (rev, rel)])
        return o.stdout if o.returncode == 0 else None
    try:
        with open(rel, encoding="utf-8") as f:
            return f.read()
    except OSError:
        return None


def handoff_hits(rev=None):
    out = []
    # 模式以 `-` 开头，必须用 -e 传，否则 git 把它当命令行选项 ⇒
    # 什么都不匹配，普查会「恒真通过」（本轮第一次就是这样假绿）
    pat = ["git", "grep", "-l", "-E", "-e", r"- 1\) \*"]
    if rev:
        pat.append(rev)
    pat += ["--", API]
    o = sh(pat)
    for line in o.stdout.splitlines():
        rel = line.split(":", 1)[1] if rev and ":" in line else line
        src = read(rel, rev)
        if src is None or not rel.endswith(".rs"):
            continue
        if rel.endswith("dto.rs"):
            continue
        key = rel.replace(API + "/", "")
        if any(key.endswith(a) for a in ALLOW):
            continue
        for i, ln in enumerate(src.split("\n")):
            if HANDOFF_RE.search(ln):
                out.append((key, i + 1, ln.strip()[:52]))
    return out


print("=" * 68)
print("一、静态普查（A 列类型 / B 计数占位 / C 手写 offset）")
print("=" * 68)
c_now = handoff_hits()
ok("CY1 没有未登记的手写 offset（极端页码绕负的形状）", not c_now,
   "%d 处：%s" % (len(c_now), [h[0] for h in c_now][:6]))
mh = read(API + "/http/misc_handlers.rs") or ""
ok("CY2 我的种子 stats SQL 的三档计数已按 bigint 给（A 类形状不再出现）",
   " 0 AS pending" not in mh and mh.count("0::bigint AS pending") >= 2,
   "还剩 %d 处 `0 AS pending`" % mh.count(" 0 AS pending"))
sd = read(API + "/staff_http/site_db.rs") or ""
syslog_block = sd.split("SELECT count(*) FROM runtime_logs")[1:2]
syslog_block = syslog_block[0].split("fetch_one")[0] if syslog_block else ""
ok("CY3 系统日志计数口按 $1..$5 补齐占位（B 类参数错位不再出现）",
   syslog_block.count(".bind(") >= 5,
   "计数块只有 %d 个 bind，而 where_sql 引用到 $5"
   % syslog_block.count(".bind("))
ok("CY4 page_offset 助手在册且带 saturating",
   "pub fn page_offset" in (read(API + "/dto.rs") or "")
   and "saturating_mul" in (read(API + "/dto.rs") or ""),
   "dto.rs 里没找到助手")

c_ctl = handoff_hits(CTRL_REV)
mh_ctl = read(API + "/http/misc_handlers.rs", CTRL_REV) or ""
ok("CY5 正控（%s）：三类形状当时都在册（判据承重）" % CTRL_REV,
   len(c_ctl) >= 10 and " 0 AS pending" in mh_ctl,
   "手写 offset %d 处 / stats 里 `0 AS pending` %s"
   % (len(c_ctl), " 0 AS pending" in mh_ctl))
print("   普查基数：正控层手写 offset %d 处，工作树 %d 处" % (len(c_ctl), len(c_now)))

print()
if os.environ.get("CY_SKIP_LIVE"):
    print("（CY_SKIP_LIVE=1：跳过活体层）")
    print("红账：%s" % (FAILURES or "无"))
    sys.exit(1 if FAILURES else 0)
BASE = os.environ.get("FLUX_API_BASE", "http://127.0.0.1:8080/api/v1")
print("=" * 68)
print("二、活体只读 A/B（打的是 %s）" % BASE)
print("=" * 68)


def rows_of(r):
    d = dig(r, "data")
    if isinstance(d, dict):
        for k in ("rows", "items", "list"):
            if isinstance(d.get(k), list):
                return d[k], d.get("total")
        return [], d.get("total") if isinstance(d, dict) else None
    if isinstance(d, list):
        return d, None
    return [], None


tok = login()
five00 = []
badtotal = []
for ep, paged in LIST_EPS:
    sep = "&" if "?" in ep else "?"
    s0, r0 = call("GET", ep, None, tok)
    s1, r1 = call("GET", ep + sep + "page=" + PAGin, None, tok)
    s2, r2 = call("GET", ep + sep + "page=-7", None, tok)
    rows0, total0 = rows_of(r0)
    rows1, _ = rows_of(r1)
    tag = "%-34s base=%s/%d条/total=%s  MAX=%s/%d条  neg=%s" % (
        ep, s0, len(rows0), total0, s1, len(rows1), s2)
    if s0 >= 500 or s1 >= 500 or s2 >= 500:
        five00.append(tag)
        print("   [500] " + tag)
    else:
        print("   " + tag)
    # B 类：列表有行而 total 报 0 ⇒ 那句总数是谎话
    if s0 == 200 and len(rows0) > 0 and isinstance(total0, int) and total0 == 0:
        badtotal.append(tag)
    if not paged:
        print("        ↑ 这支没有 page 参数（只有 limit），换页判据不适用")
        continue
    if s1 == 200:
        ok("CY6 %s 极端页码必须回空页而不是丢行" % ep, len(rows1) == 0,
           "回了 %d 条" % len(rows1))
ok("CY7 任何一支列表口都不许因页码打 500（C 类）", not five00,
   "%d 支：%s" % (len(five00), [t.split()[0] for t in five00]))
ok("CY8 不许出现「列表有行、total 报 0」的谎报（B 类）", not badtotal,
   "%d 支：%s" % (len(badtotal), [t.split()[0] for t in badtotal]))
s9, r9 = call("GET", "/me/torrentlist", None, tok)
ok("CY9 成员「我的种子」必须打得开（A 类恒定 500）", s9 == 200 and r9.get("code") == 0,
   "http=%s code=%s msg=%s" % (s9, r9.get("code"), str(r9.get("message"))[:60]))
s10, r10 = call("GET", "/me/torrentlist?kind=completed", None, tok)
ok("CY9b 同口的 completed 档也必须打得开", s10 == 200 and r10.get("code") == 0,
   "http=%s code=%s" % (s10, r10.get("code")))

print()
print("红账：%s" % (FAILURES or "无"))
sys.exit(1 if FAILURES else 0)
