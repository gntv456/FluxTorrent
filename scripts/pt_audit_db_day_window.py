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
TOK = r"date_trunc\('(day|week)',%snow\(\)" % SEP
WRONG_A = re.compile(TOK + r"\s*AT TIME ZONE 'UTC'%s\+%sinterval%s'8 hours'"
                     % (SEP, SEP, SEP))
WRONG_B = re.compile(TOK + r"\s*AT TIME ZONE 'UTC'\)%s\+%sinterval%s'8 hours'"
                     % (SEP, SEP, SEP))
RIGHT = re.compile(TOK + SEP + r"AT TIME ZONE 'Asia/Shanghai'\)" + SEP
                   + r"AT TIME ZONE 'Asia/Shanghai'")

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
    for tag, pat in (("A", WRONG_A), ("B", WRONG_B)):
        for mm in pat.finditer(txt):
            hits.append((tag, rel, txt[:mm.start()].count("\n") + 1))
    for mm in RIGHT.finditer(txt):
        right_sites.append((rel, txt[:mm.start()].count("\n") + 1))

real = [h for h in hits if not any(h[1].startswith(k) for k in PENDING)]
for tag, rel, ln in hits:
    print("   错%s  %-46s:%-4d（待裁台账内，见文件头）" % (tag, rel, ln))
ok("DB1 两种「+8 小时」错法在全仓归零（命中 %d，台账 %d，真违例 %d）"
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
          "apps/worker/src/jobs/game_coupons.rs")
have = set(r for r, _l in right_sites)
miss = [f for f in EXPECT if f not in have]
ok("DB2 九处日/周窗口都在「两次 AT TIME ZONE」形状上（实测 %d 处、缺 %d 个口）"
   % (len(right_sites), len(miss)), not miss,
   "缺=%s｜有=%s" % ("、".join(miss), "、".join(sorted(have))))

# 正控：判据自己得能被这两种错法打红（内存里造假源码，不碰产品文件）
PC_A = ("  AND l.created_at >= date_trunc('day', now() AT TIME ZONE 'UTC' "
        "+ interval '8 hours')")
PC_B = ("  AND l.created_at >= date_trunc('week', now() AT TIME ZONE 'UTC') "
        "+ interval '8 hours'")
pc_hit = (["A"] if WRONG_A.search(PC_A) else []) \
    + (["B"] if WRONG_B.search(PC_B) else [])
ok("DB2a 正控：两种错法各造一条假源码，判据必须都抓到（抓到 %s）" % pc_hit,
   pc_hit == ["A", "B"], str(pc_hit))
ok("DB2b 正控：同一份假源码里不许误报「正确形状」",
   not RIGHT.search(PC_A) and not RIGHT.search(PC_B))


def psql(q):
    # ON_ERROR_STOP=1：psql 默认报错也回 rc=0（本轮就被「空读数」骗过一次），
    # 加上它才能让 SQL 语法错误变成响亮的失败而不是安静的空值
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s" % (r.stderr or "").strip()[:170])
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

# ---------- DB9：本批改过的每个口都必须在活体打得通（防「SQL 改坏但判据没覆盖」） ----------
# 本轮真实踩到过：改 arcade_board 的周窗口时把行连接符塞进了 r#"…"# 原始串，
# SQL 里多出字面反斜杠 → /games/arcade-meta 整口 500，而 DB7/DB8 恰好没覆盖它。
TOUCHED = ["/games", "/games/arcade-meta", "/games/arcade/leaderboard",
           "/games/linkage/status", "/games/fishing/rod", "/games/pet/log",
           "/forums/side", "/forums/recent"]
t5xx = []
for path in TOUCHED:
    st9, body9 = call("GET", path, token=tok)
    if st9 >= 500 or st9 <= 0:
        t5xx.append((path, st9, json.dumps(body9, ensure_ascii=False)[:70]))
ok("DB9 改过窗口的 8 个活口全部打得通（5xx=%d）" % len(t5xx), not t5xx,
   "、".join("%s=%s" % (p, s) for p, s, _ in t5xx))
for p, s, b in t5xx:
    print("   [5xx] %s http=%s %s" % (p, s, b))

summary()
