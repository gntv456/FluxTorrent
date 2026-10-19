# -*- coding: utf-8 -*-
"""闸门 DI：付费发放类写口的「收钱必须发货」与通知失败必须出声。

批一百六十一 P-345 实测（真库事务内造现场后 ROLLBACK）：
  会员曾持有某限时勋章且**已过期** → 赠送前的「已拥有」判据（只看未过期行）
  回 false ⇒ 允许送；而 user_medals 主键是 (user_id, medal_id)，旧语句
  `ON CONFLICT DO NOTHING` 实写 **0 行** ⇒ spend_spark 已扣全价、税已入站免池、
  接口回 200，对方一件没收到，且没有任何出声。改成
  `DO UPDATE … WHERE expires_at <= now()`（过期件复活、重新起算有效期），
  同一条 A/B 实写 **1 行**；仍为 0 行（并发竞态/勋章被删）时退款并回中文错误。
  现网 dormant：当前无勋章配 duration_days>0，但后台勋章表单
  （apps/web/components/admin-medals-form.tsx:159）一格就能配 ⇒ 一踩就中。

判据分四层：
  DI1 真库红证 A/B（旧 0 行 / 新 1 行）＋ 残料清零；
  DI2 medal_gift.rs 的复活分支、rows_affected 检查、退款幂等键都在（可负控）；
  DI3 「先扣钱后发放」一族写口普查：扣款之后必须有可核对的发放凭据
      （rows_affected / CAS 置位 / 先占位后扣费），出现裸发放就红；
  DI4 通知静默棘轮：`let _ = sqlx::query(` 包住 `INSERT INTO messages` 的
      处数只许降不许升（基线取本批修完后的实测值）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_di_paid_grant.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOTDIR, "apps", "api", "src")
GIFT = os.path.join(SRC, "community_http", "medal_gift.rs")
# DI4 基线：批一百六十一把 medal_gift 的收件通知改成出声后的实测处数。
# 口径＝语句体内出现 INSERT INTO messages 的 `let _ = sqlx::query(`；
# 只许降不许升，升了就说明新写了静默吞失败的通知口（P-323/P-328 同族）。
NOTICE_SWALLOW_BASELINE = 15


def cut(f):
    s = io.open(f, encoding="utf-8", errors="replace").read()
    s = re.sub(r"/\*.*?\*/", "", s, flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in s.split("\n"))


def rs_files():
    out = []
    for root, _d, names in os.walk(SRC):
        if "target/" in root.replace("\\", "/"):
            continue
        out += [os.path.join(root, n) for n in names if n.endswith(".rs")]
    return out


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s" % (r.stderr or "").strip()[:220])
    return (r.stdout or "").strip()


def sval(q):
    return psql(q).split("\n")[0].strip()


# ---------- DI1 真库红证：过期持有下，旧语句 0 行、新语句 1 行 ----------
TAG = int(__import__("time").time()) % 100000
AB = r"""
SET lock_timeout = '5s';
BEGIN;
SELECT 'medal=' || coalesce((SELECT min(id) FROM medals
    WHERE price > 0), 0)::text || '|user=' || (SELECT min(id)
    FROM users WHERE status < 2)::text;
-- 取样：一枚可赠送的勋章 + 一个正常会员，先造「曾持有且已过期」
INSERT INTO user_medals (user_id, medal_id, source, expires_at)
SELECT u.id, m.id, 'gift', now() - interval '1 day'
  FROM (SELECT id FROM medals WHERE price > 0 ORDER BY id LIMIT 1) m,
       (SELECT id FROM users WHERE status < 2 ORDER BY id LIMIT 1) u
ON CONFLICT (user_id, medal_id) DO UPDATE SET source = 'gift',
  expires_at = now() - interval '1 day'
RETURNING 'planted_user=' || user_id || ' medal=' || medal_id;
-- ① 赠送前的「已拥有」判据（只看未过期）——必须回 f 才走得进扣款
SELECT 'owned=' || EXISTS(SELECT 1 FROM user_medals um
  JOIN users u ON u.id = um.user_id
  WHERE u.status < 2 AND (um.expires_at IS NULL OR um.expires_at > now())
    AND um.user_id = (SELECT id FROM users WHERE status < 2 ORDER BY id
                      LIMIT 1)
    AND um.medal_id = (SELECT id FROM medals WHERE price > 0 ORDER BY id
                       LIMIT 1))::text;
-- ② 旧语句（DO NOTHING）实际写入行数
WITH o AS (INSERT INTO user_medals (user_id, medal_id, source, expires_at)
  SELECT u.id, m.id, 'gift', now() + make_interval(days => m.duration_days)
  FROM (SELECT id, duration_days FROM medals WHERE price > 0
        ORDER BY id LIMIT 1) m,
       (SELECT id FROM users WHERE status < 2 ORDER BY id LIMIT 1) u
  ON CONFLICT (user_id, medal_id) DO NOTHING RETURNING 1)
SELECT 'old_rows=' || count(*)::text FROM o;
-- ③ 新语句（过期件复活）实际写入行数
WITH n AS (INSERT INTO user_medals (user_id, medal_id, source, expires_at)
  SELECT u.id, m.id, 'gift', now() + make_interval(days => m.duration_days)
  FROM (SELECT id, duration_days FROM medals WHERE price > 0
        ORDER BY id LIMIT 1) m,
       (SELECT id FROM users WHERE status < 2 ORDER BY id LIMIT 1) u
  ON CONFLICT (user_id, medal_id) DO UPDATE SET source = 'gift',
    expires_at = EXCLUDED.expires_at
  WHERE user_medals.expires_at IS NOT NULL
    AND user_medals.expires_at <= now() RETURNING 1)
SELECT 'new_rows=' || count(*)::text FROM n;
ROLLBACK;
"""
rd = {}
for ln in psql(AB).split("\n"):
    ln = ln.strip()
    if "=" not in ln:
        continue
    key, val = ln.split("=", 1)
    rd[key.strip()] = val.strip()
# psql 把 boolean 渲染成 true/false，与 -tA 的 f/t 两种都要认
if rd.get("owned") in ("true", "false"):
    rd["owned"] = "t" if rd["owned"] == "true" else "f"
rd["planted"] = rd.get("planted_user", "")
ok("DI1a 红证确实取到读数（planted=%s owned=%s old=%s new=%s）"
   % (rd.get("planted", "-").split("|")[0][:14], rd.get("owned"),
      rd.get("old_rows"), rd.get("new_rows")),
   bool(rd.get("planted")) and rd.get("owned") == "f",
   "取数失败＝psql 报错/取样为空；owned 必须为 f（判据放过才会走到扣款）")
ok("DI1b 旧语句在过期持有下写 0 行＝收钱不发货（实测 old=%s，须 0）"
   % rd.get("old_rows"), rd.get("old_rows") == "0", "-")
ok("DI1c 新语句把过期件复活、确实写 1 行（实测 new=%s，须 1）"
   % rd.get("new_rows"), rd.get("new_rows") == "1",
   "0 行＝修完仍然静默不发货；2 行＝取样漂移")
res = sval("SELECT count(*) FROM user_medals WHERE source = 'gift' "
           "AND expires_at < now() - interval '23 hours'")
ok("DI1d 红证残料清零（回滚后不该留下过期 gift 行，实测 %s）" % res,
   res == "0", "-")

# ---------- DI2 medal_gift.rs 的修形承重 ----------
g = cut(GIFT)
need = {
    "过期件复活分支 (DO UPDATE … expires_at <= now())":
        re.search(r"DO UPDATE SET[\s\S]{0,200}expires_at <= now\(\)", g),
    "发放行数检查 rows_affected": "rows_affected()" in g,
    "0 行时退款（medal-gift-refund 幂等键）": "medal-gift-refund:" in g,
    "收件通知失败出声（不再 let _ =）":
        "tracing::warn" in g and "let _ = sqlx::query" not in g,
}
miss = [k for k, v in need.items() if not v]
ok("DI2 赠送口的四道形全在（缺失 %d）" % len(miss), not miss,
   "、".join(miss))
ok("DI2b 旧形状 `ON CONFLICT (user_id, medal_id) DO NOTHING` 不得回归",
   "DO NOTHING" not in g, "回到静默吞冲突＝P-345 复发")

# ---------- DI3 先扣钱后发放的一族：扣款后要有可核对的发放凭据 ----------
PROOF = ("rows_affected()", "SpendOutcome", "applied", "status = 0",
         "RETURNING id")
sites = []
for f in rs_files():
    s = cut(f)
    if "spend_spark(" not in s:
        continue
    after = s[s.index("spend_spark("):]
    grants = re.findall(r"INSERT INTO (user_\w+|\w+_purchases|\w+_items)\b",
                        after)
    if not grants:
        continue
    has_proof = any(p in after for p in PROOF)
    sites.append((os.path.relpath(f, ROOTDIR).replace("\\", "/"),
                  len(grants), has_proof))
naked = [p for p, _n, proof in sites if not proof]
ok("DI3 凡「扣款后写发放行」都有凭据可核对（写口 %d 处、裸发 %d）"
   % (len(sites), len(naked)), bool(sites) and not naked,
   "裸发放：%s" % ("、".join(naked) or "-"))

# ---------- DI4 通知静默棘轮（只降不升）----------
cnt = 0
for f in rs_files():
    s = cut(f)
    for m in re.finditer(r"let _ = sqlx::query\(([\s\S]*?)\.await;", s):
        if "INSERT INTO messages" in m.group(1):
            cnt += 1
ok("DI4 被 `let _ =` 吞掉的站内信写入：%d 处 ≤ 基线 %d（只许降）"
   % (cnt, NOTICE_SWALLOW_BASELINE), cnt <= NOTICE_SWALLOW_BASELINE,
   "新增静默通知口＝收件人投诉时查不到痕迹（P-323/P-328 同族）")
print("   [台账] DI4 现存量 %d 处属既有债，逐只改造需单独一批（改一减一，"
      "基线随之收缩）；本批不代改，避免一次动十几只写口却够不到界面复验"
      % cnt)

summary()
