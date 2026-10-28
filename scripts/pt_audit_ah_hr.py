# -*- coding: utf-8 -*-
"""批十二闸门 H：H&R 执法链端到端（worker 真跑，不是读码）。

H&R 是 PT 站最容易「界面承诺、代码没接」的一条链：违规会停下载权限，
达标要自清，免罪要扣魔力。本轮把 worker 的 hr_enforce / hr_punish 经
面板入口（POST /admin/jobs/run 入队 → worker 60s tick 认领）真跑一遍，
逐出口断言：hr_snapshots / hr_violations / 站内信 / push_outbox /
snatches.hr_flag / users.download_enabled。

本文件的判据一条没动（G0 前置 → G7 收尾），只把共用工具搬到
pt_audit_ah_base.py（500 行门禁：本文件 513 行，工具块 125 行）。
`snatch` 会把清理语句追加进 base 里的同一个 CLEAN 列表，收尾照旧。

判据纪律（[[feedback-verification-visible-layer]]）：每条否定式都配
正控（同一断言在「该有」的场景必须为真），豁免与冻结两路各验各自出口。
"""
import os
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_ah_base import (  # noqa: E402
    CLEAN, ROOT, RUN, ST, exec_sql, member, q, run_job, setting, snatch,
    upload)
from pt_audit_lib import call, ok, summary  # noqa: E402



print("== G0 前置：设定收口 + 语料 ==")
ORIG = {}
for k in ST:
    ORIG[k] = q("SELECT value FROM site_settings WHERE name = '%s'" % k)
CLEAN.append("__restore_settings__")
# 原值若已经等于本闸门的探针值，说明上一次运行在中途崩了——闸门会把
# 污染值当「原值」复原，污染就被永久固化（10-18/10-19 两次崩溃实测：
# hr_days=1/hr_hours=1/cancel_hr=100/lookback=72 一直留在现网）。
SIG = {"hr_days": "1", "hr_hours": "1", "cancel_hr": "100",
       "hr_scan_lookback_hours": "72"}
FACTORY = {"hr_days": "14", "hr_hours": "120", "cancel_hr": "10000",
           "hr_scan_lookback_hours": "6"}  # 0311/0025/0278/0270
if all(ORIG.get(k) == v for k, v in SIG.items()):
    # 只 WARN 不算修好：现网因此连着几周按 hr_days=1 判违规。四把同时撞上
    # 才判污染（单把相等可能真是站长选的），改按迁移出厂值复原。
    ORIG.update(FACTORY)
    print("   [复原基线] 读到的是本闸门探针签名 ⇒ 按迁移出厂值复原：%s"
          % FACTORY)
LEFTOVER = q("SELECT count(*) FROM users WHERE username LIKE 'ahm%' "
             "AND status < 2")
if LEFTOVER not in ("", "0"):
    print("   [WARN] 更早的 AH 运行留下 %s 个未墓碑探针号（可能仍被 hr 锁住）"
          % LEFTOVER)
    # 只 WARN 不清 = 下一轮被毒害：复用的 ahm 号带着上次的未解决违规，
    # 「2/3 未达阈值不下停下载」的反控会红在残留上（2026-10-22 实测：
    # 首跑 300s 超时被 SIGTERM → 次跑 G4 反控 FAIL，enabled=false）。
    # 按 G7d 同一条产品墓碑路径收：封禁 → DELETE → 清账 → 清违规/阶梯态。
    for uid in q("SELECT id FROM users WHERE username LIKE 'ahm%' "
                 "AND status < 2").split():
        if not uid.strip():
            continue
        call("POST", "/admin/users/status",
             {"user_id": int(uid), "status": 2}, ROOT)
        call("DELETE", "/admin/users/%s" % uid, None, token=ROOT)
        exec_sql("DELETE FROM spark_ledger WHERE user_id = %s" % uid)
        exec_sql("DELETE FROM traffic_ledger WHERE user_id = %s" % uid)
        exec_sql("DELETE FROM hr_warn_states WHERE user_id = %s" % uid)
        exec_sql("DELETE FROM hr_violations WHERE user_id = %s" % uid)
        exec_sql("DELETE FROM hr_snapshots WHERE user_id = %s" % uid)
    print("   [清理] 残留探针号已按产品墓碑路径收走")
# 站点侧参数（快照与处罚都按这三档走）
setting("hr_days", "1")
setting("hr_hours", "1")
setting("hr_violation_limit", "3")
setting("enable_hr", "yes")
setting("cancel_hr", "100")
setting("hr_prewarn_hours", "0")
LB_SET = "72"
# 回看窗必须盖住探针的完成时点：hr_enforce 的快照 SQL 只处理
# `completed_at > now() - hr_scan_lookback_hours`（ZT81 增量窗口，本为
# worker 停机积压补跑）。探针造的完成记录在 3 天前，窗口若留缺省 6h，
# 快照根本不会建——判据会红在探针自己的前提上，不是站点缺陷。
setting("hr_scan_lookback_hours", "72")

MUSER, MID, MTOK = member("ahm")
# 0423 新手保护期：build_snapshots 要求 completed_at > created_at+grace_days。
# 探针把完成时间倒填到 2 天前，账号却是刚建的 ⇒ 永远不建快照（整支全红）。
# 把账号拨回 400 天模拟真实老会员；grace 上限 365，仍留 35 天余量。
exec_sql("UPDATE users SET created_at = now() - interval '400 days' "
         "WHERE id = %d" % MID)
ok("G0 探针成员可用", MID > 0 and MTOK is not None, "uid=%s" % MID)
TIDS = []
for i in (1, 2, 3):
    nm = "ahprobe r%d t%d" % (RUN, i)
    r = upload(ROOT, nm)
    tid = (r.get("data") or {}).get("id")
    if not tid:
        ok("G0 探针种可上传（%s）" % nm, False, str(r)[:160])
        sys.exit("语料不全，闸门终止")
    TIDS.append(int(tid))
    CLEAN.append("DELETE FROM promotions WHERE torrent_id = %d" % tid)
    CLEAN.append("DELETE FROM hr_violations WHERE torrent_id = %d" % tid)
    CLEAN.append("DELETE FROM hr_snapshots WHERE torrent_id = %d" % tid)
    CLEAN.append("DELETE FROM torrent_sections WHERE torrent_id = %d" % tid)
    CLEAN.append("DELETE FROM tags WHERE torrent_id = %d" % tid)
    CLEAN.append("DELETE FROM torrents WHERE id = %d" % tid)
T1, T2, T3 = TIDS
SIZE = 4096
ok("G0 三颗探针种齐", len(TIDS) == 3, str(TIDS))
CLEAN.append("UPDATE users SET status = 2 WHERE id = %d" % MID)
# 促销与 H&R 的相互作用是双向的，本闸门两头都要钉：
# ① 站内可能已有覆盖探针完成时点的免/2x 促销（实测本栈就挂着一条
#    10-07→10-11 的**全站 free**，B-01 会把所有新快照豁免掉——站点行为
#    正确、探针前提错）。先把这类促销按时点摘出探针窗口，收尾原值还原。
CONFLICT = q("SELECT string_agg(id::text || '@' || to_char(ends_at, "
             "'YYYY-MM-DD\"T\"HH24:MI:SS.US\"+00\"'), ';') FROM promotions "
             "WHERE kind IN ('free','x2free') AND torrent_id IS NULL "
             "AND starts_at <= now() - interval '2 day' "
             "AND ends_at > now() - interval '2 day'")
if CONFLICT:
    for pair in CONFLICT.split(";"):
        pid, orig = pair.split("@", 1)
        CLEAN.append("UPDATE promotions SET ends_at = '%s'::timestamptz "
                     "WHERE id = %s" % (orig, pid))
    exec_sql("UPDATE promotions SET ends_at = now() - interval '4 day' "
             "WHERE kind IN ('free','x2free') AND torrent_id IS NULL "
             "AND starts_at <= now() - interval '2 day' "
             "AND ends_at > now() - interval '2 day'")
# ② T2 单独挂「本格 free」：完成时刻落在免费窗口内应豁免（B-01 正判据）。
#    scope 必须用 'torrent'——promotions_scope_shape_check 要求
#    「scope ∈ 全局档 ⇔ torrent_id IS NULL」，配错直接被约束拒。
exec_sql("INSERT INTO promotions (kind, scope, torrent_id, starts_at, "
         "ends_at) VALUES ('free', 'torrent', %d, now() - interval '3 day', "
         "now() + interval '1 day')" % T2)
PROMO = q("SELECT max(id)::text FROM promotions WHERE torrent_id = %d" % T2)
CLEAN.append("DELETE FROM promotions WHERE id = %s" % PROMO)
CLEAN.append("DELETE FROM messages WHERE subject IN "
             "('H&R 违规确认','H&R 违规已消除','下载权限暂停：H&R 违规超限') "
             "AND receiver_id = %d" % MID)
CLEAN.append("DELETE FROM push_outbox WHERE topic = 'hr' AND user_id = %d"
             % MID)

print("== G1 违规产生：快照→violation→PM→推送→角标 ==")
snatch(MID, T1, completed_ago_days=2, seeded=0, downloaded=SIZE)
snatch(MID, T2, completed_ago_days=2, seeded=0, downloaded=SIZE)
jok, jres = run_job("hr_enforce")
ok("G1 worker 真跑完 hr_enforce", jok is True, "ok=%s %s" % (jok, jres))
st1 = q("SELECT status FROM hr_snapshots WHERE user_id = %d AND torrent_id = %d"
        % (MID, T1))
ok("G1 逾期且未达标 → 快照 violated", st1 == "violated", "status=%s" % st1)
req_s = q("SELECT required_seconds::text FROM hr_snapshots "
          "WHERE user_id = %d AND torrent_id = %d" % (MID, T1))
ok("G1 义务时长按站点设定 hr_hours=1 计算", req_s == "3600",
   "required=%s" % req_s)
viol = q("SELECT count(*) FROM hr_violations WHERE user_id = %d "
         "AND torrent_id = %d AND resolved_at IS NULL" % (MID, T1))
ok("G1 违规落表（未解决）", viol == "1", "rows=%s" % viol)
pm = q("SELECT count(*) FROM messages WHERE receiver_id = %d "
       "AND subject = 'H&R 违规确认'" % MID)
ok("G1 违规站内信到达", pm == "1", "rows=%s" % pm)
push = q("SELECT count(*) FROM push_outbox WHERE user_id = %d "
         "AND dedupe_key = 'hr_violation:%d:%d'" % (MID, T1, MID))
ok("G1 Web Push 入 outbox", push == "1", "rows=%s" % push)
flag = q("SELECT hr_flag::text FROM snatches WHERE user_id = %d "
         "AND torrent_id = %d" % (MID, T1))
ok("G1 列表角标位 hr_flag 刷新", flag == "true", "hr_flag=%s" % flag)
st2 = q("SELECT count(*) FROM hr_snapshots WHERE user_id = %d "
        "AND torrent_id = %d" % (MID, T2))
ok("G1 免费窗口内完成 → 不建快照（B-01 豁免，反向）", st2 == "0",
   "rows=%s" % st2)

print("== G2 用户侧读口 /me/hr ==")
s, r = call("GET", "/me/hr", token=MTOK)
rows = ((r.get("data") or {}).get("rows") or [])
mine = [x for x in rows if x.get("torrent_id") == T1]
ok("G2 /me/hr 看得到自己这条 violated", s == 200 and mine
   and mine[0].get("status") == "violated", "http=%s rows=%d" % (s, len(rows)))
ok("G2 读口回传的默认口径与站点设定一致",
   (r.get("data") or {}).get("default_seed_hours") == 1,
   "default_seed_hours=%s" % (r.get("data") or {}).get("default_seed_hours"))

print("== G3 达标自清（承诺「持续做种即消除」成立）==")
exec_sql("UPDATE snatches SET seeded_seconds = 7200 WHERE user_id = %d "
         "AND torrent_id = %d" % (MID, T1))
jok, jres = run_job("hr_enforce")
ok("G3 hr_enforce 二轮跑完", jok is True, "ok=%s %s" % (jok, jres))
st1 = q("SELECT status FROM hr_snapshots WHERE user_id = %d AND torrent_id = %d"
         % (MID, T1))
ok("G3 达标后快照转 satisfied", st1 == "satisfied", "status=%s" % st1)
viol = q("SELECT count(*) FROM hr_violations WHERE user_id = %d "
         "AND torrent_id = %d AND resolved_at IS NOT NULL" % (MID, T1))
ok("G3 违规标记已解决", viol == "1", "rows=%s" % viol)
pm = q("SELECT count(*) FROM messages WHERE receiver_id = %d "
       "AND subject = 'H&R 违规已消除'" % MID)
ok("G3 自清后补发说明信", pm == "1", "rows=%s" % pm)

print("== G4 停下载与自动恢复 ==")
# G3 把 T1 做成「达标」了：hr_enforce 每轮都会按 snatch 现值把达标者判回
# satisfied 并再销一次违规（resolve_met_violations，0286 的正道）。所以凑
# 三条之前必须先让 T1 回到未达标，否则手工 flip 立刻被合法回销——上一轮
# rows=2 就是这么来的（站点没错，是探针自相矛盾）。
exec_sql("UPDATE snatches SET seeded_seconds = 0 WHERE user_id = %d "
         "AND torrent_id = %d" % (MID, T1))
# 直接把 T2/T3 造逾期违规：先清掉 T2 的 free 促销（豁免只在建快照时判定一次）
exec_sql("UPDATE hr_snapshots SET status = 'violated' WHERE user_id = %d "
         "AND torrent_id = %d" % (MID, T1))
exec_sql("UPDATE hr_violations SET resolved_at = NULL WHERE user_id = %d "
         "AND torrent_id = %d" % (MID, T1))
snatch(MID, T3, completed_ago_days=2, seeded=0, downloaded=SIZE)
jok, jres = run_job("hr_enforce")
ok("G4 第三颗逾期违规产生", jok is True and q(
   "SELECT count(*) FROM hr_violations WHERE user_id = %d AND torrent_id = %d"
   % (MID, T3)) == "1", "ok=%s %s" % (jok, jres))
# 现值 2 条未解决（T1+T3）< 阈值 3 → 不该被停
jok, jres = run_job("hr_punish")
# 0388 警告阶梯与 hr_violation_limit 是两套并行策略：阶梯第 2 档就可能
# 先关下载（state.restrict_download=true），不能把「已关」一律读成 hr_punish
# 越界。硬处罚越界的判据＝没关，或关的人确实是阶梯。
lock = q("SELECT download_enabled::text||'|'||"
         "COALESCE(download_locked_by,'-') FROM users WHERE id = %d" % MID)
warn_lock = q("SELECT count(*) FROM hr_warn_states WHERE user_id = %d "
              "AND restrict_download AND expires_at > now()" % MID)
ok("G4 未达硬阈值(2/3) → hr_punish 不越界（阶梯限下属预期）",
   lock.startswith("true") or warn_lock == "1",
   "lock=%s 阶梯限下=%s" % (lock, warn_lock))
# 补到 3 条：T2 撤促销后重建快照再判
exec_sql("DELETE FROM promotions WHERE id = %s" % PROMO)
# 自解释判据：撤完促销后若还有任何 free/x2free 盖住 T2 的完成时点，
# 快照就不会建——把盖住它的到底是哪条打出来（否则下一轮又是一条猜谜红）
COVER = q("SELECT string_agg(id::text||':'||scope||'/'||kind||"
          "'['||to_char(starts_at,'MM-DD HH24')||'→'||"
          "to_char(ends_at,'MM-DD HH24')||']', ' ') FROM promotions p "
          "WHERE p.kind IN ('free','x2free') AND "
          "(p.torrent_id = %d OR (p.torrent_id IS NULL AND ("
          " p.scope = 'global' OR (p.scope = 'official' AND "
          "  (SELECT official_tag FROM torrents WHERE id = %d)) OR "
          " (p.scope = 'non_official' AND NOT "
          "  (SELECT official_tag FROM torrents WHERE id = %d)) OR "
          " (p.scope = 'category' AND p.category_id = "
          "  (SELECT category_id FROM torrents WHERE id = %d))))) AND "
          "p.starts_at <= (SELECT completed_at FROM snatches WHERE "
          "user_id = %d AND torrent_id = %d) AND p.ends_at > "
          "(SELECT completed_at FROM snatches WHERE user_id = %d "
          "AND torrent_id = %d)"
          % (T2, T2, T2, T2, MID, T2, MID, T2))
ok("G4 撤促销后 T2 完成时点已无覆盖性促销（前提自证）",
   COVER in ("", "NULL"), "covering=%s" % (COVER or "无"))
# 断言前重新钉设定并回读：只看一次写过的值会被任何并发写手静默改掉
setting("hr_scan_lookback_hours", LB_SET)
print("   [前提回读] lookback=%s" % q("SELECT value FROM site_settings "
                                     "WHERE name = 'hr_scan_lookback_hours'"))
jok, jres = run_job("hr_enforce")
# 撤促销后补建可能不在同一轮 tick 内完成（hr_enforce 每分钟一轮，且
# 与手动触发抢同一把 advisory 锁）：跨 ≤3 轮等状态出现，避免把「单轮
# 假设」的探针时序错报成站点缺陷。
three = "0"
for _ in range(3):
    three = q("SELECT count(*) FROM hr_violations WHERE user_id = %d "
              "AND resolved_at IS NULL" % MID)
    if three == "3":
        break
    time.sleep(65)
    jok, jres = run_job("hr_enforce")
ok("G4 三条逾期违规到位", three == "3", "rows=%s" % three)
jok, jres = run_job("hr_punish")
row = q("SELECT download_enabled::text||'|'||COALESCE(download_locked_by,'-') "
        "FROM users WHERE id = %d" % MID)
ok("G4 超限 → 停下载并记归因 hr", row == "false|hr", "row=%s" % row)
# 停下载有两条告知路径：① 违规超限处罚（"下载权限暂停：H&R 违规超限"）；
# ② 警告阶梯第 2 档起自带限下（"H&R 警告（第 N 档）"，正文写明「已暂停：
# 下载」）。探针凑够三条违规时，阶梯往往**先一步**把下载关掉，处罚口看到
# download_enabled 已经是 false 就不再重复发信 ⇒ 只认处罚信会假红。
pm = q("SELECT count(*) FROM messages WHERE receiver_id = %d "
       "AND (subject = '下载权限暂停：H&R 违规超限' "
       "OR subject LIKE 'H&R 警告（第 %%')" % MID)
ok("G4 停下载有站内信告知（处罚信或阶梯警告信）", pm >= "1", "rows=%s" % pm)
s, r = call("GET", "/me/hr", token=MTOK)
ok("G4 停下载后本人读口仍可用（不 500）", s == 200, "http=%s" % s)

print("== G5 自助免罪（扣魔力即销账，重放不双花）==")
exec_sql("UPDATE users SET spark_balance = 500 WHERE id = %d" % MID)
s, r = call("POST", "/me/hr/pardon", {"torrent_id": T3}, token=MTOK)
ok("G5 免罪成功且按 cancel_hr=100 计价",
   s == 200 and (r.get("data") or {}).get("cost") == 100,
   "http=%s %s" % (s, str(r)[:120]))
bal = q("SELECT spark_balance::text FROM users WHERE id = %d" % MID)
ok("G5 只扣一次 100", bal == "400", "balance=%s" % bal)
v3 = q("SELECT count(*) FROM hr_violations WHERE user_id = %d "
       "AND torrent_id = %d AND resolved_at IS NOT NULL" % (MID, T3))
ok("G5 对应违规已解决", v3 == "1", "rows=%s" % v3)
sp = q("SELECT status FROM hr_snapshots WHERE user_id = %d AND torrent_id = %d"
       % (MID, T3))
ok("G5 快照转 pardoned", sp == "pardoned", "status=%s" % sp)
s2, r2 = call("POST", "/me/hr/pardon", {"torrent_id": T3}, token=MTOK)
bal2 = q("SELECT spark_balance::text FROM users WHERE id = %d" % MID)
ok("G5 重放被拒且不退款/不再扣（幂等）",
   s2 != 200 and bal2 == bal, "http=%s balance=%s" % (s2, bal2))
# pardon 掉 T3 后只剩 T1/T2 两条未解决违规（2 < 阈值 3）→ 阈值这条线该放人
#
# 但**阶梯限下在位时不能放**：P-445 明确「阶梯第 2 档起的『限下』有自己的
# expires_at」，restore_hr_locks 遇到 hr_warn_states.step>0 且
# restrict_download 时**故意不放人**（否则 14/30 天的锁会被一分钟抹掉）。
# 这条承诺也写在停下载通知里：「违规数降回阈值以下后下载权限将自动恢复
#（若处于 H&R 警告限制期，则待警告到期后再解）」。
#
# 原判据只写「降回阈值 → 自动恢复」，没排除阶梯在位 ⇒ 实测假红
#（2026-10-20：worker 日志显示阶梯已推进，restore 被 P-445 挡住；
#  隔离复现：阶梯在位 → 不恢复；阶梯状态清掉 → 恢复）。
# 这里把两段拆开判：阶梯在位 → 保持关闭（阶梯期限不被抹掉）；
# 阶梯状态清掉（模拟到期）→ 下一轮必须恢复（阈值路径单独承重）。
# 阶梯状态**自己钉**，不靠抢时序：阶梯作业（hr_warn_ladder，每 5 分钟一轮）
# 会随时把探针重新推到限下档，两种写法都会踩竞态——
#   ① 作业前读状态：阶梯还没推进 → 读成「不在位」（首轮实测就是假红）；
#   ② 作业后读状态：定时阶梯作业正好在读之前跑过 → 读成「在位」但
#      那一刻其实还没限下（同样假红）。
# 所以：① 段自己把阶梯钉成限下在位；② 段自己把阶梯清干净，
# 若中途又被定时阶梯作业推回限下（可观测：状态行又长回来了），
# 再清一次重跑，最多 4 轮（阶梯 5 分钟一轮，4 轮内必有一次干净窗口）。
LADDER_ON_SQL = (
    "INSERT INTO hr_warn_states (user_id, step, base_days, expires_at,"
    " restrict_download, block_upload, block_invite) "
    "VALUES (%d, 2, 7, now() + interval '14 days', true, false, false) "
    "ON CONFLICT (user_id) DO UPDATE SET step=2, "
    " expires_at=now() + interval '14 days', restrict_download=true, "
    " block_upload=false, block_invite=false" % MID)
LADDER_OFF_SQL = (
    "UPDATE hr_warn_states SET step=0, expires_at=NULL, "
    " restrict_download=false, block_upload=false, block_invite=false "
    "WHERE user_id = %d" % MID)
exec_sql(LADDER_ON_SQL)
jok, jres = run_job("hr_punish")
de = q("SELECT download_enabled::text FROM users WHERE id = %d" % MID)
warn = q("SELECT COALESCE(step::text,'-')||'|'||"
          "COALESCE(restrict_download::text,'f') FROM hr_warn_states"
          " WHERE user_id = %d" % MID)
ok("G5a 阶梯限下在位 → 阈值恢复被挡住（P-445：阶梯期限不被抹掉）",
   de == "false", "enabled=%s warn=%s %s" % (de, warn, jres))
restored = False
detail = ""
for _try in range(4):
    exec_sql(LADDER_OFF_SQL)
    jok, jres = run_job("hr_punish")
    de = q("SELECT download_enabled::text FROM users WHERE id = %d" % MID)
    detail = "enabled=%s %s" % (de, jres)
    if de == "true":
        restored = True
        break
    time.sleep(3)
ok("G5b 阶梯状态清掉（模拟到期）→ 下载权限自动恢复",
   restored, detail)

print("== G6 假开关反向：enable_hr=no 整段冻结 ==")
exec_sql("DELETE FROM hr_snapshots WHERE user_id = %d" % MID)
exec_sql("DELETE FROM hr_violations WHERE user_id = %d" % MID)
setting("enable_hr", "no")
# T2 在 G1 已插过 snatch（当时被单种 free 豁免），这里要重用得先删旧行——
# snatches 主键就是 (user_id, torrent_id)，重复 INSERT 直接撞约束。
exec_sql("DELETE FROM snatches WHERE user_id = %d AND torrent_id = %d"
         % (MID, T2))
snatch(MID, T2, completed_ago_days=2, seeded=0, downloaded=SIZE)
jok, jres = run_job("hr_enforce")
n = q("SELECT count(*) FROM hr_snapshots WHERE user_id = %d" % MID)
ok("G6 enable_hr=no 时不建快照也不判违规（设定真被消费）", n == "0",
   "rows=%s" % n)
setting("enable_hr", "yes")
jok, jres = run_job("hr_enforce")
n2 = q("SELECT count(*) FROM hr_snapshots WHERE user_id = %d "
       "AND torrent_id = %d" % (MID, T2))
ok("G6 复开后同一条能建上（正控，防 G6 恒真）", n2 == "1", "rows=%s" % n2)

print("== G7 收尾 ==")
for stmt in reversed(CLEAN):
    if stmt == "__restore_settings__":
        for k, v in ORIG.items():
            if v:
                setting(k, v)
            else:
                exec_sql("DELETE FROM site_settings WHERE name = '%s'" % k)
        continue
    exec_sql(stmt)
MINE = q("SELECT count(*) FROM torrents WHERE name LIKE 'ahprobe r%d t%%'"
         % RUN)
ok("G7 本轮探针种无残留", MINE == "0", "rows=%s" % MINE)
OLD = q("SELECT count(*) FROM torrents WHERE name LIKE 'ahprobe%'")
if OLD not in ("", "0"):
    # 不代删：可能是并行在跑的另一轮；本轮前缀已独立，旧账只喊不判红
    print("   [WARN] 更早的 AH 运行留下 %s 条 ahprobe 探针种" % OLD)
ok("G7 快照/违规无残留", q("SELECT (SELECT count(*) FROM hr_snapshots "
                           "WHERE user_id = %d) + (SELECT count(*) FROM "
                           "hr_violations WHERE user_id = %d)" % (MID, MID))
   == "0", "residue")
ok("G7 站点设定已复原", q("SELECT value FROM site_settings WHERE "
                          "name = 'hr_days'") == ORIG["hr_days"],
   "hr_days=%s" % q("SELECT value FROM site_settings WHERE name='hr_days'"))
# G7d 探针成员必须走产品自己的墓碑：本闸门把 MID 封成 status=2 取证后就没人管了
# （2026-10-19 全站账实体检实测：库里积下 21 个 `ahm*@ah.local` 停在封禁态、
#  余额 -100 —— 成员目录虽然挡住了它们，后台用户列表与统计却长期被污染）。
# 对齐 BK 闸门 K11c 的既有做法：封禁 → DELETE /admin/users → 删流水 → 断言残留。
if MID:
    call("POST", "/admin/users/status", {"user_id": MID, "status": 2}, ROOT)
    call("DELETE", "/admin/users/%d" % MID, None, token=ROOT)
    exec_sql("DELETE FROM spark_ledger WHERE user_id = %d" % MID)
    exec_sql("DELETE FROM traffic_ledger WHERE user_id = %d" % MID)
    # 阶梯状态行也要收：它挂在探针号上会长期留在 hr_warn_states 里
    #（现网实测：49 行里 41 行属于已墓碑的探针号），
    # 还会挡住「同一探针能生成邀请码」这类以 hr_warn_states 为前提的判据。
    exec_sql("DELETE FROM hr_warn_states WHERE user_id = %d" % MID)
ok("G7d 探针成员已走产品墓碑（status>=2 且 passkey 变 deleted-*；余额被 "
   "user_purge 冲平属收尾留痕，不是本闸门的账）",
   q("SELECT count(*) FROM users WHERE id = %d AND status >= 2 AND "
     "passkey LIKE 'deleted-%%'" % MID) == "1"
   and q("SELECT count(*) FROM spark_ledger WHERE user_id = %d" % MID) == "0",
   "tomb=%s led=%s" % (q("SELECT count(*) FROM users WHERE id = %d AND "
                         "status >= 2 AND passkey LIKE 'deleted-%%'" % MID),
                       q("SELECT count(*) FROM spark_ledger WHERE user_id = %d"
                         % MID)))
summary()

