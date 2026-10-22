# -*- coding: utf-8 -*-
"""闸门 DG：任务指标的「基线口径」一致性（批一百五十九 P-343/P-344）。

缺陷本体（现网 SQL 实证，事务内造现场后 ROLLBACK）：
  P-343 字幕：结算用 `count(*) FROM subtitles WHERE user_id=$1`（生命周期
        总数）判一个 **非 tier**（增量口径）任务。认领前就已发过 3 条字幕的
        成员领 task 3「字幕翻译志愿行」(reward 1500)，下一个 tick 直接达标
        白拿——其余增量指标都减了认领时基线，只它没有。
  P-344 下载：task_claims 只有 base_uploaded / base_seed_seconds / base_uploads
        三列，`download_delta` 一直拿 users.downloaded 现值直比。站方把总下载
        量配成普通任务＝认领即达标（实测 1 TiB 生命周期、认领后 0 字节：旧
        判定 t / 新判定 f）；tier（等级系列）任务本就该累计，旧=新=t 不许变。
  展示层同罪：/tasks 与 /me/exams 两处把 subtitles 报成生命周期总数，于是
        「面板 3/3 达标、结算判 0/3」→ 任务挂到到期还扣罚金。

判据分五层：静态普查（判定用的左值、认领写口是否给基线、展示是否同窗）＋
真库红证 A/B（带残料断言）＋ 列是否已上线（未上线记 WARN，不算通过）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_dg_task_metric.py
"""
import io
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SETTLE = os.path.join(ROOTDIR, "apps", "worker", "src", "task_jobs",
                      "settle.rs")
CLAIM = os.path.join(ROOTDIR, "apps", "api", "src", "ops_http",
                     "task_overview.rs")
EXAMS = os.path.join(ROOTDIR, "apps", "api", "src", "ops_http", "exams.rs")
RECORDS = os.path.join(ROOTDIR, "apps", "api", "src", "ops_http",
                       "task_records.rs")
TAG = int(time.time()) % 100000


def cut(f):
    """剥掉 // 行注释与 /* */ 块注释再普查（注释里的旧形状不算洞）。"""
    s = io.open(f, encoding="utf-8", errors="replace").read()
    s = re.sub(r"/\*.*?\*/", "", s, flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in s.split("\n"))


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s" % (r.stderr or "").strip()[:200])
    return (r.stdout or "").strip()


txt = cut(SETTLE)

# ---------- DG1 判定左值普查：每个 metric 键都得吃「带基线」的量 ----------
KEYS = re.findall(r"pub\(crate\) (\w+): Option<i64>", txt)
LHS = dict(re.findall(r"if let Some\(v\) = metric\.(\w+) \{[^}]*?"
                      r"met &= (\w+) >= v", txt))
SINCE = {"up_d", "dl_d", "seed_d", "uploads_d", "subtitles_now"}
nol = [k for k in KEYS if k not in LHS]
raw = [k for k, v in LHS.items() if v not in SINCE]
ok("DG1 结算判定全用带基线左值（键 %d 个、认不出 %d、吃现值 %d）"
   % (len(KEYS), len(nol), len(raw)),
   not nol and not raw and len(LHS) == len(KEYS) == 6,
   "认不出 %s ／ 吃现值 %s" % ("、".join(nol) or "-", "、".join(raw) or "-"))

# ---------- DG2 认领写口普查：新增写口不给基线就红（全仓 .rs，别只挑我改过的）
rs_files = []
for root, _dirs, names in os.walk(os.path.join(ROOTDIR, "apps")):
    if "target/" in root.replace("\\", "/"):
        continue
    rs_files += [os.path.join(root, n) for n in names if n.endswith(".rs")]
sites = []
for f in rs_files:
    for b in re.findall(r"INSERT INTO task_claims \((.*?)\)\s*SELECT",
                        cut(f), re.S):
        sites.append((os.path.relpath(f, ROOTDIR).replace("\\", "/"),
                      "base_downloaded" in b))
miss = [p for p, has in sites if not has]
ok("DG2 每处认领写口都写 base_downloaded（写口 %d 处、缺 %d）"
   % (len(sites), len(miss)),
   len(sites) >= 3 and not miss,
   "缺基线：%s ／ 在册：%s" % ("、".join(miss) or "-",
                                "、".join(sorted(set(p for p, _ in sites)))))

# ---------- DG3 展示与判定同窗（否则面板说谎、结算扣罚金）----------
bad = []
for f in (RECORDS, EXAMS):
    s = cut(f)
    if not re.search(r"sub\.created_at >= \s*c\.claimed_at", s):
        bad.append(os.path.basename(f) + ":字幕未按认领时窗")
    if "subtitles sub WHERE sub.user_id = u.id" in s and \
            "AND sub.deleted_at IS NULL)" not in s:
        bad.append(os.path.basename(f) + ":现值未排除已删字幕")
ok("DG3 /tasks 与 /me/exams 的字幕进度与结算同窗（失真 %d）" % len(bad),
   not bad, "、".join(bad) or "-")

# ---------- DG3b 展示侧五源齐全（P-527：考核屏曾缺 downloaded）----------
METRIC_KEYS = {"uploaded", "downloaded", "seed_seconds", "uploads",
               "subtitles"}


def cur_keys(f):
    """取 `"seed_seconds"` 所在那个 current 映射里的指标键。"""
    s = cut(f)
    i = s.find('"seed_seconds"')
    if i < 0:
        return set()
    win = s[max(0, i - 320):i + 320]
    return set(re.findall(r'"([a-z_]+)":', win)) & METRIC_KEYS


ks = {os.path.basename(f): cur_keys(f) for f in (RECORDS, EXAMS)}
gaps = {k: sorted(METRIC_KEYS - v) for k, v in ks.items()
        if v != METRIC_KEYS}
ok("DG3b 我的任务记录与我的考核的 current 都是五源（缺 %d 处）" % len(gaps),
   not gaps,
   "缺键=%s ／ 实测=%s" % (gaps, {k: sorted(v) for k, v in ks.items()}))
ok("DG3c 正控：这一条确实读得到键（两文件都是五键）",
   all(len(v) == 5 for v in ks.values()),
   "实测=%s" % {k: len(v) for k, v in ks.items()})

# ---------- DG4 列是否已上线（未上线＝豁免，不是通过）----------
has_col = psql("SELECT count(*) FROM information_schema.columns WHERE "
               "table_name = 'task_claims' AND column_name = "
               "'base_downloaded'") == "1"
applied = psql("SELECT count(*) FROM _sqlx_migrations WHERE version = 395")
if not has_col:
    print("   [WARN] 0395 尚未上线（_sqlx_migrations 登记 %s 条）⇒ "
          "base_downloaded 列还不存在，下面那段红证用事务内临时列跑" % applied)
ok("DG4 迁移与代码同批（列已在 或 列不在且迁移也没登记）",
   has_col or applied == "0", "列=%s 登记=%s" % (has_col, applied))

# ---------- DG5 真库红证：老尺子白拿、新尺子拦住；tier 口径不变 ----------
SRC = r"""
SET lock_timeout = '5s';
BEGIN;
ALTER TABLE task_claims ADD COLUMN IF NOT EXISTS base_downloaded
    BIGINT NOT NULL DEFAULT 0;
CREATE TEMP TABLE probe AS SELECT 1::bigint AS uid, now() AS claimed;
INSERT INTO tasks (name, metric, starts_at, ends_at, target_class, reward,
                   penalty, claim_limit, tier, kind, period)
SELECT '闸门DG-下载任务-%(tag)s',
       jsonb_build_object('download_delta', 1099511627776),
       now() - interval '1 day', now() + interval '30 day',
       0, 5000, 0, 200, NULL, 'task', 'once';
CREATE TEMP TABLE tref AS
  SELECT id AS tid, tier FROM tasks WHERE name = '闸门DG-下载任务-%(tag)s';
INSERT INTO tref
  SELECT id, tier FROM tasks WHERE tier IS NOT NULL
    AND metric ? 'download_delta' ORDER BY id LIMIT 1;
UPDATE users SET downloaded = 1099511627776
 WHERE id = (SELECT uid FROM probe);
INSERT INTO task_claims (task_id, user_id, status, claimed_at, base_uploaded,
                         base_downloaded, base_seed_seconds, base_uploads)
SELECT x.tid, p.uid, 0, p.claimed, 0, 1099511627776, 0, 0
  FROM probe p, tref x;
SELECT 'DL' || CASE WHEN t.tier IS NULL THEN '-non' ELSE '-tier' END,
       (u.downloaded >= (t.metric->>'download_delta')::bigint)::int,
       (CASE WHEN t.tier IS NULL
             THEN (u.downloaded - c.base_downloaded
                   >= (t.metric->>'download_delta')::bigint)
             ELSE (u.downloaded >= (t.metric->>'download_delta')::bigint)
        END)::int
  FROM task_claims c JOIN probe p ON p.uid = c.user_id
  JOIN users u ON u.id = c.user_id JOIN tasks t ON t.id = c.task_id
  JOIN tref r ON r.tid = t.id
 WHERE c.task_id IN (SELECT tid FROM tref)
 ORDER BY 1;
INSERT INTO subtitles (torrent_id, user_id, title, lang, file_ref, created_at)
SELECT s.torrent_id, p.uid, '闸门DG字幕-%(tag)s-' || g, s.lang,
       s.file_ref || '-dg-%(tag)s-' || g, p.claimed - interval '2 hours'
  FROM subtitles s, probe p, generate_series(1, 3) g;
SELECT 'SUB',
       count(*) FILTER (WHERE s.deleted_at IS NULL)::int,
       (count(*) FILTER (WHERE s.deleted_at IS NULL) >= 3)::int,
       count(*) FILTER (WHERE s.deleted_at IS NULL
                        AND s.created_at >= p.claimed)::int,
       (count(*) FILTER (WHERE s.deleted_at IS NULL
                         AND s.created_at >= p.claimed) >= 3)::int
  FROM subtitles s JOIN probe p ON p.uid = s.user_id;
ROLLBACK;
""" % {"tag": TAG}
rows = {}
for ln in psql(SRC).split("\n"):
    parts = [x for x in ln.split("|") if x.strip() != ""]
    if parts:
        rows[parts[0]] = parts[1:]
have_sub = int(psql("SELECT count(*) FROM subtitles") or 0) > 0
tier_row, non_row = rows.get("DL-tier"), rows.get("DL-non")
sub_row = rows.get("SUB")
ok("DG5a 红证落地：闸门确实取到读数（tier %s／非 tier %s／字幕 %s）"
   % (bool(tier_row), bool(non_row), bool(sub_row)),
   bool(tier_row) and bool(non_row) and (bool(sub_row) or not have_sub),
   "取数失败＝psql 报错或被限流，别把空读数当绿")
if non_row:
    ok("DG5b 非 tier download_delta 的白拿窗口已封：实测旧尺子=%s "
       "新尺子=%s（必须 1→0）" % (non_row[0], non_row[1]),
       non_row == ["1", "0"], "读数不是「白拿→拦住」这对差")
if tier_row:
    ok("DG5c tier（等级系列）仍按累计判定，old=new 且达标 (读数 %s)"
       % "/".join(tier_row), tier_row[0] == tier_row[1] == "1", "-")
if sub_row:
    ok("DG5d 字幕：历史 3 条在旧尺子下恒达标(=1)，新尺子按认领后计数(=0) "
       "实测 %s/%s/%s/%s" % tuple(sub_row),
       sub_row == ["3", "1", "0", "0"], "-")

# ---------- DG6 残料：事务回滚后不许留任何一行 ----------
res = {t: psql("SELECT count(*) FROM %s" % t) for t in
       ("task_claims", "tasks")}
left = psql("SELECT count(*) FROM tasks WHERE name LIKE '闸门DG-%%'")
lsub = psql("SELECT count(*) FROM subtitles WHERE title LIKE "
            "'闸门DG字幕-%s%%'" % TAG)
ok("DG6 残料清零（task/claim 总数 %s/%s，本轮造的任务 %s、字幕 %s）"
   % (res.get("tasks"), res.get("task_claims"), left, lsub),
   left == "0" and lsub == "0", "回滚没吃干净＝闸门在往现网写数据")

# ---------- DG7 台账：现网是否有「非 tier + download_delta」的错配任务----
cnt = psql("SELECT count(*) FROM tasks WHERE tier IS NULL "
           "AND metric ? 'download_delta'")
print("   [台账] 现网 tier 为空却配了 download_delta 的任务：%s 条"
      "（0395 后按增量判定，不再有白拿窗口）" % cnt)
ok("DG7 判定键集合未漂移（TaskMetric 仍 6 键，新增键须同步本闸门）",
   len(KEYS) == 6, "实测 %d：%s" % (len(KEYS), "、".join(KEYS)))

summary()
