# -*- coding: utf-8 -*-
"""闸门 MK：成就墙进度尺（GET /me/achievements）与达成口径同源（批二百四十一 P-507）。

缺陷本体（现网实测，root uid=1，`achievement_defs` 30 行）：
  旧 handler 只 SELECT `ua.metric_value`，而该列**只在达标那一刻**由 worker
  `achievement_grant` / 票根册 `sync_stubs` 落库 ⇒ 未达成的 25 行全是 NULL。
  界面按 `metric_value / threshold` 画进度、按 `0 < v < threshold` 判「进行中」，
  于是成就墙「进行中」恒 0、25 张进度条全空、`doing` 态在界面上不可达——
  会员看不出自己离下一档还差多少（而 worker 每小时都在算这些数）。

修法：读侧对未达成行现算实时值（五个 PT 指标与 worker 逐字同谓词、十个街机
指标与票根册同 ref_type 口径、stub_count 按已得票根数）。

判据分层：
  MK1–MK3 静态：CASE 必须覆盖 `achievement_defs` 里出现过的**每一个** metric
      （将来加新指标忘了加分支 ⇒ 又回到 NULL）；界面 familyLabel 的键必须是
      DB 里存在的族（旧 `seed:` 是死键 ⇒ 「保种」标签永不命中）；读侧五个 PT
      谓词与 worker 逐字相等（一侧改了没同步另一侧即红）。
  MK4–MK9 活体：行数等于定义数；未达成行不得再有 NULL 进度；已达成的快照值
      不得低于门槛；「进行中」可达；未达成行的进度与独立 SQL 反查一致；
      接口 earned 数 == 库里快照数（反证「doing 的进度来自读侧现算，而不是
      数据碰巧齐」）。

缺陷本体的红证取自提交前：旧 handler 的 SELECT 只有 `ua.metric_value`
（见 `git show 31e9cd79:apps/api/src/v4_http.rs` 第 63 行），库里 root 只有 5 条
快照行 ⇒ 25 个未达成行全 NULL；修后同一口实测 25 条里 20 条带进度。

本支只读现网（不插夹具、不改数据），唯一副作用是 root 用正确密码登录一次。

跑法：
  PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_mk_achievement_progress.py
  （要换基线：FLUX_API_BASE=http://127.0.0.1:8099/api/v1 前缀同上）
"""
import io
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import call, dig, login, ok, summary  # noqa: E402

READ = "apps/api/src/me_achievements.rs"
WORKER = "apps/worker/src/jobs/achv_dormant.rs"
PAGE = "apps/web/app/(main)/me/achievements/page.tsx"


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def cut(src):
    """剥 // 行注释与 /* */ 块注释（注释里出现的旧形状不算违例）。"""
    s = re.sub(r"/\*.*?\*/", "", src, flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in s.split("\n"))


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-F", "|",
         "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql 失败：%s" % (r.stderr or "").strip()[:200])
    return (r.stdout or "").strip()


def scalar(sql):
    lines = psql(sql).splitlines()
    return lines[0].strip() if lines else ""


def rows_of(sql):
    out = []
    for line in psql(sql).split("\n"):
        if line.strip():
            out.append(line.split("|"))
    return out


def metric_clauses(text):
    """抽 `(SELECT count(*) FROM <表> <别名> WHERE ...) AS <metric>` 的谓词。

    两侧用同一个抽取器 ⇒ 比的是「口径字符串」，不是「有没有提过这张表」。
    """
    out = {}
    for m in re.finditer(
        r"\(\s*SELECT count\(\*\)\s+FROM\s+(\w+)\s+(\w+)\s+WHERE"
        r"(.*?)\)\s*AS\s+(\w+)", text, re.S):
        tbl, _alias, cond, metric = m.groups()
        out[metric] = (tbl, re.sub(r"\s+", " ", cond).strip())
    return out


read_src = cut(rd(READ))
worker_src = cut(rd(WORKER))
page_src = cut(rd(PAGE))

defs_total = int(scalar("SELECT count(*) FROM achievement_defs"))
db_metrics = [r[0] for r in rows_of(
    "SELECT DISTINCT metric FROM achievement_defs ORDER BY metric")]
db_families = [r[0] for r in rows_of(
    "SELECT DISTINCT family FROM achievement_defs ORDER BY family")]

# ── MK1 读侧 CASE 覆盖每一个在册 metric ───────────────────
covered = set(re.findall(r"WHEN\s+'(\w+)'\s+THEN", read_src))
missing = [m for m in db_metrics if m not in covered]
print("   [读数] 定义 %d 行 / 在册 metric %d 个 / CASE 分支 %d 个"
      % (defs_total, len(db_metrics), len(covered)))
ok("MK1 读侧 CASE 覆盖 achievement_defs 的每一个 metric"
   "（漏一个就又回 NULL）", not missing, "未覆盖=%s" % missing)

# ── MK2 界面族标签不许有死键 ──────────────────────────────
label_keys = re.findall(r"^\s+(\w+):\s+t\.family\w+,", page_src, re.M)
dead = [k for k in label_keys if k not in db_families]
unlabeled = [f for f in db_families if f not in label_keys]
print("   [读数] familyLabel 键=%s；DB 族=%s；落进「其他」=%s"
      % (label_keys, db_families, unlabeled))
ok("MK2 界面 familyLabel 的每个键都是 DB 里存在的族"
   "（旧 `seed:` 是死键 ⇒ 标签永不命中）", not dead, "死键=%s" % dead)

# ── MK3 五个 PT 谓词与 worker 逐字同口径 ──────────────────
rw, rr = metric_clauses(worker_src), metric_clauses(read_src)
pt_metrics = ["rescue_count", "upload_count", "post_count", "subtitle_count"]
diverge = [m for m in pt_metrics if rw.get(m) != rr.get(m) or m not in rw]
seeding_pair = all("COALESCE(u.seeding_size, 0)" in t and "AS seeding_bytes"
                   in t for t in (worker_src, read_src))
print("   [读数] worker 抽到 %s；读侧抽到 %s；不一致=%s"
      % (sorted(rw), sorted(rr), diverge))
ok("MK3 读侧五个 PT 指标的 SQL 谓词与 worker achievement_grant 逐字相等",
   not diverge and seeding_pair,
   "不一致=%s；seeding 口径两侧齐=%s" % (diverge, seeding_pair))

# ── MK4–MK8 活体 ─────────────────────────────────────────
tok = login("root", "password123")
st, resp = call("GET", "/me/achievements", token=tok)
rows = dig(resp, "data") or []
ok("MK4 GET /me/achievements 回 200 且行数等于定义数",
   st == 200 and dig(resp, "code") == 0 and len(rows) == defs_total,
   "http=%s code=%s 行数=%s 定义=%s"
   % (st, dig(resp, "code"), len(rows), defs_total))

unearned = [r for r in rows if not r.get("earned")]
null_prog = [r.get("code") for r in unearned
             if r.get("metric_value") is None]
below = [(r.get("code"), r.get("metric_value"), r.get("threshold"))
         for r in rows if r.get("earned")
         and (r.get("metric_value") or 0) < (r.get("threshold") or 0)]
doing = [r for r in unearned
         if 0 < (r.get("metric_value") or 0) < (r.get("threshold") or 0)]
print("   [读数] 已达成 %d / 未达成 %d（其中「进行中」%d、进度 NULL %d）；"
      "已达成但快照低于门槛 %d"
      % (len(rows) - len(unearned), len(unearned), len(doing), len(null_prog),
         len(below)))
ok("MK5 未达成的成就也必须带进度值"
   "（旧读法只 SELECT ua.metric_value ⇒ 未达成全 NULL）", not null_prog,
   "进度为 NULL 的 code=%s" % null_prog[:8])
ok("MK6 已达成的 metric_value 是达成时快照，不得低于门槛",
   not below, "异常=%s" % below[:5])
ok("MK7 「进行中」态可达（存在 0 < 进度 < 门槛 的行）", len(doing) > 0,
   "doing=0 而未达成 %d 行 ⇒ 界面进度区整块空" % len(unearned))

# 独立 SQL 反查（措辞与读侧不同）对质四个可复算的现值
uid = scalar("SELECT id FROM users WHERE username = 'root'")
expect = {
    "post_count": int(scalar(
        "SELECT count(*) FROM posts WHERE user_id = " + uid)),
    "upload_count": int(scalar(
        "SELECT count(*) FROM torrents WHERE owner_id = " + uid
        + " AND approval_status = 1")),
    "seeding_bytes": int(scalar(
        "SELECT COALESCE(seeding_size, 0) FROM users WHERE id = " + uid)),
    "game_plays": int(scalar(
        "SELECT count(*) FROM spark_ledger WHERE user_id = " + uid
        + " AND kind = 'game' AND amount < 0")),
}
code_metric = {}
for line in psql("SELECT code || '#' || metric FROM achievement_defs").split(
        "\n"):
    if "#" in line:
        c, m = line.split("#", 1)
        code_metric[c] = m
# 只对「未达成」行对质现值：已达成行的 metric_value 是达成那一刻的快照
# （root 的 up_50 快照 59、up_1 快照 1、arc_first 快照 15，现值分别是 150/150/79，
#  拿快照去比现值必然「不一致」，那是口径而不是洞 —— MK6 已经管住快照的下界）。
mismatch = []
checked = 0
for r in unearned:
    metric = code_metric.get(r.get("code"))
    if metric not in expect:
        continue
    checked += 1
    if (r.get("metric_value") is None
            or int(r["metric_value"]) != expect[metric]):
        mismatch.append((r.get("code"), metric, r.get("metric_value"),
                         expect[metric]))
share = sum(1 for r in unearned if code_metric.get(r.get("code")) in expect)
print("   [读数] 独立反查对质 %d/%d 行（未达成行里落在这四个指标上的）：%s"
      % (checked, share, expect))
ok("MK8 未达成行的进度值与独立 SQL 反查一致（防「读侧算错也算绿」）",
   checked == share and checked > 0 and not mismatch,
   "对质到 %d 行（候选 %d）；不一致=%s" % (checked, share, mismatch[:5]))

# MK9：反证「MK5 的绿不是数据碰巧齐」。未达成行在 `user_achievements` 里
# 根本没有行（earned 判据就是 ua.user_id IS NOT NULL），所以接口报出的
# 「进行中」值只可能来自读侧现算。这条把两端的行数钉成同一个数：
# 接口 earned 数 == 库里该用户的快照行数；一旦 JOIN 写错（多算/漏算）即红。
snap = int(scalar("SELECT count(*) FROM user_achievements WHERE user_id = "
                  + uid))
earned_api = len(rows) - len(unearned)
print("   [读数] 库里快照行 %d 条 / 接口 earned %d 条 / 接口 doing %d 条；"
      "旧读法（只取 ua.metric_value）在同一 %d 个未达成行上全是 NULL，"
      "接口里 NULL %d 条" % (snap, earned_api, len(doing), len(unearned),
                            len(null_prog)))
ok("MK9 正控：接口 earned 数等于库里快照数 ⇒ doing 的进度只能来自读侧现算"
   "（不是数据碰巧齐）", snap == earned_api and earned_api > 0 and len(doing) > 0,
   "snap=%d earned=%d doing=%d" % (snap, earned_api, len(doing)))

summary()
