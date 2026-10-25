# -*- coding: utf-8 -*-
"""闸门 DN：站免池「池账」与「荣誉榜」的包含关系，以及两把尺的现网差值。

批一百六十六（P-351，待裁口径，先钉桩不代改）。会员视角打开 /magic-pool：
同一个屏幕上「进度条」和「捐赠榜」是两个数，代码里还多一个 dashboard 的
「捐赠合计」，三者读三处：

  magic_pool.donated_total   ← 四条写入：自愿捐赠 pool_donate、众包 funding_contribute、
                               勋章赠送税 medal_gift、付费种子抽税 torrents/charge.rs
  pool_donations（荣誉榜）    ← 只前三条写；**付费种子抽税不写**
  dashboard.rs:45             ← SUM(pool_donations) 全量，不带 month

于是两条不变量与一条已知差值：
  ① 硬不变量：任一月份 榜单 ≤ 池账（榜单行必然伴随同事务的池账累加）。
     反过来（榜单 > 池账）说明有第五条写入只记了荣誉榜不记池账 = 账实不符。
     DN1 钉这条，DN4 用正控证明它真的会红。
  ② 单一月键：写这两张表的代码必须用 economy::pool_month（P-340 的教训：
     UTC `%Y-%m` 会把 00:00–08:00 的北京时间记到上个月）。DN2 普查。
  ③ 已知差值（待裁，不在本批改口径）：付费种子抽税只进池账。现网实测
     2026-09 差 250、2026-10 差 2950（全量付费税 2400，余下差额含 P-340
     之前的错桶历史）。两条出路都要站方拍板：
       (a) charge.rs 也写 pool_donations ⇒ 三处同源，但买家「被抽的税」会被
           列进他的捐赠额与荣誉榜（他没打算捐）；
       (b) medal_gift 停写 pool_donations ⇒ 榜单只列自愿捐赠，但 pool_donations
           没有来源列，历史行无法区分，需要一次带 source 的迁移。
     本闸门把差值报成 WARN 读数并钉住「谁改了口径必须同步这里」。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_dn_pool_ledger.py
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
WRITERS = ("magic_pool", "pool_donations")


def rd(p):
    return io.open(p, encoding="utf-8", errors="replace").read()


def cut(f):
    s = re.sub(r"/\*.*?\*/", "", rd(f), flags=re.S)
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
        raise RuntimeError("   psql 失败：%s" % (r.stderr or "").strip()[:220])
    return (r.stdout or "").strip()


# ---------- DN1 硬不变量：任一月 榜单 ≤ 池账 ----------
rows = psql("SELECT m.month || '|' || m.donated_total || '|' || "
            "coalesce(d.s, 0) FROM magic_pool m LEFT JOIN (SELECT month, "
            "sum(amount) s FROM pool_donations GROUP BY month) d "
            "ON d.month = m.month ORDER BY m.month")
months = [x.strip() for x in rows.split("\n") if x.strip()]
over = []
for ln in months:
    mon, pool, board = ln.split("|")
    if int(board) > int(pool):
        over.append("%s 榜单 %s > 池账 %s" % (mon, board, pool))
ok("DN1 各月「荣誉榜合计 ≤ 池账」成立（在册 %d 月，违例 %d）"
   % (len(months), len(over)), bool(months) and not over,
   "、".join(over) or "-")

# ---------- DN2 月键单一尺子 ----------
bad_key = []
for f in rs_files():
    s2 = cut(f)
    writes = re.search(r"INSERT INTO (magic_pool|pool_donations)", s2)
    if not writes:
        continue
    if "pool_month" not in s2:
        bad_key.append(os.path.relpath(f, ROOTDIR).replace("\\", "/"))
ok("DN2 凡是写池账/榜单的文件都用 economy::pool_month 定月键（违例 %d）"
   % len(bad_key), not bad_key,
   "自算月界：%s（P-340 教训：UTC 口径会把 00:00-08:00 记到上个月）"
   % ("、".join(bad_key) or "-"))

# ---------- DN3 三个读数各自的来源钉住 ----------
st = cut(os.path.join(SRC, "economy_http", "pool.rs"))
dash = cut(os.path.join(SRC, "economy_http", "dashboard.rs"))
ok("DN3 /magic-pool 进度读 donated_total、榜单读 pool_donations；看板读"
   "合计（来源混用就红）",
   "donated_total" in st and "FROM pool_donations" in st.replace("\n", " ")
   and "FROM pool_donations" in dash, "-")

# ---------- DN4 正控：只加榜单不动池账，DN1 必须红 ----------
ctl = psql(r"""
SET lock_timeout = '5s';
BEGIN;
CREATE TEMP TABLE ct AS SELECT month, donated_total FROM magic_pool
  ORDER BY month DESC LIMIT 1;
INSERT INTO pool_donations (user_id, amount, month)
SELECT 1, (SELECT donated_total FROM ct) + 1000, month FROM ct;
SELECT 'over=' || count(*) FROM magic_pool m JOIN (SELECT month,
  sum(amount) s FROM pool_donations GROUP BY month) d USING (month)
  WHERE d.s > m.donated_total;
ROLLBACK;
""")
planted = "over=1" in ctl
ok("DN4 正控落地：造一条「榜单>池账」的假账后判据确实数到（读数 %s）"
   % ctl.replace("\n", " ")[:40], planted,
   "数不到＝DN1 是空判据（正控没抓住已知洞）")
after = psql("SELECT count(*) FROM pool_donations")
ok("DN4b 正控残料：回滚后 pool_donations 行数可读数（%s）" % after,
   after.isdigit(), "读数失败＝psql 报错")

# ---------- DN5 待裁台账（报数，不判红）----------
gap = psql("SELECT coalesce(sum(m.donated_total - coalesce(d.s,0)),0) "
           "FROM magic_pool m LEFT JOIN (SELECT month, sum(amount) s "
           "FROM pool_donations GROUP BY month) d ON d.month=m.month")
print("   [待裁 P-351] 池账比榜单多计 %s 魔力——现由「付费种子抽税只进池账」"
      "造成；(a) charge.rs 补写 pool_donations（三处同源，但买家被抽的税会"
      "进他的捐赠额）或 (b) 赠送税停写榜单（需给 pool_donations 加来源列"
      "并迁移历史）。本批不代改，改口径必须同步本闸门与这段说明" % gap)

summary()
