# -*- coding: utf-8 -*-
"""常驻闸门：首页「30 天新增资源」的分桶轴与标注轴必须是同一个日界（P-465）。

形状：`http/home.rs` 里 `today = Utc::now() + 8h`（**站点日**，函数注释自己写着
「与 /attendance、/checkin 同口径；容器 TZ=UTC 时 Local 会让首页日历在北京
0-8 点窗口显示『昨日未签』」），可 SQL 那侧分桶写的是 `created_at::date`——
PG 会话时区实测 `UTC`，于是每根柱子装的是「本地 08:00 → 次日 08:00」的量，
却挂着本地零点的日期标签。后果按小时分两段：

  * 北京 00:00–08:00：`today` 已经是新的一天，而 UTC 桶里还没有这一天的键 ⇒
    首页「今日新增资源」**恒显示 0**，凌晨（华语 PT 站发种高峰）过的种全挂在
    「昨天」那根柱子上；
  * 其余时段：数字不为 0，但每根柱子都错开 8 小时，「7 日均/30 日计」同样错。

修法：分桶表达式换成与 `today` 同一把尺（仓里既有写法：
`(x AT TIME ZONE 'UTC') + interval '8 hours'`，见 attachment_video_upload.rs:195、
loan_repay.rs:322、games_http/linkage.rs:37）。

判据分层：
  H0 静态：home.rs 的分桶表达式含站点日换算，且裸 `created_at::date AS d` 不在
     SQL 里（**先剥 `//` 注释**——本文件注释正写着那个裸形式，不剥就是假红）
  H1 判据敏感性（与数据无关）：拿四个已知时刻断言两轴给出不同日界，且站点轴
     才是会员看到的那一天（这条红了说明尺子失效，不是站坏了）
  H2 活体：把源码里那段字面 SQL 取出来跑，「今日」桶必须等于独立公式
     `created_at >= 站点今日零点对应的 UTC 时刻` 的行数；同时打印旧轴读数
  H3 台账：首页文件里不得残留任何 `created_at::date`（管理总览 14 天图与个人
     面板 30 天登录趋势是 UTC/UTC **自洽**，不在本闸门断言范围内——那是
     「全站日界统一」口径问题，交站长）
跑法：python -u scripts/pt_audit_home_day_axis.py [--live] [--selftest]
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import ok, psql_stdin, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
HOME_RS = os.path.join(ROOT, "apps", "api", "src", "http", "home.rs")
SITE_DAY = "((created_at AT TIME ZONE 'UTC') + interval '8 hours')::date"
UTC_DAY = "created_at::date"
SITE_TODAY = "((now() AT TIME ZONE 'UTC') + interval '8 hours')::date"


def read_source():
    raw = io.open(HOME_RS, encoding="utf-8", newline="").read()
    return raw.replace("\r\n", "\n")


def strip_line_comments(text):
    out = []
    for line in text.split("\n"):
        i = line.find("//")
        if i >= 0 and line.count('"', 0, i) % 2 == 0:
            line = line[:i]
        out.append(line)
    return "\n".join(out)


def home_sql(text):
    """取出 home.rs 里那段「30 天新增资源」的字面 SQL（Rust 续行拼平）。

    按形状 `SELECT … ORDER BY d` 找，新旧两种分桶写法都取得到——判据必须
    在被翻回旧形状时仍然读得到那段 SQL，否则 H0 的红只是「没读到东西」。
    """
    m = re.search(r'sqlx::query_as\(\s*"?(SELECT [^"]*?ORDER BY d)"',
                  text, re.S)
    if not m:
        return ""
    lit = m.group(1)
    if "interval '30 days'" not in lit:
        return ""
    return re.sub(r"[ \t]*\\\n[ \t]*", " ", lit)


def h0_static():
    text = strip_line_comments(read_source())
    sql = home_sql(text)
    ok("H0a 首页统计 SQL 确实在册", bool(sql), sql[:80])
    ok("H0b 分桶轴 = 站点日表达式", SITE_DAY in sql, sql[:120])
    ok("H0c 裸 UTC 分桶不得留在 SQL 里",
       re.search(r"SELECT %s AS d" % re.escape(UTC_DAY), sql) is None,
       "命中即「标签站点日 × 分桶 UTC 日」两轴错开")
    m = re.search(r"let today = .*?date_naive\(\)", text, re.S)
    ok("H0d 标注轴仍是站点日（两轴同源）",
       bool(m) and "hours(8)" in (m.group(0) if m else ""),
       "改标注轴而不同改分桶轴＝把缺陷挪个方向")
    return sql


def h1_ruler_sensitivity():
    sql = ("SELECT (t AT TIME ZONE 'UTC')::date AS utc_day, "
           "((t AT TIME ZONE 'UTC') + interval '8 hours')::date AS site_day "
           "FROM (VALUES ('2026-10-21 01:30+08'::timestamptz), "
           "('2026-10-21 07:59+08'::timestamptz), "
           "('2026-10-21 08:01+08'::timestamptz), "
           "('2026-10-21 23:30+08'::timestamptz)) v(t) "
           "ORDER BY 1")
    got = psql_stdin(sql)
    rows = [ln.split("|") for ln in got.split("\n") if ln.strip()]
    ok("H1a 四条时刻都读回来了", len(rows) == 4, got[:120])
    early = [r for r in rows if r[0] != r[1]]
    ok("H1b 两轴可分辨（北京凌晨两格应给出不同日）",
       len(early) == 2,
       "差异行数=%d / %s" % (len(early), got.replace("\n", " ")[:140]))
    d0130 = rows[0][1] if rows else ""
    ok("H1c 站点日轴给的是会员眼中的那一天（01:30 ⇒ 10-21）",
       d0130.endswith("10-21"), d0130)
    ok("H1d UTC 轴在 01:30 那格给出前一天（旧缺陷本体）",
       rows and rows[0][0].endswith("10-20"),
       rows[0][0] if rows else "")


def h2_live(sql):
    if not sql:
        return
    # 一条语句里同时算「首页分组」与「独立公式」——同一快照，不会被正在跑的
    # 别的闸门（它们会真发探针种/下架）在两次往返之间挪走数字。
    # 首页那段 SQL 是 GROUP BY d 的分组结果，取今日量必须 sum(ordinary+official)；
    # 第一版写成 count(*) ⇒ 数的是「今天有几个分组」＝恒 1，红得没道理。
    stmt = (
        "WITH q AS (%s), "
        "a AS (SELECT COALESCE(sum(ordinary + official), 0)::bigint AS n "
        "FROM q WHERE d = %s), "
        "b AS (SELECT count(*)::bigint AS n FROM torrents "
        "WHERE approval_status = 1 "
        "AND created_at >= %s - interval '8 hours'), "
        "c AS (SELECT count(*)::bigint AS n FROM torrents "
        "WHERE approval_status = 1 AND %s = current_date) "
        "SELECT a.n || '|' || b.n || '|' || c.n FROM a, b, c"
        % (sql, SITE_TODAY, SITE_TODAY, UTC_DAY))
    got = psql_stdin(stmt)
    parts = got.split("|")
    ok("H2 「今日」桶 = 站点今日零点以来行数",
       len(parts) == 3 and parts[0] == parts[1], got)
    print("   [H2] 现网读数：首页站点日桶=%s 独立公式=%s 旧轴（UTC 日）=%s"
          % (parts[0] if parts else "?",
             parts[1] if len(parts) > 1 else "?",
             parts[2] if len(parts) > 2 else "?"))


def h3_ledger():
    text = strip_line_comments(read_source())
    ok("H3 首页文件不再出现任何裸 `::date` 分桶",
       "created_at::date" not in text, "残留即两轴又分开")


def selftest():
    """负控：把分桶表达式翻回裸 `created_at::date`，H0b/H3 必须同时红。

    第一版在这里写的是「源码里有 SITE_DAY 这个整串」——可 home.rs 那行为了
    守 80 列被拆成两行 `\` 续行，拼平才等于 SITE_DAY（H0 用的就是拼平后的
    SQL）。所以前置检查用错了视图，selftest 直接 rc=2 报「源码里没有」，
    而闸门本体是好的：判据的坏，不是站的坏。改成按**物理行对**翻转。
    """
    raw = io.open(HOME_RS, encoding="utf-8", newline="").read()
    nl = "\r\n" if "\r\n" in raw[:2000] else "\n"
    lines = raw.split(nl)
    pair = [(i, ln) for i, ln in enumerate(lines)
            if ln.strip() == '"SELECT ((created_at AT TIME ZONE \'UTC\') \\']
    nxt = pair[0][0] + 1 if len(pair) == 1 else 0
    if len(pair) != 1 or "+ interval '8 hours')::date AS d" not in lines[nxt]:
        print("SELFTEST 前置失败：找不到那两行续行的分桶表达式（本闸门测的就是它）")
        return 2
    i = pair[0][0]
    flipped = list(lines)
    flipped[i:i + 2] = ['        "SELECT created_at::date AS d, \\']
    io.open(HOME_RS, "w", encoding="utf-8", newline="").write(nl.join(flipped))
    try:
        p = subprocess.run([sys.executable, os.path.abspath(__file__)],
                           capture_output=True, encoding="utf-8",
                           errors="replace")
        out = p.stdout or ""
        reds = [ln for ln in out.splitlines() if "[FAIL]" in ln]
        print("SELFTEST rc=%d 红条=%d" % (p.returncode, len(reds)))
        for ln in reds:
            print("   ", ln.strip()[:120])
        need = ("H0b" in " ".join(reds)) and ("H3" in " ".join(reds))
        if p.returncode == 0 or not need:
            print("SELFTEST FAILED：翻回旧形状应让 H0b 与 H3 同时红")
            return 1
        return 0
    finally:
        io.open(HOME_RS, "w", encoding="utf-8", newline="").write(raw)
        same = io.open(HOME_RS, encoding="utf-8",
                       newline="").read() == raw
        print("SELFTEST 还原字节一致=%s" % same)


def main():
    if "--selftest" in sys.argv:
        sys.exit(selftest())
    live = "--live" in sys.argv
    sql = h0_static()
    h1_ruler_sensitivity()
    if live:
        h2_live(sql)
    h3_ledger()
    summary()


if __name__ == "__main__":
    main()
