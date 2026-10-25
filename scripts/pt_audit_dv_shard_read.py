# -*- coding: utf-8 -*-
"""闸门 DV：碎片读数的尺子必须与播种的尺子同一条（P-361）。

批一百七十三。会员视角：我抽过卡、攒了 100 碎片，某天打开抽卡页——碎片栏写 0。

  P-361（本批已修）：`/gacha/me` 与 `/gacha/pool` 读碎片都写死
  `COALESCE(balance, 0)`，而 `gacha_shard_balance` 那行**只有抽卡路径会建**
  （P-353 的同一条根）。于是「只在 `gacha_shard_ledger` 里有碎片」的会员
  读到的就是 0——消费口（分解/兑换/升级）走 `lock_shard_balance` 会自愈建行，
  但展示不必等到那一刻才说实话。现网实证：user 69 流水净额 100、无快照行，
  旧读式回 0、新读式回 100；root 有快照行，两种读式都是 15（不误伤）。

  修法：读数收进 `gacha_http/shards.rs::shard_balance`（执行器泛型，与
  `farm_egg::farm_unit` 同式），fallback 用 `sum(delta)`——与
  `ensure_shard_row` 的播种口径**同一个表达式**，两条规则不再各长一份。

判据形状：DV1-DV3 钉「读侧不再写死 0」「原始 SQL 只剩共用那一处」
（后台全量求和那种聚合读法不算会员读数，单独放行并写明理由），
DV4 是真库 A/B（旧式 vs 新式 vs 流水净额），DV5 把「有流水却无快照行」
的人数钉成只降不升的账实上界，DV6 残料。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_dv_shard_read.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GACHA = os.path.join(ROOTDIR, "apps", "api", "src", "gacha_http")
SHARDS = os.path.join(GACHA, "shards.rs")


def rd(p):
    return io.open(p, encoding="utf-8", errors="replace").read()


def cut(f):
    s = re.sub(r"/\*.*?\*/", "", rd(f), flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in s.split("\n"))


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("   psql 失败：%s" % (r.stderr or "").strip()[:240])
    return (r.stdout or "").strip()


def sval(q):
    return psql(q).split("\n")[0].strip()


sh = cut(SHARDS)

# ---------- DV1 共用读数 ----------
i_fn = sh.find("pub(super) async fn shard_balance")
ok("DV1 shards.rs 有会员读数共用函数 shard_balance（命中=%s）" % (i_fn >= 0),
   i_fn >= 0,
   "读数没收进一处，就会一处一处地长出 COALESCE(...,0)")
body = sh[i_fn:sh.find("\n}", i_fn) + 1] if i_fn >= 0 else ""
ok("DV1b 快照那一段的 fallback 是流水净额，不是写死 0",
   "sum(delta)" in body
   and not re.search(r"gacha_shard_balance[^)]*\)\s*,\s*0\s*\)", body),
   "fallback 写死 0＝只在流水里有碎片的人被当面清零"
   "（注意别把 sum(delta), 0 的聚合兜底误当成它——那一个是合法的）")
ok("DV1c fallback 与播种用的是同一条尺子（ensure_shard_row 也按流水）",
   "sum(delta)" in sh[sh.find("fn ensure_shard_row"):i_fn],
   "两份尺子迟早会分叉（本仓 P 系列反复出现的形状）")

# ---------- DV2 会员读数不再各写各的 SQL ----------
BAD = []
RAW = []
for fn in sorted(os.listdir(GACHA)):
    if not fn.endswith(".rs") or fn == "shards.rs":
        continue
    t = cut(os.path.join(GACHA, fn))
    if "gacha_shard_balance" not in t:
        continue
    # 逐处登记：谁还在直接读这张表、走的是不是共用函数
    for m in re.finditer(r"gacha_shard_balance", t):
        seg = t[max(0, m.start() - 120):m.start() + 120]
        if "shard_balance(" in seg or "lock_shard_balance" in seg:
            continue
        RAW.append((fn, seg.replace("\n", " ")[:70]))
        if re.search(r"COALESCE\(\s*\(?\s*balance", seg) and "user_id" in seg:
            BAD.append(fn)
ok("DV2 除后台聚合（SUM(balance) 全站求和）外，不再有别处直接 SELECT 会员快照行 %s"
   % (sorted(set(f for f, _ in RAW)) or "[]"),
   not BAD,
   "会员读数写死 0 的地方就是 P-361 的复发点：%s" % sorted(set(BAD)))
print("   [在册] 仍直接触碰 gacha_shard_balance 的位置（放行理由见文件头）：")
for fn, seg in RAW:
    print("      %s: %s" % (fn, seg))

# ---------- DV3 调用点真的接上了 ----------
me = cut(os.path.join(GACHA, "me.rs"))
pool = cut(os.path.join(GACHA, "pool.rs"))
ok("DV3 /gacha/me 走共用读数（shards::shard_balance）",
   "shards::shard_balance(" in me, "没接线的共用函数等于没有")
ok("DV3b /gacha/pool 也走同一处（事务内读也接了）",
   "shards::shard_balance(" in pool, "两个口两份尺子迟早分叉")

# ---------- DV4 真库 A/B ----------
AB = r"""
WITH s AS (SELECT l.user_id, sum(l.delta)::int net FROM gacha_shard_ledger l
  GROUP BY l.user_id)
SELECT 'who=' || s.user_id || ' old=' || COALESCE(b.balance, 0)
  || ' new=' || COALESCE(b.balance, s.net) || ' net=' || s.net
  FROM s LEFT JOIN gacha_shard_balance b USING (user_id)
 WHERE b.user_id IS NULL OR COALESCE(b.balance, 0) IS DISTINCT FROM s.net
 ORDER BY s.user_id LIMIT 3;
"""
rows = [ln for ln in psql(AB).split("\n") if ln.startswith("who=")]
if not rows:
    print("   [说明] 现网已无「读数与流水不一致」的取样（DV4 转为不适用）")
    ok("DV4 现网读数与流水净额无不一致样本（0 条）", True, "")
else:
    badpair = [r for r in rows if "old=" in r and r.split(" new=")[1]
               .split(" net=")[0] != r.split(" net=")[1]]
    oldzero = [r for r in rows if r.split("old=")[1].split(" new=")[0] == "0"]
    ok("DV4 取样 %d 条：旧读式回 0 的有 %d 条，而新读式与流水净额逐条相等"
       % (len(rows), len(oldzero)),
       not badpair,
       "新读式与流水净额不等＝共用尺子自己撒了谎")
    for r in rows[:3]:
        print("      %s" % r)

# ---------- DV5 账实上界（只降不升） ----------
orph = sval("SELECT count(*) FROM (SELECT user_id FROM gacha_shard_ledger "
            "GROUP BY user_id) l WHERE NOT EXISTS (SELECT 1 FROM "
            "gacha_shard_balance b WHERE b.user_id = l.user_id)")
ok("DV5 「有流水却无快照行」的人数 %s ≤ 上界 1（P-353/P-361 的历史遗留）" % orph,
   int(orph or 0) <= 1,
   "新增＝又有一条路径只写流水不建快照（读侧靠 DV1 兜住，写侧要查是谁）")

# ---------- DV6 残料 ----------
res = sval("SELECT count(*) FROM gacha_shard_balance b WHERE b.user_id NOT "
           "IN (SELECT id FROM users)")
ok("DV6 快照表没有指向不存在会员的行（%s 条）" % res, res == "0",
   "有＝闸门或消费口在往库里写孤儿行")

summary()
