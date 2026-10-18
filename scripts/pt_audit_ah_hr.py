# -*- coding: utf-8 -*-
"""批十二闸门 H：H&R 执法链端到端（worker 真跑，不是读码）。

H&R 是 PT 站最容易「界面承诺、代码没接」的一条链：违规会停下载权限，
达标要自清，免罪要扣魔力。本轮把 worker 的 hr_enforce / hr_punish 经
面板入口（POST /admin/jobs/run 入队 → worker 60s tick 认领）真跑一遍，
逐出口断言：hr_snapshots / hr_violations / 站内信 / push_outbox /
snatches.hr_flag / users.download_enabled。

判据纪律（[[feedback-verification-visible-layer]]）：每条否定式都配
正控（同一断言在「该有」的场景必须为真），豁免与冻结两路各验各自出口。
"""
import os
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_form import BOUNDARY, make_torrent, multipart  # noqa: E402
from pt_audit_lib import BASE, call, login, ok, summary  # noqa: E402

import json
import urllib.error
import urllib.request

ROOT = login()
CLEAN = []
ST = ("hr_days", "hr_hours", "hr_violation_limit", "enable_hr",
      "cancel_hr", "hr_prewarn_hours", "hr_scan_lookback_hours")


def q(sql_text):
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", sql_text],
        capture_output=True, text=True, encoding="utf-8")
    return p.stdout.strip()


def exec_sql(sql_text):
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-c", sql_text],
        capture_output=True, text=True, encoding="utf-8")
    if p.returncode:
        raise RuntimeError("psql 失败: %s" % (p.stderr or "")[:220])


def setting(name, value):
    exec_sql("INSERT INTO site_settings (name, value) VALUES ('%s', '%s') "
             "ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value"
             % (name, value))


def upload(tok, name):
    body = multipart({"name": name, "category_id": "1",
                      "descr": "H&R 探针正文，长度足够过常规校验",
                      "small_descr": "ah"},
                     make_torrent(name), filename="ah.torrent")
    req = urllib.request.Request(BASE + "/torrents", method="POST",
                                 data=body)
    req.add_header("Content-Type",
                   "multipart/form-data; boundary=%s" % BOUNDARY)
    req.add_header("Authorization", "Bearer " + tok)
    try:
        r = urllib.request.urlopen(req, timeout=40)
        return json.loads(r.read())
    except urllib.error.HTTPError as e:
        return {"status": e.code,
                "raw": e.read()[:200].decode("utf-8", "replace")}


def run_job(job, wait_s=170):
    """确认「改完数据之后 worker 真的跑过一轮该任务」。

    hr_enforce / hr_punish 的 cadence 是 60s（catalog.rs），面板入队
    （/admin/jobs/run）与定时触发抢同一把 advisory 锁，谁先到谁跑——
    手动那一条经常以「锁未抢到」失败，这是良态竞争，不是缺陷。
    判据因此钉在 `job_status.last_started_at` 前移且 last_ok=true 上：
    这证明的是 worker 本体（同一份 SQL）在本步骤之后执行过，
    至于是定时还是手动认领，与断言无关。
    """
    t0 = q("SELECT to_char(now(), "
           "'YYYY-MM-DD\"T\"HH24:MI:SS.US')")
    call("POST", "/admin/jobs/run", {"job": job}, token=ROOT)
    deadline = time.time() + wait_s
    while time.time() < deadline:
        row = q("SELECT to_char(last_started_at, "
                "'YYYY-MM-DD\"T\"HH24:MI:SS.US')||'|'||"
                "COALESCE(last_ok::text,'-')||'|'||"
                "LEFT(COALESCE(last_result,''),60) FROM job_status "
                "WHERE job = '%s'" % job)
        if "|" in row:
            started, okv, res = row.split("|", 2)
            if started > t0:
                return (okv == "true"), "started=%s %s" % (started, res)
        time.sleep(4)
    return None, "等待 worker 本轮执行超时（%ss）" % wait_s


def member(tag):
    """普通成员：adduser + 先改初始密码（避开登录限流与弱口令门）"""
    user = "%s%d" % (tag, int(time.time()) % 100000)
    pw = "Gate#123456"
    call("POST", "/admin/adduser", {"username": user, "password": pw,
                                    "email": "%s@ah.local" % user},
         token=ROOT)
    uid = q("SELECT id::text FROM users WHERE username = '%s'" % user)
    t0 = login(user, pw)
    call("POST", "/me/password/change",
         {"old_password": pw, "new_password": pw + "x"}, token=t0)
    return user, int(uid), login(user, pw + "x")


def snatch(uid, tid, completed_ago_days, seeded, downloaded):
    # 完成时点取 2 天前：既要落在 hr_scan_lookback_hours（本闸门设 72）之内
    # ——ZT81 增量窗口按 `completed_at > now() - lookback` 过滤，正好压 72h
    # 边沿会因为 now() 前进而永久出局；又要晚于 hr_days=1 的期限，好让
    # 「逾期未达标」一轮就成立。
    exec_sql("INSERT INTO snatches (user_id, torrent_id, downloaded, "
             "seeded_seconds, completed_at, progress, last_seen_at) VALUES "
             "(%d, %d, %d, %d, now() - interval '%d day', 100, now())"
             % (uid, tid, downloaded, seeded, completed_ago_days))
    CLEAN.append("DELETE FROM snatches WHERE user_id = %d AND torrent_id = %d"
                 % (uid, tid))


print("== G0 前置：设定收口 + 语料 ==")
ORIG = {}
for k in ST:
    ORIG[k] = q("SELECT value FROM site_settings WHERE name = '%s'" % k)
CLEAN.append("__restore_settings__")
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
ok("G0 探针成员可用", MID > 0 and MTOK is not None, "uid=%s" % MID)
TIDS = []
for i in (1, 2, 3):
    nm = "ahprobe t%d %d" % (i, int(time.time()))
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
de = q("SELECT download_enabled::text FROM users WHERE id = %d" % MID)
ok("G4 未达阈值(2/3) → 不下停下载（反向）", de == "true", "enabled=%s" % de)
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
pm = q("SELECT count(*) FROM messages WHERE receiver_id = %d "
       "AND subject = '下载权限暂停：H&R 违规超限'" % MID)
ok("G4 停下载说明信到达", pm == "1", "rows=%s" % pm)
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
# 阈值降回 2 → 下一轮自动恢复下载
jok, jres = run_job("hr_punish")
de = q("SELECT download_enabled::text FROM users WHERE id = %d" % MID)
ok("G5 违规降回阈值 → 下载权限自动恢复", de == "true", "enabled=%s" % de)

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
ok("G7 探针种无残留", q("SELECT count(*) FROM torrents WHERE "
                        "name LIKE 'ahprobe%'") == "0",
   q("SELECT count(*) FROM torrents WHERE name LIKE 'ahprobe%'"))
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
