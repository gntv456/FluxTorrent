# -*- coding: utf-8 -*-
"""闸门 DQ：抽卡子系统的碎片快照必须自愈，且自愈不许吞历史。

批一百六十八 P-353。会员视角：管理员给我发过碎片，我去兑换一张定向卡。
现网实测：
  gacha_shard_ledger 里有 5 行，gacha_shard_balance 只有 1 行 ——
  user 69 有一笔 `grant delta=100 balance_after=100` 的流水，**却没有余额行**
  （历史残留：发放口今天的代码会先建行，那笔是早期写进去的）。
  后果两层：
    ① 分解/兑换/升级三条口共用 craft.rs 的 shard_move，它用
       `SELECT balance … FOR UPDATE` + fetch_one **假设行存在** ⇒
       RowNotFound 被折成 DomainError::Internal ⇒ **HTTP 500**，
       会员应得的「碎片不足」400 永远看不到；
    ② 只补建行不够——按 `VALUES (user, 0)` 播种会把余额建成 0，
       **静默吞掉流水里的 100 碎片**（真库 A/B：按 0 播种余额=0，
       按 sum(delta) 播种余额=100，且「余额≠流水净额」的人数从 1 变 0）。
  修法：shard_move 与 draw.rs 的入账前建行都按**流水净额**播种
  （快照以流水为准是本仓既有口径：traffic_ledger 的 6h reconcile 同理）。

判据：
  DQ1 两处碎片建行都按 sum(delta) 播种，`VALUES (…, 0)` 形状不得回归；
  DQ2 shard_move 在读余额之前必须先自愈建行（三处消费口共用）；
  DQ3 三条写口各自的守护形状：dismantle 判重 + held<=1、exchange 唯一键 CAS
      （rows_affected=0 → LedgerConflict）、levelup 满级 400；
  DQ4 真库恒等式：balance == sum(delta)（现网 1 人不符 → 上界只降不升，
      该用户下次走任一消费口即自愈）；逐行 balance_after == running sum 必须 0；
  DQ5 正控：事务内造「有流水无余额行」，两种播种形状必须给出 0 与 100 的差；
  DQ6 残料：正控回滚后三张表计数逐位回到原样。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_dq_gacha_shards.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GH = os.path.join(ROOTDIR, "apps", "api", "src", "gacha_http")
CRAFT = os.path.join(GH, "craft.rs")
DRAW = os.path.join(GH, "draw.rs")
STAFF = os.path.join(GH, "staff.rs")
SHARDS = os.path.join(GH, "shards.rs")
IMBALANCE_MAX = 1  # DQ4 现网历史残留 1 人，只降不升


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
        print("   psql 失败：%s" % (r.stderr or "").strip()[:220])
    return (r.stdout or "").strip()


def sval(q):
    return psql(q).split("\n")[0].strip()


def seeds_from_ledger(text):
    """碎片建行是否按流水净额播种（而不是 0）。"""
    for m in re.finditer(r"INSERT INTO gacha_shard_balance[\s\S]{0,260}?"
                         r"ON CONFLICT \(user_id\) DO NOTHING", text):
        if "sum(delta)" in m.group(0):
            return True
    return False


craft, draw, staff = cut(CRAFT), cut(DRAW), cut(STAFF)
shards = cut(SHARDS)

# ---------- DQ1/DQ2 自愈与播种 ----------
ok("DQ1 碎片建行按 sum(delta) 播种（shards=%s）" % seeds_from_ledger(shards),
   seeds_from_ledger(shards),
   "按 0 播种＝把只在流水里的历史碎片静默吞掉（P-353）")
inl = [os.path.basename(f) for f in (CRAFT, DRAW, STAFF)
       if "INSERT INTO gacha_shard_balance" in cut(f)]
ok("DQ1c 建行只有一处实现（内联残留 %d：%s）"
   % (len(inl), "、".join(inl) or "无"), not inl,
   "每处各自长一个建行＝迟早有一处按 0 播种")
def zero_seeded(text):
    """碎片建行是否按 0 播种（无论写成 VALUES 还是 SELECT 形状）。"""
    k = text.find("INSERT INTO gacha_shard_balance")
    if k < 0:
        return False
    win = text[k:k + 240]
    return "SELECT $1, 0" in win or "VALUES ($1, 0" in win


zero_seed = [os.path.basename(f) for f, s in (("shards.rs", shards),
                                             ("craft.rs", craft),
                                             ("draw.rs", draw))
             if zero_seeded(s)]
ok("DQ1b `VALUES ($1, 0)` 旧形状不得回归（残留 %d：%s）"
   % (len(zero_seed), "、".join(zero_seed) or "无"), not zero_seed, "-")
k_lb = shards.find("async fn lock_shard_balance")
lb_body = shards[k_lb:k_lb + 700] if k_lb >= 0 else ""
i_ens = lb_body.find("ensure_shard_row")
i_lock = lb_body.find("FOR UPDATE")
ok("DQ2 lock_shard_balance 体内先自愈再取行锁（craft 调用=%s draw 调用=%s）"
   % ("lock_shard_balance(" in craft, "lock_shard_balance(" in draw),
   i_ens >= 0 and 0 <= i_ens < i_lock
   and "lock_shard_balance(" in craft and "lock_shard_balance(" in draw,
   "没有先自愈＝只在流水里有碎片的会员一进消费口就 500（ensure=%d lock=%d）"
   % (i_ens, i_lock))

# ---------- DQ3 三条写口的守护形状 ----------
guards = {
    "dismantle 判重": "gacha_shard_ledger" in craft and "重复的分解请求" in craft,
    "dismantle 保最后一张": "held <= 1" in craft,
    "exchange 唯一键 CAS": bool(re.search(r"INSERT INTO gacha_exchanges"
                                          r"[\s\S]{0,300}?ON CONFLICT", craft))
    and "rows_affected() == 0" in craft,
    "levelup 满级先拒": "已满级" in craft,
    "发放口幂等可审计": "已发放过" in staff,
}
weak = [k for k, v in guards.items() if not v]
ok("DQ3 五道守护形状齐全（缺失 %d）" % len(weak), not weak,
   "、".join(weak) or "-")

# ---------- DQ4 真库恒等式 ----------
imbal = sval("SELECT count(*) FROM gacha_shard_balance b JOIN (SELECT "
             "user_id, sum(delta) s FROM gacha_shard_ledger GROUP BY user_id"
             ") l USING (user_id) WHERE b.balance <> l.s")
orphan = sval("SELECT count(*) FROM (SELECT DISTINCT user_id FROM "
              "gacha_shard_ledger) l WHERE NOT EXISTS (SELECT 1 FROM "
              "gacha_shard_balance b WHERE b.user_id = l.user_id)")
rowsbad = sval("SELECT count(*) FROM (SELECT id, balance_after - sum(delta) "
               "OVER (PARTITION BY user_id ORDER BY id) AS d FROM "
               "gacha_shard_ledger) x WHERE d <> 0")
ok("DQ4 余额==流水净额 不符 %s 人（上界 %d）；逐行 balance_after 不符 %s 行"
   % (imbal, IMBALANCE_MAX, rowsbad),
   int(imbal or 0) <= IMBALANCE_MAX and rowsbad == "0",
   "逐行不符＝某次动账没同事务更新快照（快照与流水分家）")
print("   [台账] 只在流水里、还没有余额行的用户：%s 人（P-353 的运行期自愈"
      "会在其下一次走消费口时按净额补建；要立刻补齐需一次带 sum(delta) 的"
      "启动修数）" % orphan)

# ---------- DQ5 正控：两种播种形状必须给出差值 ----------
snap = {t: sval("SELECT count(*) FROM %s" % t) for t in
        ("gacha_shard_ledger", "gacha_shard_balance", "gacha_user_cards")}
AB = r"""
SET lock_timeout = '5s';
BEGIN;
CREATE TEMP TABLE ab AS SELECT l.user_id AS uid, sum(l.delta) AS net
  FROM gacha_shard_ledger l JOIN gacha_user_cards c ON c.user_id = l.user_id
  WHERE NOT EXISTS (SELECT 1 FROM gacha_shard_balance b
                    WHERE b.user_id = l.user_id)
  GROUP BY l.user_id LIMIT 1;
SELECT 'net=' || coalesce(max(net), (-1))::text FROM ab;
INSERT INTO gacha_shard_balance (user_id, balance)
SELECT uid, 0 FROM ab ON CONFLICT (user_id) DO NOTHING;
SELECT 'zero_seed=' || coalesce((SELECT balance FROM gacha_shard_balance b
  JOIN ab ON b.user_id = ab.uid), -1)::text;
ROLLBACK;
BEGIN;
CREATE TEMP TABLE ab AS SELECT l.user_id AS uid, sum(l.delta) AS net
  FROM gacha_shard_ledger l JOIN gacha_user_cards c ON c.user_id = l.user_id
  WHERE NOT EXISTS (SELECT 1 FROM gacha_shard_balance b
                    WHERE b.user_id = l.user_id)
  GROUP BY l.user_id LIMIT 1;
INSERT INTO gacha_shard_balance (user_id, balance)
SELECT uid, coalesce((SELECT sum(delta) FROM gacha_shard_ledger l2
                      WHERE l2.user_id = ab.uid), 0)
  FROM ab ON CONFLICT (user_id) DO NOTHING;
SELECT 'ledger_seed=' || coalesce((SELECT balance FROM gacha_shard_balance b
  JOIN ab ON b.user_id = ab.uid), -1)::text;
ROLLBACK;
"""
rd5 = {}
for ln in psql(AB).split("\n"):
    ln = ln.strip()
    if "=" in ln:
        k, v = ln.split("=", 1)
        rd5[k.strip()] = v.strip()
net = rd5.get("net", "-1")
ok("DQ5 正控：同一取样下按 0 播种=%s、按流水播种=%s（净额 %s 时两者必须不同）"
   % (rd5.get("zero_seed"), rd5.get("ledger_seed"), net),
   net not in ("-1", "") and rd5.get("ledger_seed") == net
   and rd5.get("zero_seed") == "0",
   "取样为空＝现网已无「只在流水里」的用户（判据该改成正例），或 psql 报错")

# ---------- DQ6 残料 ----------
after = {t: sval("SELECT count(*) FROM %s" % t) for t in
         ("gacha_shard_ledger", "gacha_shard_balance", "gacha_user_cards")}
ok("DQ6 正控回滚后三表计数逐位回到原样 %s→%s" % (snap, after),
   snap == after, "回滚没吃干净＝闸门在往现网写数据")

# ---------- DQ7 客户端幂等键必须按用户划域（P-382） ----------
# 现网 gacha_shard_ledger.idempotency_key 上有**全局唯一索引**，而键是客户端
# 自填的：键里不带 uid ⇒ 别人用过同一个字符串，我的请求就被误判「重复提交」。
# 同仓先例是商城（economy_http/shop.rs:153 注释原话「全站可碰撞的键，不同请求
# 互相误判重放」），修法照它：format!("gacha:dis:{uid}:{key}")。
KEYD = re.compile(r'format!\(\s*"gacha:(dis|exc|lv):\{\}:\{\}",\s*uid,')
got = KEYD.findall(craft)
ok("DQ7a dis/exc/lv 三把键都按 uid 划域（命中=%d/3）" % len(set(got)),
   len(set(got)) == 3 and len(got) == 3, "命中=%s" % got)
ok("DQ7b 承重自证：旧的两段无主键必须判红",
   not KEYD.search('format!("gacha:dis:{}", body.idempotency_key.trim())'),
   "旧形状应判红")
ok("DQ7c 键前缀仍与商城/银行同族可辨（gacha: 开头，后台按前缀对账数得到）",
   "gacha:dis:" in craft and "gacha:exc:" in craft and "gacha:lv:" in craft)

summary()
