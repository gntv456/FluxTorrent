# -*- coding: utf-8 -*-
"""闸门 DB：站点日界（北京自然日）——两种「+8 小时」错法不许回来。

批一百五十二（P-338）实测的缺陷本体：站内「今日/本周」有三种互相矛盾的窗口，
其中两种算错，错法都是「naive 值拿去和 timestamptz 列比」被会话时区回弹：

  错 A  date_trunc('day', now() AT TIME ZONE 'UTC' + interval '8 hours')
        ⇒ naive「北京零点」按会话时区（本库 UTC）读成 UTC 零点，窗口实际起点
          = 北京 08:00。后果：每天 00:00-08:00 的做种不计入「今日」，
          worker 的满 N 小时发券判定与面板「今日做种小时」同源失真（少算）。
  错 B  date_trunc('week', now() AT TIME ZONE 'UTC') + interval '8 hours'
        ⇒ 先按 UTC 截周再加 8 小时 ⇒ 窗口起点落在上一周。
          后果：周榜/周成就/周领奖资格把上一周的成绩算进本周（多算）。
  对    (date_trunc('day'|'week', now() AT TIME ZONE 'Asia/Shanghai')
          AT TIME ZONE 'Asia/Shanghai')
        ⇒ 先转北京墙上时间截断、再转回 timestamptz（仓里 overview.rs 已是这形状）。

判据不依赖跑判据时的时钟：窗口表达式喂固定时刻（北京 2026-10-19 03:10），
行集是两条写死的流水时间戳 —— 纯函数 A/B，任何时刻重跑结论一样。真库一层由
DB3~DB6 在现场 Postgres 上求值（不是本地模拟）。

批二百三十（P-472）补两条本闸门的盲区：
  DB12 refundable_settle 的「一天一结」幂等窗口写的是 `window_start > current_date`
       （UTC 日界），而作业 cadence=1h ⇒ 北京 07:00 与 08:00 两班在同一站点日，
       第二班看不见第一班那条退款 ⇒ 同一站点日退两次（下载量多退 1/14）。
  DB13 四条错法都带 date_trunc/::date，看不见直接拿裸 current_date 比较的那一族
       ——按台账钉「恰好这几个口」，多一个＝新引入 UTC 日界，少一个＝修掉却没销账。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_db_day_window.py
侧进程：FLUX_API_BASE=http://127.0.0.1:8099/api/v1 python -u \\
        scripts/pt_audit_db_day_window.py
"""
import io
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# Rust 字符串里的行连接符 `\` 会夹在 SQL 片段中间 ⇒ 分隔位一律 [\s\\]*
SEP = r"[\s\\]*"
TOK = r"date_trunc\('(day|week|month)',%snow\(\)" % SEP
WRONG_A = re.compile(TOK + r"\s*AT TIME ZONE 'UTC'%s\+%sinterval%s'8 hours'"
                     % (SEP, SEP, SEP))
WRONG_B = re.compile(TOK + r"\s*AT TIME ZONE 'UTC'\)%s\+%sinterval%s'8 hours'"
                     % (SEP, SEP, SEP))
# 错C：连 +8 都没写，`now()` 后面直接收括号 ⇒ 拿 UTC 日历界去比 timestamptz。
# 月界这档本批实测在个人页「本月做种收益（本月）」上露出（P-339）。
WRONG_C = re.compile(r"date_trunc\('(day|week|month)',%snow\(\)\)" % SEP)
RIGHT = re.compile(TOK + SEP + r"AT TIME ZONE 'Asia/Shanghai'\)" + SEP
                   + r"AT TIME ZONE 'Asia/Shanghai'")

# 错D：把 timestamptz 截成 date 再去比「now() ± interval」——
# ::date 按**会话时区**（本库 UTC）取日界，差 8 小时：
# 00:00-08:00（北京时间）跑出来的「昨日」其实是前天的后半段 + 昨天的一半。
# 形状：`created_at::date = (now() - interval '1 day')::date`。
WRONG_D = re.compile(r"::date\s*=\s*\(\s*now\(\)\s*[-+]\s*interval")
# 待裁台账（不是本批的活）：这些口把日/周界写进了**持久化的 date 键**，
# 换窗口等于给存量数据换身份证 ⇒ 要站方拍板 + 迁移，本批只登记不动。
PENDING = ("apps/api/src/invite_http.rs", "apps/api/src/economy_http/",
           "apps/worker/src/bank_jobs")


def strip_comments(text):
    """剥 // 行注释与 /* */ 块注释：本批注释里写着旧形状，不剥就造假红
    （记忆卡：静态 grep 型判据必须先剥注释再扫）。"""
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    out = []
    for line in text.split("\n"):
        if "http://" in line or "https://" in line:
            out.append(line)
            continue
        cut = re.search(r"(?<!:)//", line)
        out.append(line[:cut.start()] if cut else line)
    return "\n".join(out)


def sources():
    for base in ("apps/api/src", "apps/worker/src"):
        for dp, _dn, fn in os.walk(os.path.join(ROOTDIR, base)):
            for f in fn:
                if f.endswith(".rs"):
                    p = os.path.join(dp, f)
                    yield (os.path.relpath(p, ROOTDIR).replace("\\", "/"),
                           strip_comments(io.open(p, encoding="utf-8",
                                                  errors="replace").read()))


hits, right_sites = [], []
for rel, txt in sources():
    for tag, pat in (("A", WRONG_A), ("B", WRONG_B), ("C", WRONG_C),
                   ("D", WRONG_D)):
        for mm in pat.finditer(txt):
            hits.append((tag, rel, txt[:mm.start()].count("\n") + 1))
    for mm in RIGHT.finditer(txt):
        right_sites.append((rel, txt[:mm.start()].count("\n") + 1))

real = [h for h in hits if not any(h[1].startswith(k) for k in PENDING)]
for tag, rel, ln in hits:
    print("   错%s  %-46s:%-4d（待裁台账内，见文件头）" % (tag, rel, ln))
ok("DB1 三种日历界错法（+8 回弹两种 / 裸 UTC 一种）在全仓归零"
   "（命中 %d，台账 %d，真违例 %d）"
   % (len(hits), len(hits) - len(real), len(real)), not real,
   "、".join("%s:%s" % (h[1], h[2]) for h in real[:5]))
# 按「文件」而不是「处」钉：少一个口就点名是哪个口
EXPECT = ("apps/api/src/games_http/linkage.rs",
          "apps/api/src/games_http/arcade_meta.rs",
          "apps/api/src/games_http/arcade_claim.rs",
          "apps/api/src/games_http/fishing_extra.rs",
          "apps/api/src/games_http/arcade_leaderboard.rs",
          "apps/api/src/games_http/arcade_board.rs",
          "apps/api/src/games_http/pet_log.rs",
          "apps/api/src/repo/community/forum_side.rs",
          "apps/api/src/auth_http/profile.rs",
          "apps/worker/src/jobs/game_coupons.rs",
          "apps/worker/src/jobs/settle.rs",
          "apps/api/src/admin_http/overview.rs",
          "apps/api/src/auth_http/me_settings.rs")
have = set(r for r, _l in right_sites)
miss = [f for f in EXPECT if f not in have]
ok("DB2 十三处日/周/月窗口都在「两次 AT TIME ZONE」形状上（实测 %d 处、缺 %d 个口）"
   % (len(right_sites), len(miss)), not miss,
   "缺=%s｜有=%s" % ("、".join(miss), "、".join(sorted(have))))

# 正控：判据自己得能被这两种错法打红（内存里造假源码，不碰产品文件）
PC_A = ("  AND l.created_at >= date_trunc('day', now() AT TIME ZONE 'UTC' "
        "+ interval '8 hours')")
PC_B = ("  AND l.created_at >= date_trunc('week', now() AT TIME ZONE 'UTC') "
        "+ interval '8 hours'")
PC_C = "  AND t.created_at >= date_trunc('month', now())"
PC_D = ("  WHERE decided_by = 0 AND created_at::date = "
        "(now() - interval '1 day')::date")
pc_hit = (["A"] if WRONG_A.search(PC_A) else []) \
    + (["B"] if WRONG_B.search(PC_B) else []) \
    + (["C"] if WRONG_C.search(PC_C) else []) \
    + (["D"] if WRONG_D.search(PC_D) else [])
ok("DB2a 正控：四种错法各造一条假源码，判据必须都抓到（抓到 %s）" % pc_hit,
   pc_hit == ["A", "B", "C", "D"], str(pc_hit))
ok("DB2b 正控：三条假源码里都不许误报「正确形状」",
   not RIGHT.search(PC_A) and not RIGHT.search(PC_B)
   and not RIGHT.search(PC_C))


def psql(q):
    # ON_ERROR_STOP=1：psql 默认报错也回 rc=0（本轮就被「空读数」骗过一次），
    # 加上它才能让 SQL 语法错误变成响亮的失败而不是安静的空值
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("   psql 失败：%s" % (r.stderr or "").strip()[:170])
    return (r.stdout or "").strip()


FIX = "TIMESTAMP WITH TIME ZONE '2026-10-19 03:10:00+08'"
D_A = "(date_trunc('day', %s AT TIME ZONE 'UTC' + interval '8 hours')" \
      "::timestamptz)" % FIX
W_B = "(date_trunc('week', %s AT TIME ZONE 'UTC') + interval '8 hours')" \
      "::timestamptz" % FIX
D_OK = "(date_trunc('day', %s AT TIME ZONE 'Asia/Shanghai') " \
       "AT TIME ZONE 'Asia/Shanghai')" % FIX
W_OK = "(date_trunc('week', %s AT TIME ZONE 'Asia/Shanghai') " \
       "AT TIME ZONE 'Asia/Shanghai')" % FIX

# 四个窗口起点（HH24；用 HH12 会把 00:00 印成 1200——本轮踩过）
b = psql("SELECT to_char(%s,'YYYY-MM-DD HH24:MI'), to_char(%s,"
         "'YYYY-MM-DD HH24:MI'), to_char(%s,'YYYY-MM-DD HH24:MI'), "
         "to_char(%s,'YYYY-MM-DD HH24:MI')" % (D_A, D_OK, W_B, W_OK))
v = b.split("|")
print("   北京 10-19 03:10 的四个窗口起点（UTC 显示）："
      "错A=%s 对(日)=%s 错B=%s 对(周)=%s" % tuple(v + ["?"] * 4)[:4])
ok("DB3 错 A 的日界 = 10-19 00:00Z（北京 08:00），正确日界 = 10-18 16:00Z"
   "（北京 00:00）：恒差 8 小时",
   len(v) == 4 and v[0] == "2026-10-19 00:00" and v[1] == "2026-10-18 16:00", b)
ok("DB4 错 B 的周界 = 10-12 08:00Z（上一周），正确周界 = 10-18 16:00Z"
   "（北京本周一 00:00）",
   len(v) == 4 and v[2] == "2026-10-12 08:00" and v[3] == "2026-10-18 16:00", b)

# 行集 A/B：两条写死的流水时间戳，看四个窗口各数到几条
#   P1 = 北京 10-19 01:30（今日凌晨）  P2 = 北京 10-15 10:00（上周内）
rows = psql("""WITH t(ts) AS (VALUES
    (TIMESTAMP WITH TIME ZONE '2026-10-19 01:30:00+08'),
    (TIMESTAMP WITH TIME ZONE '2026-10-15 10:00:00+08')
), w AS (SELECT %s d_a, %s d_ok, %s w_b, %s w_ok)
SELECT 'day_ok', count(*) FILTER (WHERE ts >= (SELECT d_ok FROM w)
         AND ts < (SELECT d_ok FROM w) + interval '24 hours') FROM t
UNION ALL SELECT 'day_A', count(*) FILTER (WHERE ts >= (SELECT d_a FROM w)
         AND ts < (SELECT d_a FROM w) + interval '24 hours') FROM t
UNION ALL SELECT 'wk_ok', count(*) FILTER (WHERE ts >= (SELECT w_ok FROM w))
FROM t
UNION ALL SELECT 'wk_B', count(*) FILTER (WHERE ts >= (SELECT w_b FROM w))
FROM t""" % (D_A, D_OK, W_B, W_OK))
m = dict(l.split("|") for l in rows.split("\n") if "|" in l)
print("   两条探针流水在四个窗口下的计数：日_对=%s 日_错A=%s 周_对=%s 周_错B=%s"
      % (m.get("day_ok"), m.get("day_A"), m.get("wk_ok"), m.get("wk_B")))
ok("DB5 凌晨那条流水被错 A 的「今日」漏掉（对=%s vs 错A=%s，应 1 vs 0）"
   "⇒ 00:00-08:00 的做种不计入今日、worker 不发券"
   % (m.get("day_ok"), m.get("day_A")),
   m.get("day_ok") == "1" and m.get("day_A") == "0", rows.replace("\n", " "))
ok("DB6 上周那条流水被错 B 的「本周」收进来（对=%s vs 错B=%s，应 1 vs 2）"
   "⇒ 周榜/周成就/周领奖把上一周成绩算进本周（多算）"
   % (m.get("wk_ok"), m.get("wk_B")),
   m.get("wk_ok") == "1" and m.get("wk_B") == "2", rows.replace("\n", " "))

# ---------- DB10：月界同族（P-339，个人页「本月做种收益（本月）」） ----------
FIX2 = "TIMESTAMP WITH TIME ZONE '2026-11-01 03:10:00+08'"
M_UTC = "date_trunc('month', %s AT TIME ZONE 'UTC')" % FIX2
M_OK = "(date_trunc('month', %s AT TIME ZONE 'Asia/Shanghai') " \
       "AT TIME ZONE 'Asia/Shanghai')" % FIX2
mon = psql("""WITH t(ts) AS (VALUES
    (TIMESTAMP WITH TIME ZONE '2026-11-01 01:30:00+08'),
    (TIMESTAMP WITH TIME ZONE '2026-10-31 23:30:00+08')
)
SELECT '月_对', count(*) FILTER (WHERE ts >= %s) FROM t
UNION ALL SELECT '月_裸UTC', count(*) FILTER (WHERE ts >= (%s)::timestamptz)
FROM t""" % (M_OK, M_UTC)).replace("\n", " ")
mm = dict(re.findall(r"(月_[^|]+)\|(\d+)", mon))
print("   固定时刻 2026-11-01 03:10+08 的月界 A/B：%s" % mon)
ok("DB10 月界同族（P-339 个人页「本月做种收益（本月）」）：北京 11-01 01:30 "
   "与 10-31 23:30 两条流水，北京月界只数本月那条（对=%s），裸 UTC 月界把上月"
   "的也算进「本月」（裸=%s，应 1 vs 2）⇒ 每月 1 号 00:00-08:00 该磁贴显示的是"
   "上一整个月的收益，本月的第一笔要等 08:00 才计入"
   % (mm.get("月_对"), mm.get("月_裸UTC")),
   mm.get("月_对") == "1" and mm.get("月_裸UTC") == "2", mon)

# ---------- 活体层：新码才判，旧镜像回 WARN（豁免不是通过） ----------
tok = login()
KW40 = "甲" * 40
pr = call("POST", "/wishlist", {"keyword": KW40}, token=tok)
NEWCODE = pr[0] == 200
if NEWCODE:
    call("DELETE", "/wishlist/%s" % ((pr[1] or {}).get("data") or {}).get("id"),
         None, token=tok)
print("   本次跑的是%s二进制（心愿单 40 汉字探针 http=%s）"
      % ("**新批**" if NEWCODE else "现网**旧**", pr[0]))

st, body = call("GET", "/games/arcade/leaderboard", token=tok)
api_week = ((body or {}).get("data") or {}).get("week")
sql_week = psql("SELECT to_char(date_trunc('week', "
                "now() AT TIME ZONE 'Asia/Shanghai'), 'IYYY\"W\"IW')"
                ).replace("\n", "")
print("   /games/arcade/leaderboard http=%s week=%s；DB 北京周=%s"
      % (st, api_week, sql_week))
if not NEWCODE:
    print("   WARN DB7 豁免：旧镜像报的是上一周（API=%s / DB=%s）"
          % (api_week, sql_week))
elif api_week is None:
    ok("DB7 周榜接口没给读数（模块关了还是改了形状？）", False, "http=%s" % st)
else:
    ok("DB7 周榜自述的周编号 = 北京自然周编号（API=%s / DB=%s）"
       % (api_week, sql_week), str(api_week) == sql_week)

side = call("GET", "/forums/side", token=tok)
api_today = ((((side[1] or {}).get("data") or {}).get("stats"))
             or {}).get("today_topics")
Q = ("SELECT count(*) FROM topics t JOIN forums f ON f.id = t.forum_id "
     "WHERE f.minclassread <= 1 AND t.created_at >= ")
w_ok = psql(Q + "(date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai') "
            "AT TIME ZONE 'Asia/Shanghai')").replace("\n", "")
w_utc = psql(Q + "date_trunc('day', now())").replace("\n", "")
print("   /forums/side today_topics=%s；北京日窗口=%s，UTC 日窗口=%s"
      % (api_today, w_ok, w_utc))
if not NEWCODE:
    print("   WARN DB8 豁免：旧镜像回 UTC 窗口（API=%s / 北京=%s / UTC=%s）"
          % (api_today, w_ok, w_utc))
elif w_ok != w_utc:
    ok("DB8 论坛「今日新帖」按北京日算（API=%s / 北京=%s / UTC=%s）"
       % (api_today, w_ok, w_utc), str(api_today) == str(w_ok))
else:
    print("   WARN DB8 此刻两窗口无法区分，这条不承重（数据依赖，非放过）")

# ---------- DB11：个人页那格「本月」真的走了改过的 SQL（活体接线层） ----------
prof = call("GET", "/users/1", token=tok)
api_month = ((prof[1] or {}).get("data") or {}).get("month_seed_earn")
S = ("SELECT COALESCE(sum(amount),0)::bigint FROM spark_ledger WHERE "
     "user_id = 1 AND amount > 0 AND kind = 'seeding_reward' "
     "AND created_at >= ")
m_ok = psql(S + "(date_trunc('month', now() AT TIME ZONE 'Asia/Shanghai') "
            "AT TIME ZONE 'Asia/Shanghai')").replace("\n", "")
m_utc = psql(S + "date_trunc('month', now())").replace("\n", "")
print("   /users/1 month_seed_earn=%s；北京月窗口=%s，UTC 月窗口=%s"
      % (api_month, m_ok, m_utc))
if not NEWCODE:
    print("   WARN DB11 豁免：旧镜像走 UTC 月窗口（API=%s / 北京=%s / UTC=%s）"
          % (api_month, m_ok, m_utc))
elif m_ok != m_utc:
    ok("DB11 个人页「本月做种收益」= 北京月窗口读数（API=%s / 北京=%s / UTC=%s）"
       % (api_month, m_ok, m_utc), str(api_month) == str(m_ok))
else:
    print("   WARN DB11 此刻两窗口同数，这条不承重（数据依赖，非放过）")

# ---------- DB9：本批改过的每个口都必须在活体打得通（防「SQL 改坏但判据没覆盖」） ----------
# 本轮真实踩到过：改 arcade_board 的周窗口时把行连接符塞进了 r#"…"# 原始串，
# SQL 里多出字面反斜杠 → /games/arcade-meta 整口 500，而 DB7/DB8 恰好没覆盖它。
TOUCHED = ["/games", "/games/arcade-meta", "/games/arcade/leaderboard",
           "/games/linkage/status", "/games/fishing/rod", "/games/pet/log",
           "/forums/side", "/forums/recent", "/users/1"]
t5xx = []
for path in TOUCHED:
    st9, body9 = call("GET", path, token=tok)
    if st9 >= 500 or st9 <= 0:
        t5xx.append((path, st9, json.dumps(body9, ensure_ascii=False)[:70]))
ok("DB9 改过窗口的 %d 个活口全部打得通（5xx=%d）" % (len(TOUCHED), len(t5xx)), not t5xx,
   "、".join("%s=%s" % (p, s) for p, s, _ in t5xx))
for p, s, b in t5xx:
    print("   [5xx] %s http=%s %s" % (p, s, b))

# ---------- DB12：退款幂等窗口（P-472，refundable_settle「一天一结」） ----------
# 作业 cadence=1h ⇒ 北京 07:00（UTC 10-20 23:00）与 08:00（UTC 10-21 00:00）两班
# 落在同一个站点日里。旧写法 `window_start > current_date` 的日界是 UTC 零点，
# 第二班看不见第一班那条退款流水 ⇒ 同一站点日退两次（下载量多退 1/14＝分享率虚高）。
OLD12 = "clock::date"  # current_date 在本库（会话 UTC）里的展开
NEW12 = ("(date_trunc('day', clock AT TIME ZONE 'Asia/Shanghai') "
         "AT TIME ZONE 'Asia/Shanghai')")


def dedup_pair(clock, prev):
    """在真库求值：第二班能否看见第一班那条负流水（t=能看见=不重复退）。"""
    return psql(("WITH c(clock, prev) AS (VALUES "
                 "(TIMESTAMP WITH TIME ZONE '%s', TIMESTAMP WITH TIME ZONE "
                 "'%s')) SELECT 'old', (prev > %s)::text FROM c "
                 "UNION ALL SELECT 'new', (prev > %s)::text FROM c")
                % (clock, prev, OLD12, NEW12))


r12 = dict(l.split("|") for l in dedup_pair(
    "2026-10-21 00:00+00", "2026-10-20 23:00+00").split("\n") if "|" in l)
print("   跨界那一班：旧闸门看得见=%s 新闸门看得见=%s"
      % (r12.get("old"), r12.get("new")))
ok("DB12 同一站点日跨 UTC 零点那一班：旧窗口看不见上一班退款（会重复退）、"
   "新窗口看得见 ⇒ 一天一结 %s/%s" % (r12.get("old"), r12.get("new")),
   r12.get("old") == "false" and r12.get("new") == "true",
   "old=%s new=%s" % (r12.get("old"), r12.get("new")))
# 正控：不跨界的一班里两窗口必须同真——否则「旧闸门看不见」只是判据恒假
r12b = dict(l.split("|") for l in dedup_pair(
    "2026-10-20 09:00+00", "2026-10-20 02:00+00").split("\n") if "|" in l)
ok("DB12b 正控：同 UTC 日内两窗口都看得见（旧闸门不是恒假）%s/%s"
   % (r12b.get("old"), r12b.get("new")),
   r12b.get("old") == "true" and r12b.get("new") == "true",
   "old=%s new=%s" % (r12b.get("old"), r12b.get("new")))
SETTLE = strip_comments(io.open(os.path.join(ROOTDIR, "apps", "worker",
                                             "src", "jobs", "settle.rs"),
                                encoding="utf-8").read())
ok("DB12c settle.rs 的日界已换成站点日形状（裸 current_date 归零）",
   "current_date" not in SETTLE.lower(),
   "命中=%d" % SETTLE.lower().count("current_date"))

# ---------- DB13：裸 current_date 的比较族（本闸门的盲区，登记不等于放过） ----------
# DB1 的四条错法都带 date_trunc/::date，看不见直接拿 current_date 比的那一族。
# 台账钉「恰好这几个口」：多一个＝新引入 UTC 日界；少一个＝有人修掉了却没更新
# 台账（同批删掉那一行即可）。持久化 date 键那两族改窗口＝给存量数据换身份证，
# 与文件头 PENDING 同口径，等站方拍板。
CMP13 = re.compile(r"(?:[<>]=?|::date\s*=)\s*current_date\b"
                   r"|current_date\b\s*(?:[-+]\s*\d|[-+]\s*interval|<=?|>=?)",
                   re.I)
NAIVE_LEDGER = {
    "apps/api/src/economy_http/checkin.rs": "签到近 30 天列表（展示窗口）",
    "apps/worker/src/bank_jobs/deposit.rs": "利息日界＝持久化 date 键（待裁）",
    "apps/worker/src/bank_jobs/loan.rs": "同上（待裁）",
}
naive = {rel for rel, txt in sources() if CMP13.search(txt)}
extra = sorted(naive - set(NAIVE_LEDGER))
gone = sorted(set(NAIVE_LEDGER) - naive)
ok("DB13 裸 current_date 比较族恰好等于台账 %d 个口（多=%s 少=%s）"
   % (len(NAIVE_LEDGER), extra, gone),
   not extra and not gone,
   "、".join("%s=%s" % (f, NAIVE_LEDGER[f]) for f in sorted(naive)))

# ---------- DB14：后台大盘「今日」那一格（P-473） ----------
# trend() 原来用 generate_series(current_date-13,…) 配 created_at::date ⇒ 格子是
# 「北京 08:00→次日 08:00」，而首页/会员中心的「今日」按北京零点（P-465 同族）：
# 站长在大盘看到的今日格整段漏掉北京 0–8 点，那些行全堆到昨日格。
st14, body14 = call("GET", "/admin/overview", token=tok)
tr14 = ((body14 or {}).get("data") or {}).get("trend") or []
site14 = psql("SELECT count(*) FROM users WHERE created_at >= "
              "date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai') "
              "AT TIME ZONE 'Asia/Shanghai'").strip()
utc14 = psql("SELECT count(*) FROM users WHERE created_at::date = "
             "current_date").strip()
lab14 = psql("SELECT to_char((now() AT TIME ZONE 'Asia/Shanghai')::date, "
             "'MM-DD')").strip()
last14 = tr14[-1] if isinstance(tr14, list) and tr14 else []
print("   后台最后一格：标签=%s 注册数=%s｜站点日读数=%s UTC 日读数=%s（14 格=%d）"
      % (last14[:1], last14[1:2], site14, utc14, len(tr14)))
ok("DB14 后台趋势最后一格 = 北京今日 %s 且注册数 = 站点日读数 %s（API=%s）"
   % (lab14, site14, last14[1:2]),
   st14 == 200 and len(tr14) == 14 and last14[0] == lab14
   and str(last14[1]) == site14,
   "http=%s 格=%s" % (st14, json.dumps(last14, ensure_ascii=False)))
if site14 == utc14:
    print("   WARN DB14a 此刻两轴同数（都=%s），上一条只验标签；分岔窗口内它会变硬判据"
          % site14)
else:
    ok("DB14a 正控：此刻两轴读数分岔（站点日=%s / UTC 日=%s），上一条是硬判据"
       % (site14, utc14), str(last14[1]) != utc14,
       "旧形状会把今日格读成 %s" % utc14)

# ---------- DB15：控制面板「最近 30 天登录趋势」那一格（P-474） ----------
# 旧写法 `created_at::date`（会话 UTC）+ `now() - interval '30 days'`（滚动 720h）
# ⇒ 北京 0–8 点的登录记到「昨天」，今日那格空一截，而下面「30 天内登录 N 天」
# 数的也是这些 UTC 格。root 今天实测：站点日 285 条 / UTC 日 59 条。
uid15 = psql("SELECT id FROM users WHERE username = 'root'").strip()
# 对照必须与被验 SQL 同一谓词：那条趋势语句数的是**全部**登录（含失败），
# 我先加了 AND ok 就差了 1 条 ⇒ 两边都去掉，另把「失败登录该不该进活跃图」
# 单独记成待裁（不是本批改动，旧写法也一直没过滤）。
site15 = psql("SELECT count(*) FROM login_events WHERE user_id = %s "
              "AND created_at >= date_trunc('day', now() AT TIME ZONE "
              "'Asia/Shanghai') AT TIME ZONE 'Asia/Shanghai'" % uid15).strip()
utc15 = psql("SELECT count(*) FROM login_events WHERE user_id = %s "
             "AND created_at::date = current_date" % uid15).strip()
st15, body15 = call("GET", "/me/overview", token=tok)
tr15 = ((body15 or {}).get("data") or {}).get("login_trend_30d") or []
lab15 = psql("SELECT to_char((now() AT TIME ZONE 'Asia/Shanghai')::date, "
             "'YYYY-MM-DD')").strip()
hit15 = [r for r in tr15 if isinstance(r, dict) and r.get("date") == lab15]
cnt15 = str(hit15[0].get("count")) if hit15 else "无今日格"
print("   控制面板今日格：API=%s/%s｜站点日读数=%s UTC 日读数=%s（格数=%d）"
      % (lab15, cnt15, site15, utc15, len(tr15)))
ok("DB15 控制面板趋势今日格 = 站点日读数 %s（API=%s，标签 %s 在册=%s）"
   % (site15, cnt15, lab15, bool(hit15)),
   st15 == 200 and bool(hit15) and cnt15 == site15,
   "http=%s 格=%s" % (st15, json.dumps(hit15, ensure_ascii=False)))
if site15 == utc15:
    print("   WARN DB15a 此刻两轴同数（都=%s），上一条只验标签对齐" % site15)
else:
    ok("DB15a 正控：此刻两轴分岔（站点日=%s / UTC 日=%s），上一条是硬判据"
       % (site15, utc15), cnt15 != utc15, "旧形状今日格读成 %s" % utc15)
MSS15 = strip_comments(io.open(os.path.join(ROOTDIR, "apps", "api", "src",
                                           "auth_http", "me_settings.rs"),
                               encoding="utf-8").read())
# Rust 字符串续行（\ + 换行）会吃掉下一行缩进 ⇒ 静态判据按 DB1/DB2
# 同款 SEP=[\s\\]* 匹配，不做全文拼平（免得误伤别的判据）。
B15 = r"\+ interval '8 hours'\)[\s\\]*::date[\s\\]*AS d"
ok("DB15b 静态：me_settings.rs 的趋势桶带 +8 偏移且窗口是站点日整日",
   re.search(B15, MSS15)
   and "created_at > now() - interval '30 days'" not in MSS15,
   "桶命中=%s 旧窗残留=%s" % (bool(re.search(B15, MSS15)),
                            "created_at > now() - interval '30 days'"
                            in MSS15))

summary()
