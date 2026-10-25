# -*- coding: utf-8 -*-
"""批二十六闸门 AO：新人考核引擎端到端（0093/0105，worker 真跑）。

链路：exam_assign 自动派发（onboard 窗口 + PM）→ /me/exams 进度读口
→ task_settle 达标结算（发奖 + 转正信 + 重跑幂等）→ admin 豁免/恢复
→ module_exams 只拦「派发新考核」，不拦「收尾在途认领」。

本批修复哨兵（P-39）：派发 SQL 注释写「未删号」，实际 JOIN 无 status
过滤——封号(2)/软删(3)/退役(4) 账号会被一并派发并收 PM。哨兵断言
status>=2 的账号 claim 数为 0，同时 status<2 侧必须有正控（>0）。
先红后绿：aa1011g（未修）上本条红，修后绿。

面：onboard 分支走真窗口（注册 ≤30 天），本闸门会向窗口内全部演示
账号派发；S4 收尾把 claims/私信/流水/余额全部清回（开跑前先自愈
上一轮中断的残料——本栈 exams 从未真跑过，这类行只能是本闸门造的）。
"""
import os
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402

ROOT = login()
CLEAN = []
TS = int(time.time()) % 100000
PW = "Gate#123456"
N_A = "aoA%d" % TS      # 正常成员：派发→达标→结算
N_B = "aoB%d" % TS      # 封号成员：不得被派发（P-39 哨兵）
N_C = "aoC%d" % TS      # 豁免/恢复流转
N_D = "aoD%d" % TS      # 模块负控：关着不派、开了才派
TASK_KINDS = "('onboard','periodic')"


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


def uid_of(name):
    return int(q("SELECT id::text FROM users WHERE username = '%s'" % name))


def mkuser(name):
    call("POST", "/admin/adduser", {"username": name, "password": PW,
                                    "email": "%s@ao.local" % name},
         token=ROOT)
    u = uid_of(name)
    tok = login(name, PW)
    call("POST", "/me/password/change", {"old_password": PW,
                                         "new_password": PW + "x"},
         token=tok)
    CLEAN.append("UPDATE users SET status = 2 WHERE id = %d" % u)
    # 结算类副作用：整点 achievement_grant 可能给探针挂成就行，单独清
    CLEAN.append("DELETE FROM user_achievements WHERE user_id = %d" % u)
    # class_adj 60s tick 会按探针的 snatch 数据把 class_id 升上去（本批实测
    # 3 个探针 0→1）；晋升奖/信件只在 class_rules.promo_sparks>0 时产生，
    # 这里都按历史快照还原，防将来改了 promo_sparks 后闸门互相污染。
    cls = q("SELECT class_id::text FROM users WHERE id = %d" % u)
    CLEAN.append("DELETE FROM spark_ledger WHERE user_id = %d AND "
                 "kind = 'class_promotion'" % u)
    CLEAN.append("DELETE FROM messages WHERE receiver_id = %d AND "
                 "subject LIKE '%%晋升%%'" % u)
    CLEAN.append("UPDATE users SET class_id = %s WHERE id = %d"
                 % (cls, u))
    bal = q("SELECT spark_balance::text FROM users WHERE id = %d" % u)
    CLEAN.append("DELETE FROM spark_ledger WHERE user_id = %d AND "
                 "(idempotency_key LIKE 'task_settle:%%' OR "
                 "idempotency_key LIKE 'task_penalty:%%')" % u)
    CLEAN.append("UPDATE users SET spark_balance = %s WHERE id = %d"
                 % (bal, u))
    return u, login(name, PW + "x")


def run_job(job, wait_s=170):
    """入队并等 worker 本体真跑一轮（判据钉 last_started_at 前移）。

    双通道判据：worker 被模块闸门拦下时 run_guarded 直接 bail、
    job_status 纹丝不动（manual.rs L151 拦截路径）——只钉 started_at
    会干等超时。故同时看本轮 trigger 行的终态（done/failed 都算回执）。
    """
    t0 = q("SELECT to_char(now(), 'YYYY-MM-DD\"T\"HH24:MI:SS.US')")
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
                return okv == "true", "started=%s %s" % (started, res)
        trow = q("SELECT to_char(requested_at, "
                 "'YYYY-MM-DD\"T\"HH24:MI:SS.US')||'|'||status||'|'||"
                 "COALESCE(ok::text,'-')||'|'||"
                 "LEFT(COALESCE(result,''),60) FROM job_triggers "
                 "WHERE job = '%s' AND requested_at >= '%s'::timestamptz "
                 "AND status IN ('done','failed') ORDER BY id DESC "
                 "LIMIT 1" % (job, t0))
        if "|" in trow:
            req_at, st, okv, res = trow.split("|", 3)
            if st == "done":
                return okv == "true", "trigger=%s %s" % (st, res)
            return False, "trigger=%s %s" % (st, res)
        time.sleep(4)
    return None, "等 worker 本轮执行超时（%ss）" % wait_s


def wait_claim(claim_id, want, wait_s=200):
    """结算要等轮次；批量 LIMIT 可能把探针排到后一轮——轮询别睡死。"""
    end = time.time() + wait_s
    while time.time() < end:
        if q("SELECT status FROM task_claims WHERE id = %d"
             % claim_id) == want:
            return True
        time.sleep(5)
    return False


def probe_torrent(owner_id, tag):
    name = "AoExam.%s.%d" % (tag, TS)
    tid = int(q(
        "INSERT INTO torrents (info_hash, name, size, owner_id, "
        "category_id, approval_status) VALUES (md5(random()::text), "
        "'%s', 1024, %d, 1, 1) RETURNING id" % (name, owner_id)))
    # 删种必须单语句原子「拆卫星+删种」：worker 的 collect_milestones
    # 60s tick 随时可能再插 seed_milestones——两条独立 CLEAN 语句之间
    # 被塞行，第二条就被 FK 挡（三跑实测：单删语句照旧 skip）
    # 拆卫星+删种必须**同事务**：worker 的 collect_milestones 60s tick
    # 随时会再插 seed_milestones，两条独立语句之间被塞行，第二条就被 FK
    # 挡（三跑实测：单删语句照旧 skip）⇒ 收进一个 BEGIN/COMMIT，
    # 让 worker 的插入排在事务后面，彻底断掉这个竞态。
    CLEAN.append("BEGIN; DELETE FROM seed_milestones WHERE torrent_id = %d; "
                 "DELETE FROM torrents WHERE id = %d; COMMIT;" % (tid, tid))
    return tid


def snatch_touch(u, tid, seed_secs):
    exec_sql(
        "INSERT INTO snatches (user_id, torrent_id, downloaded, "
        "seeded_seconds, seeding, completed_at) "
        "VALUES (%d, %d, 1024, %d, TRUE, now()) "
        "ON CONFLICT (user_id, torrent_id) DO UPDATE SET "
        "seeded_seconds = EXCLUDED.seeded_seconds, seeding = TRUE"
        % (u, tid, seed_secs))
    CLEAN.append("DELETE FROM snatches WHERE user_id = %d AND torrent_id = %d"
                 % (u, tid))


def claim_of(u):
    """(claim_id, status) 或 None；只取 onboard 认领。

    S2/S3 的结算断言针对「转正考核」这条（基线 0、量可达）；periodic 探针
    门槛 864000 是给「派发面」用的，若 claim_of 取最新（periodic 后插、
    id 更大）会让 wait_claim 永远等不到结算而假红。
    """
    row = q("SELECT c.id::text||'|'||c.status::text FROM task_claims c "
            "JOIN tasks t ON t.id = c.task_id WHERE c.user_id = %d AND "
            "t.kind = 'onboard' ORDER BY c.id DESC LIMIT 1" % u)
    if "|" not in row:
        return None
    cid, st = row.split("|", 1)
    return int(cid), int(st)


def ledger_sum(cid):
    return q("SELECT COALESCE(sum(amount),0)::text FROM spark_ledger WHERE "
             "idempotency_key = 'task_settle:%d'" % cid)


def cleanup():
    """收尾：每条清理语句都要成功；失败要**红**，不许静默跳过。

    旧写法把异常吞成一行 [cleanup-skip] ⇒ 探针种留在库里，
    别的闸门（HZ6/IE3 的「过审 + 无存档」棘轮）随后集体变红，
    查起来完全看不出是本闸门留下的残料。
    """
    failed = []
    for stmt in reversed(CLEAN):
        try:
            exec_sql(stmt)
        except Exception as e:  # noqa: BLE001
            failed.append((stmt[:60], str(e)[:120]))
            print("  [cleanup-FAIL] %s: %s" % (stmt[:60], str(e)[:120]))
    return failed


print("== S0 前置：自愈残料 + module_exams 开 + 探针 ==")
# 本栈 exams 从未真跑（module 关、claim 表空）；此类行只可能是本闸门
# 上一轮中断留下的——先清再用，保证「派发总数」类判据可解释。
print("  [pre] 考核 claims=%s 考核私信=%s"
      % (q("SELECT count(*) FROM task_claims c JOIN tasks t ON "
           "t.id=c.task_id WHERE t.kind IN %s" % TASK_KINDS),
         q("SELECT count(*) FROM messages WHERE subject IN "
           "('考核已派发','转正考核通过')")))
exec_sql("DELETE FROM task_claims WHERE task_id IN "
         "(SELECT id FROM tasks WHERE kind IN %s)" % TASK_KINDS)
exec_sql("DELETE FROM messages WHERE subject IN "
         "('考核已派发','转正考核通过')")
exec_sql("DELETE FROM tasks WHERE name LIKE 'AO周期%'")
ok("S0 起点干净（上轮残料已清）",
   q("SELECT count(*) FROM task_claims c JOIN tasks t ON "
     "t.id=c.task_id WHERE t.kind IN %s" % TASK_KINDS) == "0")

MOD0 = q("SELECT COALESCE((SELECT value FROM site_settings "
         "WHERE name = 'module_exams'), '<absent>')")
setting("module_exams", "yes")
ok("S0 module_exams 已开（收尾还原 %s）" % MOD0,
   q("SELECT value FROM site_settings WHERE name = "
     "'module_exams'") == "yes")

uA, tA = mkuser(N_A)
uB, tB = mkuser(N_B)
exec_sql("UPDATE users SET status = 2 WHERE id = %d" % uB)
# 哨兵的时间基准：封号生效的那一刻。worker 自己每 60s 跑一轮
# exam_assign，探针建号到封号之间有 ~1s 窗口——那一瞬间派发出的认领
# 是**合法**的（账号当时还没被封），但哨兵按「当前 status」回看，
# 会把这种历史认领算成「封号账号被派发」⇒ 假红。判据改成
# 「封号之后新建的认领才算违规」。
BAN_T = q("SELECT to_char(now(), 'YYYY-MM-DD\"T\"HH24:MI:SS.US')")
uC, tC = mkuser(N_C)
# 周期分支探针任务：target_class=0 全体达标，用真派发链覆盖 periodic SQL
NOW = q("SELECT to_char(now(), 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')")
s, r = call("POST", "/admin/tasks", {
    "name": "AO周期%d" % TS, "kind": "periodic", "auto_assign": True,
    "period": "monthly", "target_class": 0, "duration_days": 30,
    "reward": 100, "metric": {"seed_seconds_delta": 864000},
    "starts_at": "2026-10-01T00:00:00Z",
    "ends_at": "2036-10-01T00:00:00Z"}, token=ROOT)
P1 = (r.get("data") or {}).get("id")
ok("S0 周期探针任务创建（periodic/auto/mc0）", s == 200 and P1, str(r)[:90])
if P1:
    CLEAN.append("DELETE FROM tasks WHERE id = %d" % P1)
CLEAN.append("DELETE FROM task_claims WHERE task_id IN "
             "(SELECT id FROM tasks WHERE kind IN %s)" % TASK_KINDS)
CLEAN.append("DELETE FROM messages WHERE subject IN "
             "('考核已派发','转正考核通过')")
T = probe_torrent(uA, "t")

print("== S1 自动派发：正控 + P-39 过滤哨兵 ==")
jok, jinfo = run_job("exam_assign")
ok("S1 exam_assign 真跑（模块开）", jok is True, jinfo)
ca = claim_of(uA)
ok("S1 正控：窗口内正常成员拿到认领", ca and ca[1] == 0, str(ca))
if ca:
    cidA = ca[0]
    ok("S1 基线快照=派发时点（base_seed_seconds=0）",
       q("SELECT base_seed_seconds FROM task_claims WHERE id = %d"
         % cidA) == "0")
    # PM 断言按「认领 1:1」口径：S1 的 run_job 会跑两次派发 tick（先
    # onboard-only、建周期任务后再 onboard+periodic），uA 合法持有两类
    # 考核各 1 条 ⇒ 固定「x1」不成立；「无 claim 未发信」才是本体的判据。
    PM = q("SELECT count(*) FROM messages WHERE receiver_id = %d AND "
           "subject = '考核已派发'" % uA)
    CK = q("SELECT count(*) FROM task_claims c JOIN tasks t ON "
           "t.id = c.task_id WHERE c.user_id = %d AND t.kind IN %s"
           % (uA, TASK_KINDS))
    ok("S1 派发即 PM（每考核认领恰 1 封，不多发）",
       PM == CK and int(CK) >= 1, "pm=%s claims=%s" % (PM, CK))
    ok("S1 pm_sent 置位（防重跑轰炸：考核认领无未发信行）",
       q("SELECT count(*) FROM task_claims c JOIN tasks t ON "
         "t.id = c.task_id WHERE c.user_id = %d AND t.kind IN %s AND "
         "c.pm_sent = FALSE" % (uA, TASK_KINDS)) == "0")
ob = q("SELECT count(*) FROM task_claims c JOIN tasks t ON "
        "t.id=c.task_id WHERE c.user_id = %d AND t.kind IN %s AND "
        "c.claimed_at > '%s'" % (uB, TASK_KINDS, BAN_T))
ok("S1 哨兵：封号成员不得被派发（P-39 修复点）",
   ob == "0", "n=%s" % ob)
NB2 = q("SELECT count(*) FROM task_claims c JOIN tasks t ON "
        "t.id=c.task_id JOIN users u ON u.id=c.user_id WHERE t.kind IN "
        "%s AND u.status >= 2 AND c.claimed_at > '%s'" % (TASK_KINDS, BAN_T))
ok("S1 哨兵：封号/软删/退役账号整体 0 派发（P-39 修复点）",
   NB2 == "0", "n=%s" % NB2)
POS = q("SELECT count(*) FROM task_claims c JOIN tasks t ON "
        "t.id=c.task_id JOIN users u ON u.id=c.user_id WHERE t.kind IN "
        "%s AND u.status < 2" % TASK_KINDS)
ok("S1 正控：在册账号确有派发（哨兵不是空集假绿）",
   int(POS) > 0, "n=%s" % POS)
PP2 = q("SELECT count(*) FROM task_claims WHERE task_id = %s AND user_id "
        "IN (SELECT id FROM users WHERE status >= 2)" % P1)
ok("S1 哨兵：periodic 分支同样不派给封号号（同族 SQL）",
   PP2 == "0", "n=%s" % PP2)
PP1 = q("SELECT count(*) FROM task_claims WHERE task_id = %s AND user_id "
        "IN (SELECT id FROM users WHERE status < 2)" % P1)
ok("S1 正控：periodic 分支确有派发", int(PP1) > 0, "n=%s" % PP1)

s, r = call("GET", "/me/exams", token=tA)
rows = r.get("data") or []
row = next((x for x in rows if x.get("task_id") == (
    int(q("SELECT task_id FROM task_claims WHERE id = %d" % cidA))
    if ca else -1)), None) if ca else None
ok("S1 /me/exams 读口（成员视角可见认领）", s == 200 and row, str(r)[:90])
if row:
    reward_a = int(q("SELECT reward FROM tasks WHERE id = %d"
                     % row["task_id"]))
    ok("S1 /me/exams 字段：status=0/进度 0/奖励与库一致",
       row.get("status") == 0 and row.get("current", {}).get(
           "seed_seconds") == 0 and row.get("reward") == reward_a
       and row.get("deadline"), str(row)[:120])

print("== S2 达标结算：发奖 + 转正信 + 幂等 ==")
snatch_touch(uA, T, 432000)
s, r = call("GET", "/me/exams", token=tA)
row = next((x for x in (r.get("data") or [])
            if x.get("task_id") == row["task_id"]), None) if row else None
ok("S2 进度读口跟做种增量（432000/432000）",
   row and row.get("current", {}).get("seed_seconds") == 432000,
   str(row.get("current") if row else None))
BAL_A0 = q("SELECT spark_balance::text FROM users WHERE id = %d" % uA)
jok, jinfo = run_job("task_settle")
ok("S2 task_settle 真跑", jok is True, jinfo)
ok("S2 达标转 status=1（轮询跨批次）", wait_claim(cidA, "1"))
ok("S2 reward_paid=任务奖励",
   q("SELECT reward_paid::text FROM task_claims WHERE id = %d"
     % cidA) == q("SELECT reward::text FROM tasks WHERE id = "
                  "(SELECT task_id FROM task_claims WHERE id = %d)"
                  % cidA))
ok("S2 流水落地（task_settle 幂等键 x1）",
   ledger_sum(cidA) == q("SELECT reward::text FROM tasks WHERE id = "
                         "(SELECT task_id FROM task_claims WHERE id = %d)"
                         % cidA))
BAL_A1 = q("SELECT spark_balance::text FROM users WHERE id = %d" % uA)
ok("S2 余额增量=奖励额",
   int(BAL_A1) - int(BAL_A0) == int(ledger_sum(cidA)),
   "before=%s after=%s" % (BAL_A0, BAL_A1))
ok("S2 转正站内信（onboard 专有文案）",
   q("SELECT count(*) FROM messages WHERE receiver_id = %d AND "
     "subject = '转正考核通过'" % uA) == "1")
run_job("task_settle")
time.sleep(6)
ok("S2 重跑不双发（流水仍 x1、余额不动）",
   q("SELECT count(*) FROM spark_ledger WHERE idempotency_key = "
     "'task_settle:%d'" % cidA) == "1" and
   q("SELECT spark_balance::text FROM users WHERE id = %d" % uA) == BAL_A1)

print("== S3 豁免/恢复（admin 0105 面） ==")
cc = claim_of(uC)
ok("S3 正控：豁免对象先有在途认领", cc and cc[1] == 0, str(cc))
if cc:
    cidC = cc[0]
    # 双哨兵（2026-10-11 闸门自查）：claim_of 必须命中 onboard 一条。
    # 若取成 periodic 探针（id 更大），其 864000 门槛永远追不上 ⇒ 恢复后
    # wait_claim 超时假红；两条并断言防回归。
    ok("S3 认领对象是 onboard（非 periodic 探针）",
       q("SELECT kind FROM tasks WHERE id = (SELECT task_id FROM "
         "task_claims WHERE id = %d)" % cidC) == "onboard")
    s, r = call("POST", "/admin/exam-users/%d/exempt" % cidC, None,
                token=ROOT)
    ok("S3 exempt 受理", s == 200, "http=%s %s" % (s, str(r)[:80]))
    ok("S3 exempted_at 落库",
       q("SELECT exempted_at IS NOT NULL FROM task_claims WHERE id = %d"
         % cidC) == "t")
    # 豁免先落地再造进度：反过来会被 60s tick 抢跑结算（竞态）
    snatch_touch(uC, T, 432000)   # 达标在身：不豁免就必结算
    run_job("task_settle")
    time.sleep(6)
    ok("S3 豁免期间跳过结算（status=0、无流水）",
       q("SELECT status FROM task_claims WHERE id = %d" % cidC) == "0"
       and ledger_sum(cidC) == "0")
    s, r = call("POST", "/admin/exam-users/%d/recover" % cidC, None,
                token=ROOT)
    ok("S3 recover 受理", s == 200, "http=%s %s" % (s, str(r)[:80]))
    jok, jinfo = run_job("task_settle")
    ok("S3 恢复后回到结算流并达标", wait_claim(cidC, "1"), jinfo)
    ok("S3 恢复后发奖落流水",
       ledger_sum(cidC) == q("SELECT reward::text FROM tasks WHERE id = "
                             "(SELECT task_id FROM task_claims WHERE id "
                             "= %d)" % cidC))
    s, r = call("POST", "/admin/exam-users/%d/exempt" % cidA, None,
                token=ROOT)
    ok("S3 反例：已结算认领不可再豁免", s in (400, 404),
       "http=%s %s" % (s, str(r)[:80]))

print("== S3b 模块负控：关着不派新、开着才派 ==")
setting("module_exams", "no")
uD, tD = mkuser(N_D)
jok, jinfo = run_job("exam_assign")
ok("S3b 模块关：job 记拦截失败", jok is False and
   ("未执行" in jinfo or "模块" in jinfo), jinfo)
ok("S3b 模块关：新人不被派发（负控）", claim_of(uD) is None,
   str(claim_of(uD)))
setting("module_exams", "yes")
jok, jinfo = run_job("exam_assign")
ok("S3b 模块开：同一个人立刻被派发（正控，排除窗口因素）",
   jok is True and claim_of(uD) is not None, "%s %s" % (jinfo, claim_of(uD)))
NB3 = q("SELECT count(*) FROM task_claims c JOIN tasks t ON "
        "t.id=c.task_id JOIN users u ON u.id=c.user_id WHERE t.kind IN "
        "%s AND u.status >= 2 AND c.claimed_at > '%s'" % (TASK_KINDS, BAN_T))
ok("S3b 第二轮派发后哨兵仍 0（修复面持续成立）", NB3 == "0", "n=%s" % NB3)

print("== S4 收尾：模块还原 + 探针清零 ==")
if MOD0 == "<absent>":
    CLEAN.append("DELETE FROM site_settings WHERE name = 'module_exams'")
else:
    CLEAN.append("UPDATE site_settings SET value = '%s' "
                 "WHERE name = 'module_exams'" % MOD0)
# 最后 append ⇒ 最先执行：避免清完 claims 后、还原前被 60s tick 重派
CLEAN_FAILS = cleanup()
ok("S4a 收尾语句全部成功（失败即红：残料会连累别的闸门）",
   not CLEAN_FAILS, "失败=%s" % (CLEAN_FAILS,))
ok("S4b 本闸门探针种无残留（AoExam.% ）",
   q("SELECT count(*) FROM torrents WHERE name LIKE 'AoExam.%%'") == "0",
   "残留=%s" % q("SELECT count(*) FROM torrents WHERE name LIKE 'AoExam.%%'"))
ok("S4 module_exams 还原为 %s" % MOD0,
   q("SELECT COALESCE((SELECT value FROM site_settings WHERE name = "
     "'module_exams'),'<absent>')") == MOD0)
Z = q("SELECT (SELECT count(*) FROM task_claims c JOIN tasks t ON "
      "t.id=c.task_id WHERE t.kind IN %s)::text" % TASK_KINDS)
ok("S4 考核认领清零", Z == "0", Z)
M = q("SELECT count(*) FROM messages WHERE subject IN "
      "('考核已派发','转正考核通过')")
ok("S4 考核私信清零", M == "0", M)
L = q("SELECT count(*) FROM spark_ledger WHERE user_id IN (%d,%d) AND "
      "(idempotency_key LIKE 'task_settle:%%' OR idempotency_key LIKE "
      "'task_penalty:%%')" % (uA, uC))
ok("S4 探针流水清零", L == "0", L)
U = q("SELECT count(*) FROM users WHERE id IN (%d,%d,%d) AND "
      "status = 2" % (uA, uB, uC))
ok("S4 探针账号已停用（3 个）", U == "3", U)

summary()
