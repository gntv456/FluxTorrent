# -*- coding: utf-8 -*-
"""闸门 DU：种子答谢（/torrents/{id}/thanks）的资金守恒。

批一百七十二。会员视角：种主发了一条好种，我点「+100 魔力答谢」——

  P-360（资金级，本批已修）：旧 `do_thank` 在 `amount > 0` 时**只调用
  `earn_spark(owner, amount)`，全程没有向答谢人扣款**。`torrents::thank` 只落一行
  `thanks`，钱是凭空调出来的；界面 `torrent-social.tsx` 的 +1..+10000 按钮组就是
  一条**活的增发口**（每一对「会员 × 种」白送一笔，两个号互相答谢可以无上限刷）。
  更糟的是幂等键写成 `thank-spark:{uid}:{tid}:{now().timestamp()}`——秒级时间戳让
  每次重试都是新键，重放闸门形同不存在。kind 还借用了 `task_reward`，于是这笔
  凭空魔力在流水与后台「任务/考核奖励」台账里都被记成奖励发放。

  修法：扣款（spend_spark_tx）→ 入账（earn_spark_tx）→ 落 thanks 行**同一事务**，
  幂等键锚在**本次新落的感谢行**（created_at 微秒，DU9）而不是「谁谢了哪种」——
  P-380：清号会 `DELETE FROM thanks`（user_del.rs:118）而账本流水按口径不删，
  账号又是墓碑化不物理删除，键若锚在人-种上，复活后再谢同一种永远撞 Replayed
  （实测 400「该笔答谢已受理」且感谢行也落不下去）。防重放本来就由
  `thanks_pkey (torrent_id, user_id)` 承担：并发双击在插行那一步就被弹走。
  两种结局之间不存在「记了感谢、钱没动」的中间态。种不存在直接 404（旧版
  owner=None 时静默跳过转账、再让 thanks 行去撞外键）。

判据形状：DU1-DU5 钉代码形状（含「时钟进幂等键」这条通病），DU6 是真库守恒账，
DU7 用事务内正控证明 DU6 会红（只插一边＝不成对），DU8 钉界面仍发这个口、
且回包用的是服务端真实数额，DU9 钉幂等键锚在行上（旧的两段键判红）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_du_thanks.py
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
ACT = os.path.join(SRC, "torrent_http", "interact.rs")
SPK = os.path.join(SRC, "torrent_http", "thanks_spark.rs")
DETAIL = os.path.join(SRC, "torrents", "detail.rs")
TSX = os.path.join(ROOTDIR, "apps", "web", "components", "torrent-social.tsx")


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
        print("   psql 失败：%s" % (r.stderr or "").strip()[:240])
    return (r.stdout or "").strip()


def first(q):
    return psql(q).split("\n")[0].strip()


act = cut(ACT)
spk = cut(SPK)
det = cut(DETAIL)
tsx = rd(TSX)
i0 = act.find("async fn do_thank(")
i1 = act.find("struct BookmarkReq")
thanks = act[i0:i1] if 0 <= i0 < i1 else act[i0:]

# ---------- DU1 不再有无扣款的入账 ----------
ok("DU1 答谢口不再调用非事务版 earn_spark(（interact.rs 命中 %d 处）"
   % len(re.findall(r"earn_spark\(", act)),
   not re.search(r"earn_spark\(", act),
   "非事务版入账的返回值是 ()：重放与负数都判不出来，也是这条洞的载体")
ok("DU1b thanks_spark.rs 里扣款与入账都在同一 tx 句柄上",
   "spend_spark_tx(\n            tx," in spk or "spend_spark_tx(\n        tx,"
   in spk or ",\n            tx,\n            giver," in spk,
   "两个函数若各自 begin，就还是两段写")
tp = spk[spk.find("async fn transfer("):]
ok("DU1c 扣款在前、入账在后（顺序反了就是先送钱再发现扣不动）",
   0 <= tp.find("spend_spark_tx") < tp.find("earn_spark_tx"),
   "先 earn 后 spend 时，余额不足的拒单会把已发出去的钱留在场上")

# ---------- DU2 幂等键不带时钟 ----------
ok("DU2 答谢幂等键不再掺秒（本文件内 thank-spark 键不带 timestamp）",
   "thank-spark" in spk
   and not re.search(r"thank-spark[^\n]*timestamp", spk + thanks),
   "秒级时间戳让每次重试都是新键＝重放闸门形同不存在")
ok("DU2b 入账键是扣款键的派生（:to），不是又一把独立随机键",
   '{idem}:to' in spk,
   "独立键意味着扣款与入账可以被分别重放")

# ---------- DU9 幂等键锚在「这一行感谢」上（P-380） ----------
ROW3 = re.compile(r'format!\("thank-spark:\{\}:\{\}:\{\}"')
ok("DU9a transfer 的键是三段占位且第三段用 row_ts",
   bool(ROW3.search(spk)) and "row_ts" in spk,
   "命中=%s row_ts=%s" % (bool(ROW3.search(spk)), "row_ts" in spk))
ok("DU9b thank() 把行 created_at 带回（RETURNING + timestamp_micros）",
   "RETURNING created_at" in det and "timestamp_micros" in det,
   "键的源头必须是行，不是请求时钟")
ok("DU9c do_thank 把行时间戳递进 transfer（不是自己造一个）",
   "torrents::thank(&mut *tx, tid, auth.id).await?" in thanks
   and "row_ts)" in thanks, "调用链断了就等于键又回到人-种")
OLD2 = 'let idem = format!("thank-spark:{}:{}", giver, torrent_id);'
ok("DU9d 承重自证：旧的两段键（人-种）必须判红", not ROW3.search(OLD2),
   "旧键命中=%s" % bool(ROW3.search(OLD2)))

# ---------- DU3 感谢行与转账同事务 ----------
ok("DU3 do_thank 拿到事务、感谢行写 tx、末尾一次 commit",
   ".begin()" in thanks and "torrents::thank(&mut *tx" in thanks
   and "tx.commit()" in thanks,
   "感谢行落在连接池上＝转账失败还能留下「已答谢」的假象")
ok("DU3b thank() 改成执行器泛型（能被喂 tx），不再只吃 PgPool",
   re.search(r"pub async fn thank<'e, E>\(\s*exe: E", det) is not None,
   "签名不泛化就只能两段写")

# ---------- DU4 kind 与 404 ----------
ok("DU4 答谢流水不再记成 task_reward（任务/考核奖励台账被凭空魔力污染）",
   "task_reward" not in thanks and '"forum_tip"' in spk,
   "kind 张冠李戴会让后台按 kind 求和的发放统计说谎")
ok("DU4b ref_type 独立标 torrent_thanks（借 kind 译名不借来源判定）",
   '"torrent_thanks"' in spk,
   "只靠 kind 区分，论坛打赏与种子答谢在流水里就混成一坨")
ok("DU4c 种不存在回 404，不再静默跳过转账",
   "DomainError::NotFound(torrent_id)" in spk,
   "owner=None 时旧代码照样落感谢行却一分钱不转")

# ---------- DU5 真库守恒 ----------
SP_N = ("count(*) FILTER (WHERE idempotency_key NOT LIKE '%s:to')" % "%")
EA_N = ("count(*) FILTER (WHERE idempotency_key LIKE '%s:to')" % "%")
PAIR = ("SELECT %s || '/' || %s || '/' || coalesce(sum(amount), 0) "
        "FROM spark_ledger WHERE idempotency_key LIKE 'thank-spark:%%'"
        % (SP_N, EA_N))
raw = first(PAIR)
parts = raw.split("/") if "/" in raw else ["?", "?", "?"]
sp_n, ea_n, net = parts[0], parts[1], parts[2]
ok("DU5 现网答谢流水成对且净额 0（扣 %s 笔 / 入 %s 笔 / 净额 %s）"
   % (sp_n, ea_n, net),
   sp_n == ea_n and net == "0",
   "不成对或净额非 0＝有人在凭空调钱（旧代码留下的行就是这种形状）")

# ---------- DU6 正控：只插一边必须被 DU5 抓到 ----------
PC = r"""
BEGIN;
INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key)
VALUES (nextval('spark_ledger_id_seq'), 1, -10, 'forum_tip',
        'du-probe-thank-A');
SELECT 'half=' || (SELECT count(*) FILTER (WHERE idempotency_key NOT LIKE
  '%:to') || '/' || count(*) FILTER (WHERE idempotency_key LIKE '%:to')
  FROM spark_ledger WHERE idempotency_key LIKE 'du-probe-thank%');
ROLLBACK;
SELECT 'gone=' || count(*) FROM spark_ledger WHERE idempotency_key
  LIKE 'du-probe-thank%';
"""
pc = {}
for ln in psql(PC).split("\n"):
    ln = ln.strip()
    if "=" in ln:
        k, v = ln.split("=", 1)
        pc[k.strip()] = v.strip()
ok("DU6 正控：只插扣款不插入账时判据数到 1/0（DU5 不是空判据），"
   "回滚后残留 %s 条" % pc.get("gone"),
   pc.get("half") == "1/0" and pc.get("gone") == "0",
   "数不到＝DU5 恒真；残留非 0＝正控自己把库弄脏了")

# ---------- DU7 界面仍然打这个口（这条链是活的） ----------
ok("DU7 界面仍有 +1..+10000 答谢按钮组并打同一口",
   "/thanks" in tsx and "amount" in tsx,
   "界面撤了这个口，本闸门的优先级就该往下排")
ok("DU7b 回包读的是服务端算出的 spark_given（不再回显请求里的数字）",
   "r.spark_given > 0" in tsx,
   "回显请求数字时，界面会说「已答谢 100」而账上分文未动")

# ---------- DU8 残料 ----------
res = first("SELECT count(*) FROM spark_ledger WHERE idempotency_key LIKE "
            "'du-probe%'")
ok("DU8 本闸门无残料（探针流水 %s 条）" % res, res == "0",
   "探针行留在现网会被下一个玩家碰到")

summary()
