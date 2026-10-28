#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""MZ：余额快照不变量普查（`spark_balance == base_spark + SUM(spark_ledger)`）。

为什么这条必须有（2026-10-28 现网量出来的，不是推断）：
`jobs/seeding.rs:141-149` 每小时把快照重算成
`COALESCE(balance_baseline.base_spark,0) + COALESCE(SUM(spark_ledger.amount),0)`
（排除 `orphan_offset`，且**只覆盖 2h 内有过流水的用户**）。所以「直接写
`users.spark_balance` 而不落流水、也不抬 baseline」的账号，账面是不变量外的数：
下一碰流水就被重算抹平——现网实测 `audit_vip` 钱包 97813、流水净 -2187，
被抹平就是**负余额**，而 `users.spark_balance` 是 `bigint NOT NULL` 且
**没有 CHECK**（`admin_http/clawback.rs:4` 自己写着这个洞）。

这条不变量在本仓是**成文设计**，不是我的发明：
`np_import.rs:180` 「上下量初始值：写 users 快照列 + 同步抬 balance_baseline
（P0-2 同族，0285）」、`bulk_traffic.rs:5` 「发放量必须与流水同写。
users.uploaded 只是 balance_baseline + SUM(traffic_ledger) 的快照」、
`user_ops.rs:163` 同口径。旧账「管理补流量下一次 announce 归零」就是它。

现在库里有 232 个账号违例（净额 +1,301,909），其中 227 个是 status>=2 的
闸门探针/墓碑号，6 个还在册——所以这把尺首要盯的是**制造漂移的那只手**：
闸门脚本里的 `UPDATE users SET spark_balance = …` 若不同文件抬 baseline，
就是在给共享库造脏账（而且它自己下一轮还会被抹平，症状变成「夹具钱没了」）。
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, psql_stdin, summary  # noqa: E402

# 在册（status<2）漂移账号台账：id -> 定性（多一个少一个都红）
LIVE_DRIFT = {
    7483: "fe230714：夹具直接写钱包 200，未落流水未抬 baseline",
    7524: "fe235916：同上",
    7523: "fe135916：同上",
    7482: "fe130714：同上",
    39: "deleted-39-restored：反向（流水 +30 > 钱包 0），重算会把钱包抬上去",
    # 7346 audit_vip 曾在册漂移 100000（本轮 04:49Z 实测 wallet=97813 /
    # 流水净 -2187），5 分钟后再量就与不变量一致了——中间没有任何新流水
    # （最后一行 10-25），是 seeding 那一拍把快照重算成 baseline+SUM，
    # 直接把它打成 -2187。这就是「脏账会被静默抹平」的活体证据，
    # 也是本仓成文的 P0-2 同族（np_import.rs:180、bulk_traffic.rs:5）。
}
# 脚本侧：闸门/e2e 里「写快照却不抬 baseline/不落流水」的处数是历史欠账，
# 用基数棘轮钉住（只降不升），名单每轮打印——新增一只手的瞬间就多一个文件。
MINT_BASELINE_MAX = 26  # 2026-10-28 实测基数：26 个脚本这么干过

SNAP = re.compile(r"UPDATE\s+users\s+SET[^;]*?spark_balance\s*=", re.I)
BASE = re.compile(r"balance_baseline", re.I)
LEDGER_INS = re.compile(r"INSERT\s+INTO\s+spark_ledger", re.I)


def offends(text):
    """「直接写快照，却既不抬 baseline 也不落流水」——这只手就是脏账来源。"""
    return bool(SNAP.search(text)) and not BASE.search(text) \
        and not LEDGER_INS.search(text)


def read_scripts():
    for fn in sorted(os.listdir("scripts")):
        if fn.endswith(".py") and not fn.startswith("pt_audit_mz"):
            yield "scripts/" + fn, open(os.path.join("scripts", fn),
                                        encoding="utf-8",
                                        errors="replace").read()


sql_users = """
WITH cur AS (
  SELECT u.id, u.status, u.spark_balance AS wallet,
         COALESCE(b.base_spark,0) + COALESCE(l.s,0) AS implied
  FROM users u
  LEFT JOIN balance_baseline b ON b.user_id = u.id
  LEFT JOIN (SELECT user_id, sum(amount) s FROM spark_ledger
             WHERE kind <> 'orphan_offset' GROUP BY user_id) l
            ON l.user_id = u.id)
SELECT 'live='||count(*) FILTER (WHERE status < 2
                                 AND wallet <> implied) FROM cur UNION ALL
SELECT 'purged='||count(*) FILTER (WHERE status >= 2
                                   AND wallet <> implied) FROM cur UNION ALL
SELECT 'net='||COALESCE(sum(wallet - implied), 0) FROM cur
  WHERE wallet <> implied UNION ALL
SELECT 'ids='||id FROM cur WHERE status < 2 AND wallet <> implied UNION ALL
SELECT 'users_total='||count(*) FROM cur"""

rows = psql_stdin(sql_users).splitlines()
live = purged = net = total = 0
ids = []
for ln in rows:
    if ln.startswith("live="):
        live = int(ln.split("=")[1])
    elif ln.startswith("purged="):
        purged = int(ln.split("=")[1])
    elif ln.startswith("net="):
        net = int(ln.split("=")[1])
    elif ln.startswith("users_total="):
        total = int(ln.split("=")[1])
    elif ln.startswith("ids="):
        ids.append(int(ln.split("=")[1]))

scripts = list(read_scripts())
mint_offenders = sorted(p for p, t in scripts if offends(t))
mint_all = sum(1 for _, t in scripts if SNAP.search(t))

FAKE_MINT = ('psql("UPDATE users SET spark_balance = 100000 WHERE id = 7")\n')
FAKE_FIXED = FAKE_MINT + ('psql("INSERT INTO balance_baseline (user_id, '
                          'base_spark) VALUES (7, 100000) '
                          'ON CONFLICT (user_id) DO UPDATE SET '
                          'base_spark = EXCLUDED.base_spark")\n')
ok("MZ1 正控：合成件「只写快照」必须被 offends() 判成违例",
   offends(FAKE_MINT), "")
ok("MZ1b 正控（反向）：同件补上 baseline 抬升后 offends() 必须转绿",
   not offends(FAKE_FIXED), "")
ok("MZ2 在册漂移账号 = 台账 %s（新增一个=又有只手直接写了钱包）"
   % sorted(LIVE_DRIFT), set(ids) == set(LIVE_DRIFT),
   "实测 ids=%s live=%d" % (sorted(ids), live))
ok("MZ3 反空转：普查必须覆盖全表（users_total=%d>1000 且脚本侧抓到>0 处写快照）"
   % total, total > 1000 and mint_all > 0,
   "users=%d 写快照的脚本=%d 个" % (total, mint_all))
ok("MZ4 棘轮：写快照不抬 baseline 的脚本数须 <= %d（只降不升）"
   % MINT_BASELINE_MAX, len(mint_offenders) <= MINT_BASELINE_MAX,
   "实测=%d 个=%s" % (len(mint_offenders), mint_offenders))
neg = int(psql_stdin("SELECT count(*) FROM users WHERE spark_balance < 0")
          .splitlines()[0].strip())
ok("MZ5 棘轮：负余额账号数须 <= 39（该列无 CHECK，脏账被抹平就落到这里）",
   neg <= 39, "负余额=%d" % neg)
TRAFFIC_SQL = """
WITH cur AS (
  SELECT u.id, u.status, u.uploaded, u.downloaded,
    COALESCE(b.base_up,0)+COALESCE(t.su,0) AS iu,
    COALESCE(b.base_down,0)+COALESCE(t.sd,0) AS idn
  FROM users u
  LEFT JOIN balance_baseline b ON b.user_id = u.id
  LEFT JOIN (SELECT user_id, sum(delta_up) su, sum(delta_down) sd
             FROM traffic_ledger GROUP BY user_id) t ON t.user_id = u.id)
SELECT 'drift='||count(*) FILTER (WHERE uploaded <> iu
                                  OR downloaded <> idn) FROM cur UNION ALL
SELECT 'total='||count(*) FROM cur UNION ALL
SELECT 'anchor='||(SELECT count(*) FROM traffic_ledger WHERE user_id = 0)"""

tr = {}
for ln in psql_stdin(TRAFFIC_SQL).splitlines():
    if "=" in ln:
        k, v = ln.split("=", 1)
        tr[k] = int(v.strip())
ok("MZ6 同一条不变量在 traffic 侧必须零漂移（现网实测 0；口径照 "
   "announce_main.rs:209-219，无 kind 豁免）",
   tr.get("drift", -1) == 0 and tr.get("total", 0) > 1000,
   "漂移=%s 覆盖用户=%s" % (tr.get("drift"), tr.get("total")))
ok("MZ7 traffic_ledger 的 user_id=0 锚点行须为 0（该表无外键，写了不会报错，"
   "只会被静默留在账上）", tr.get("anchor", -1) == 0,
   "锚点行=%s" % tr.get("anchor"))
TORRENT_SQL = """
WITH x AS (
 SELECT t.seeders, t.leechers, t.times_completed,
   (SELECT count(*) FROM snatches s
     WHERE s.torrent_id = t.id AND s.seeding) AS cs,
   (SELECT count(*) FROM snatches s
     WHERE s.torrent_id = t.id AND s.leeching) AS cl,
   (SELECT count(*) FROM snatches s
     WHERE s.torrent_id = t.id AND s.completed_at IS NOT NULL) AS cc
 FROM torrents t)
SELECT 'n='||count(*) FROM x UNION ALL
SELECT 'seeders='||count(*) FILTER (WHERE seeders <> cs) FROM x UNION ALL
SELECT 'leechers='||count(*) FILTER (WHERE leechers <> cl) FROM x UNION ALL
SELECT 'completed='||count(*) FILTER (WHERE times_completed <> cc) FROM x"""

tx = {}
for ln in psql_stdin(TORRENT_SQL).splitlines():
    if "=" in ln:
        k, v = ln.split("=", 1)
        tx[k] = int(v.strip())
ok("MZ8 第三腿：torrents 的三个计数快照对 snatches 权威须零漂移"
   "（口径照 announce_main.rs:228-236 / reconcile.rs:192-194）",
   tx.get("n", 0) > 0 and tx.get("seeders", -1) == 0
   and tx.get("leechers", -1) == 0 and tx.get("completed", -1) == 0,
   "种子=%s 漂移 s/l/c=%s/%s/%s"
   % (tx.get("n"), tx.get("seeders"), tx.get("leechers"),
      tx.get("completed")))

print("   [对照] traffic 侧漂移=%s / spark 侧在册漂移=%d ⇒ spark 那 232 条"
      "是写入路径的孤立问题，不是设计缺陷" % (tr.get("drift"), live))

print("   [账面] 在册漂移=%d 已处置号漂移=%d 净额=%s" % (live, purged, net))
for i in sorted(ids):
    print("      [在册 %d] %s" % (i, LIVE_DRIFT.get(i, "未登记")))
summary()
