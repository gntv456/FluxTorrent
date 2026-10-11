# -*- coding: utf-8 -*-
"""批二十六闸门 AP：复活收尾不被模块闸门误伤（P-40，2026-10-11）。

链路：module_resurrections=no（关）时，既有 open 复活契约的 settle/发奖
必须照常收尾——settle 只处理既有契约（open→done + 发火花/券/信），不给
任何人新开契约，属 _doc/契约失败流转与信誉.md §3.4「worker job 不检查模块」
原则；与 P-38 的 social_team_settle 完全同型（先例已拍板）。

先红后绿：aa1011g（未修，job 挂 resurrections）本条红；修后绿。
本闸门只碰 resurrections / spark_ledger / user_vouchers / messages 中
与探针相关的行 + module_resurrections 一个开关（收尾还原）。
"""
import os
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

CLEAN = []


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


def run_job(job, wait_s=170):
    """双通道判据：worker 被模块拦时 run_guarded 直接 bail、job_status
    不动，只有 job_triggers 回执——两处都看。"""
    t0 = q("SELECT to_char(now(), 'YYYY-MM-DD\"T\"HH24:MI:SS.US')")
    exec_sql("INSERT INTO job_triggers (job, requested_by, claimed_by) "
             "VALUES ('%s', NULL, '')" % job)
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
                return okv == "true", "started=%s %s" % (started, res)
        trow = q("SELECT status||'|'||COALESCE(ok::text,'-')||'|'||"
                 "LEFT(COALESCE(result,''),70) FROM job_triggers "
                 "WHERE job = '%s' AND requested_at >= '%s'::timestamptz "
                 "AND status IN ('done','failed') "
                 "ORDER BY id DESC LIMIT 1" % (job, t0))
        if "|" in trow:
            st, okv, res = trow.split("|", 2)
            return okv == "true", "trigger=%s %s" % (st, res)
        time.sleep(4)
    return None, "等 worker 本轮执行超时（%ss）" % wait_s


def cleanup():
    for stmt in reversed(CLEAN):
        try:
            exec_sql(stmt)
        except Exception as e:  # noqa: BLE001
            print("  [cleanup-skip] %s: %s" % (stmt[:60], str(e)[:120]))


print("== AP-S0 前置：自愈残料 + 素材（复用一个在册使用者 + 一颗探针种） ==")
# 上一轮中断（或被停）只会是本闸门造的：ApRes 探针种 / 其复活契约 / 流水券
exec_sql("DELETE FROM spark_ledger WHERE idempotency_key LIKE "
         "'resurrection:%' AND user_id IN (SELECT user_id FROM "
         "resurrections WHERE torrent_id IN (SELECT id FROM torrents "
         "WHERE name LIKE 'ApRes.%'))")
exec_sql("DELETE FROM resurrections WHERE torrent_id IN "
         "(SELECT id FROM torrents WHERE name LIKE 'ApRes.%')")
exec_sql("DELETE FROM user_vouchers WHERE source = 'resurrection' "
         "AND user_id IN (SELECT owner_id FROM torrents "
         "WHERE name LIKE 'ApRes.%')")
exec_sql("DELETE FROM seed_milestones WHERE torrent_id IN "
         "(SELECT id FROM torrents WHERE name LIKE 'ApRes.%')")
exec_sql("DELETE FROM snatches WHERE torrent_id IN "
         "(SELECT id FROM torrents WHERE name LIKE 'ApRes.%')")
exec_sql("DELETE FROM torrents WHERE name LIKE 'ApRes.%'")
RES0 = q("SELECT count(*) FROM resurrections r JOIN torrents t ON "
         "t.id = r.torrent_id WHERE t.name LIKE 'ApRes.%'")
ok("S0 起点干净（上轮残料已清）", RES0 == "0", "n=%s" % RES0)
# v2 自建三件套，不复用 AO 临时种（AO 收尾会删它，跨闸门借料=过期判据）
TS = int(time.time()) % 100000
uid = int(q("SELECT u.id FROM users u WHERE u.status < 2 ORDER BY u.id "
            "LIMIT 1"))
tid = int(q(
    "INSERT INTO torrents (info_hash, name, size, owner_id, category_id, "
    "approval_status) VALUES (md5(random()::text), 'ApRes.%d', 1024, %d, 1, 1) "
    "RETURNING id" % (TS, uid)))
CLEAN.append("DELETE FROM seed_milestones WHERE torrent_id = %d; "
             "DELETE FROM torrents WHERE id = %d" % (tid, tid))
exec_sql("INSERT INTO snatches (user_id, torrent_id, downloaded, "
         "seeded_seconds, seeding, last_port, connectable, completed_at) "
         "VALUES (%d, %d, 1024, 400*3600, TRUE, 51413, 1, now()) "
         "ON CONFLICT (user_id, torrent_id) DO UPDATE SET "
         "seeded_seconds = EXCLUDED.seeded_seconds, seeding = TRUE"
         % (uid, tid))
CLEAN.append("DELETE FROM snatches WHERE user_id = %d AND torrent_id = %d"
             % (uid, tid))
BAL0 = q("SELECT spark_balance::text FROM users WHERE id = %d" % uid)
ok("S0 素材就绪（在册用户=%d，探针种=%d）" % (uid, tid), tid > 0)

print("== AP-S1 负控面：module_resurrections=no 时既有契约仍收尾 ==")
RID = int(q(
    "INSERT INTO resurrections (torrent_id, user_id, required_hours, "
    "reward_sparks, status) VALUES (%d, %d, 400, 5000, 'open') "
    "RETURNING id" % (tid, uid)))
CLEAN.append("DELETE FROM resurrections WHERE id = %d" % RID)
CLEAN.append("DELETE FROM user_vouchers WHERE user_id = %d AND source = "
             "'resurrection'" % uid)
CLEAN.append("DELETE FROM spark_ledger WHERE idempotency_key = "
             "'resurrection:%d'" % RID)
CLEAN.append("UPDATE users SET spark_balance = %s WHERE id = %d"
             % (BAL0, uid))

MOD0 = q("SELECT COALESCE((SELECT value FROM site_settings WHERE "
         "name = 'module_resurrections'), '<absent>')")
setting("module_resurrections", "no")
if MOD0 == "<absent>":
    CLEAN.append("DELETE FROM site_settings WHERE "
                 "name = 'module_resurrections'")
else:
    CLEAN.append("UPDATE site_settings SET value = '%s' WHERE "
                 "name = 'module_resurrections'" % MOD0)

jok, jinfo = run_job("resurrection_settle")
ok("S1 模块关：job 仍真跑（不被「模块已关闭」拦死）", jok is True, jinfo)
st = q("SELECT status FROM resurrections WHERE id = %d" % RID)
ok("S1 模块关：既有契约收尾 open→done（P-40 修复点）", st == "done", st)
led = q("SELECT COALESCE(sum(amount),0)::text FROM spark_ledger WHERE "
        "idempotency_key = 'resurrection:%d'" % RID)
ok("S1 模块关：发奖落流水（5000）", led == "5000", led)
BAL1 = q("SELECT spark_balance::text FROM users WHERE id = %d" % uid)
ok("S1 模块关：余额增量=奖励", int(BAL1) - int(BAL0) == 5000,
   "before=%s after=%s" % (BAL0, BAL1))
vch = q("SELECT count(*) FROM user_vouchers WHERE user_id = %d AND "
        "source = 'resurrection'" % uid)
ok("S1 模块关：免费券 x1", vch == "1", vch)

print("== AP-S2 正控面：模块开=yes 时同样收尾（排除只在关时走通） ==")
# resurrections.torrent_id 是 UNIQUE（一种种只能一张契约，S1 崩溃当场
# 撞上限）——正控用第二颗探针种。
tid2 = int(q(
    "INSERT INTO torrents (info_hash, name, size, owner_id, category_id, "
    "approval_status) VALUES (md5(random()::text), 'ApRes2.%d', 1024, %d, "
    "1, 1) RETURNING id" % (TS, uid)))
CLEAN.append("DELETE FROM seed_milestones WHERE torrent_id = %d; "
             "DELETE FROM snatches WHERE user_id = %d AND torrent_id = %d; "
             "DELETE FROM torrents WHERE id = %d"
             % (tid2, uid, tid2, tid2))
exec_sql("INSERT INTO snatches (user_id, torrent_id, downloaded, "
         "seeded_seconds, seeding, last_port, connectable, completed_at) "
         "VALUES (%d, %d, 1024, 400*3600, TRUE, 51413, 1, now()) "
         "ON CONFLICT (user_id, torrent_id) DO UPDATE SET "
         "seeded_seconds = EXCLUDED.seeded_seconds, seeding = TRUE"
         % (uid, tid2))
RID2 = int(q(
    "INSERT INTO resurrections (torrent_id, user_id, required_hours, "
    "reward_sparks, status) VALUES (%d, %d, 400, 5000, 'open') "
    "RETURNING id" % (tid2, uid)))
CLEAN.append("DELETE FROM resurrections WHERE id = %d" % RID2)
CLEAN.append("DELETE FROM user_vouchers WHERE user_id = %d AND source = "
             "'resurrection'" % uid)
CLEAN.append("DELETE FROM spark_ledger WHERE idempotency_key = "
             "'resurrection:%d'" % RID2)
setting("module_resurrections", "yes")
jok, jinfo = run_job("resurrection_settle")
ok("S2 模块开：契约照常收尾", jok is True and
   q("SELECT status FROM resurrections WHERE id = %d" % RID2) == "done",
   "%s %s" % (jinfo, q("SELECT status FROM resurrections WHERE id = %d"
                       % RID2)))

print("== AP-S3 收尾：模块还原 + 探针清零 ==")
if MOD0 == "<absent>":
    CLEAN.append("DELETE FROM site_settings WHERE "
                 "name = 'module_resurrections'")
else:
    CLEAN.append("UPDATE site_settings SET value = '%s' WHERE "
                 "name = 'module_resurrections'" % MOD0)
cleanup()
ok("S3 module_resurrections 还原为 %s" % MOD0,
   q("SELECT COALESCE((SELECT value FROM site_settings WHERE name = "
     "'module_resurrections'),'<absent>')") == MOD0)
Z = q("SELECT count(*) FROM resurrections WHERE id IN (%d,%d)" % (RID, RID2))
ok("S3 探针契约清零", Z == "0", Z)
L = q("SELECT count(*) FROM spark_ledger WHERE idempotency_key LIKE "
      "'resurrection:%d' OR idempotency_key LIKE 'resurrection:%d'"
      % (RID, RID2))
ok("S3 探针流水清零", L == "0", L)
ok("S3 余额还原", q("SELECT spark_balance::text FROM users WHERE id = %d"
                   % uid) == BAL0)
summary()
