# -*- coding: utf-8 -*-
"""闸门 BQ：worker 的三条「管理组告警」写法（staffmessages 收件人解析）。

现场读数（未修版，2026-10-13）：
  1. PG 日志 11:52 连着三条
     `ERROR: null value in column "user_id" of relation "staffmessages"
      violates not-null constraint`，STATEMENT 就是
     `apps/worker/src/jobs/cheat_enforce.rs:127` 那条
     `INSERT … SELECT MIN(id) … WHERE class_id>=90 AND NOT EXISTS(7 天去重)`；
  2. 库层定点复现（事务 + SAVEPOINT 回滚，残料 0）：
     - 先去重命中（已有一条同类告警）再跑旧写法 → 同一个 null value 报错；
     - 把收件人条件换成 `class_id >= 9000`（= 没有在册高层）→ 同一个报错。

根因是同一个形状：**把聚合当 EXISTS 用**。`SELECT MIN(id) … WHERE <过滤>`
没有 GROUP BY，过滤命中 0 行时它**仍返回一行**，值为 NULL ⇒ INSERT 真的执行，
撞 `user_id` 的 NOT NULL 约束。三处同形：
  - `jobs/cheat_enforce.rs:127`（L2 累进告警，带 7 天去重 → 去重那一支
    从此**靠数据库报错收场**，每个 tick 刷一次错误日志，而
    `.unwrap_or(0)` 把错误吞成 0）；
  - `jobs/audit.rs:109`、`jobs/sweep.rs:121`（无去重条件，但一旦站点没有
    class≥90 的行就必然失败，且 `let _: Result<_, _> =` 同样吞错）。
⇒ 表现是「告警没发出去」，但没人看得见失败：既不报错给运维，也不记账。

本闸门四条腿：① 静态形状判据（worker 源码里不许再把聚合直接当 INSERT 的
值源；四处自动告警都要走同一个 staff_alert 出口；出口件之外不许有第二处
写 staffmessages）；② 真跑一次 `cheat_enforce` job：第一轮必须**真发出一封**
（不是"发出去了但没人收到"），第二轮在 7 天去重窗口内必须 **0 新增且 PG
日志不出现新错误**；③ 把已有告警推到 8 天前，窗口到期后必须**再发一封**
（去重是窗口不是永久静默）；④ 来信人语义（R6）：本轮新增告警不许署成员的名。

第 ④ 条的由来（P-199）：`announce.rs` 与 `audit.rs` 的 Ratio Watch 告警把
**被处置用户**绑成 `staffmessages.user_id`，而 `staff_answer` 答复时正是把
原文 PM 给这个列 ⇒ 职员一回单，处置意图就送到当事人手里。现网存量读数：
staffmessages id=1（2026-10-05）主题「客户端黑白名单自动告警」，
user_id=69 = `e2ecap1397`（class 1 普通成员，answered=0）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bq_staff_alert.py
"""
import io
import os
import re
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (dig,   # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
AGENT = "ghost:bq%d" % TAG
PREFIX = "[[cheat:%s]]" % AGENT
PROBE = "bq%d" % TAG
UID = [0]
REPO = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..",
                    "apps", "worker", "src", "jobs")
ERR = 'null value in column "user_id" of relation "staffmessages"'


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:56], (r.stderr or "").strip()[:90]))
    return (r.stdout or "").strip()


def alerts():
    return psql("SELECT count(*) FROM staffmessages WHERE body LIKE '%s%%'"
                % PREFIX)


def pg_errors(since):
    r = subprocess.run(
        ["docker", "logs", "--since", since, "flux-postgres"],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        universal_newlines=True, encoding="utf-8")
    txt = (r.stdout or "") + (r.stderr or "")
    return txt.count(ERR)


def finalize():
    psql("DELETE FROM staffmessages WHERE body LIKE '%s%%'" % PREFIX)
    psql("DELETE FROM messages WHERE body LIKE '%s%%'" % PREFIX)
    psql("DELETE FROM cheat_events WHERE agent = '%s'" % AGENT)
    for u in UID:
        if u and psql("SELECT status::text FROM users WHERE id = %d" % u) \
                != "2":
            call("POST", "/admin/users/status", {"user_id": u, "status": 2},
                 token=ROOT)
            call("DELETE", "/admin/users/%d" % u, None, token=ROOT)


import atexit  # noqa: E402

atexit.register(finalize)


def mkmember():
    import json  # noqa: F401
    codes = dig(call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                  token=ROOT)[1], "data", "codes") or []
    cap = (call("GET", "/auth/captcha", token=ROOT)[1] or {}).get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if not codes or len(nums) < 2:
        return 0
    name = "bqm%d" % TAG
    call("POST", "/auth/register", {
        "username": name, "email": "%s@bq.local" % name,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}}, token=ROOT)
    return int(psql("SELECT id FROM users WHERE username = '%s'" % name) or 0)


def run_job(job):
    """触发一次并等收口：钉 job_status.last_started_at 前移（60s tick 会撞车）。"""
    at0 = psql("SELECT to_char(last_started_at,'YYYY-MM-DD HH24:MI:SS.US') "
               "FROM job_status WHERE job = '%s'" % job) or ""
    s, b = call("POST", "/admin/jobs/run", {"job": job}, token=ROOT)
    if s != 200:
        return s, "触发失败", at0
    for _ in range(24):
        time.sleep(5)
        at1 = psql("SELECT to_char(last_started_at,'YYYY-MM-DD HH24:MI:SS.US')"
                   " FROM job_status WHERE job = '%s'" % job) or ""
        if at1 and at1 != at0:
            return 200, at1, at0
    return 200, "超时未看到 last_started_at 前移", at0


clear_register_rl()
BASE_ALERTS = int(psql("SELECT count(*) FROM staffmessages") or 0)
L2 = psql("SELECT coalesce((SELECT value FROM site_settings WHERE name = "
          "'cheat_l2_hits'), '5(缺省)')")
ok("R0a 基线：staffmessages %s 行、cheat_l2_hits=%s" % (BASE_ALERTS, L2),
   BASE_ALERTS >= 0, "基线=%s" % BASE_ALERTS)

# ===== R1 静态形状：不许把聚合当 EXISTS 用 =====
# 判的是「INSERT INTO staffmessages (列) SELECT MIN(id) …」这一形状：聚合
# 直接当值源，过滤命中 0 行时仍返回一行 NULL。修后的写法把 MIN 关进派生表，
# 由 WHERE t.uid IS NOT NULL 兜住（R1c 单独钉这条守卫），不算命中。
hits_shape, hits_call = [], []
SHAPE = re.compile(r"INSERT INTO staffmessages[\s\\\n]*\([^)]*\)"
                   r"[\s\\\n]*SELECT\s+MIN\(id\)", re.I)
for fn in sorted(os.listdir(REPO)):
    if not fn.endswith(".rs"):
        continue
    src = io.open(os.path.join(REPO, fn), encoding="utf-8").read()
    if SHAPE.search(src):
        hits_shape.append(fn)
    hits_call.append((fn, src.count("staff_alert(")))
dup = sorted(set(hits_shape))
called = sorted({f for f, n in hits_call if n > 0})
ok("R1a worker 源码里「INSERT INTO staffmessages (列) SELECT MIN(id) …」"
   "这种形状必须为 0 处（命中=%s）" % dup, not dup, "命中文件=%s" % dup)
NEED = ("cheat_enforce.rs", "audit.rs", "sweep.rs", "announce.rs")
miss = [f for f in NEED if f not in called]
ok("R1b 四处自动告警都要走同一个 staff_alert 出口（缺 %s）" % miss,
   not miss, "调用点=%s" % called)
second = [f for f in os.listdir(REPO)
          if f.endswith(".rs") and f != "staff_alert.rs"
          and "INSERT INTO staffmessages" in io.open(
              os.path.join(REPO, f), encoding="utf-8").read()]
ok("R1d worker 里写 staffmessages 的地方只许有出口件一处（第二处=%s）" % second,
   not second, "第二处=%s" % second)
EXIT_P = os.path.join(REPO, "staff_alert.rs")
exit_src = (io.open(EXIT_P, encoding="utf-8").read()
            if os.path.exists(EXIT_P) else "")
ok("R1c 出口件要同时有「没有署名就不插入」的守卫（IS NOT NULL）与在册口径"
   "（status < 2），否则只是把报错挪了个地方",
   "IS NOT NULL" in exit_src and "status < 2" in exit_src,
   "守卫=%s 在册=%s" % ("IS NOT NULL" in exit_src,
                          "status < 2" in exit_src))

# ===== R2 真跑：第一轮必须真发出去 =====
UID0 = mkmember()
UID.append(UID0)
if UID0:
    psql("INSERT INTO cheat_events (user_id, agent, reason, hits, first_seen,"
         " last_seen) VALUES (%d, '%s', 'BQ probe corpus', 99, now(), now())"
         % (UID0, AGENT))
ok("R2a 探针成员与 cheat_events 语料就绪（uid=%s agent=%s）" % (UID0, AGENT),
   UID0 > 0 and psql("SELECT count(*) FROM cheat_events WHERE agent = '%s'"
                     % AGENT) == "1", "uid=%s" % UID0)
st, at1, at0 = run_job("cheat_enforce")
a1 = int(alerts() or 0)
ok("R3a 第一轮 job 之后必须真发出 1 封管理组告警（发不出去就是静默丢失）"
   " [job=%s tick %s→%s]" % (st, at0[-12:], str(at1)[-12:]),
   a1 == 1, "本轮告警=%s 封" % a1)
uid_in = psql("SELECT coalesce(max(user_id)::text,'-') FROM staffmessages "
              "WHERE body LIKE '%s%%'" % PREFIX)
ok("R3b 告警的收件人字段必须是非空真账号（旧写法命中 NULL 时整条 INSERT 是"
   "报错的，压根不会有行）", uid_in not in ("-", "0", ""),
   "user_id=%s" % uid_in)

# ===== R4 第二轮：去重必须静默 no-op，不靠数据库报错 =====
st2, at1b, at0b = run_job("cheat_enforce")
a2 = int(alerts() or 0)
errs = pg_errors("3m")
ok("R4a 7 天内同一 (user,agent) 不重复告警（本轮仍=%s 封）" % a2,
   a2 == 1, "告警=%s 封" % a2)
ok("R4b 去重必须是**静默 no-op**：PG 日志本轮不许出现 "
   "null value in column user_id of relation staffmessages"
   "（修前每命中一次去重就报一次错，且被 unwrap_or(0) 吞掉）",
   errs == 0, "近 3 分钟该错误出现 %d 次" % errs)
dup_groups = psql(
    "SELECT count(*) FROM (SELECT left(body, 40) FROM staffmessages "
    "WHERE subject = '作弊事件累进告警' "
    "AND created_at > now() - interval '20 min' "
    "GROUP BY 1 HAVING count(*) > 1) x")
ok("R4c 防洪：近 20 分钟同一个 (user,agent) 前缀不许出现多条 L2 告警"
   "（去重一旦失效就每 tick 重发一封——出口件首版的 i64 解码错误正是这样"
   "被读成「没告警过」的）", dup_groups == "0",
   "重复前缀组=%s 组" % dup_groups)

# ===== R5 去重窗到期后必须还能再告（正向判据，不止「不报错」）=====
psql("UPDATE staffmessages SET created_at = now() - interval '8 days' "
     "WHERE body LIKE '%s%%'" % PREFIX)
st3, at1c, at0c = run_job("cheat_enforce")
a3 = int(alerts() or 0)
ok("R5a 把已有告警推到 8 天前（出 7 天窗）后，同一 (user,agent) 必须再发一封"
   "（去重是窗口不是永久静默） [job=%s tick %s→%s]"
   % (st3, at0c[-12:], str(at1c)[-12:]),
   a3 == 2, "本轮告警=%s 封（预想 2）" % a3)
l1 = psql("SELECT count(*) FROM messages WHERE body LIKE '%s%%'" % PREFIX)
ok("R5b 同轮 L1 用户提醒写在 messages（口径与 L2 分表，本轮=%s 封）" % l1,
   l1 in ("0", "1"), "L1=%s" % l1)
errs_all = pg_errors("8m")
ok("R5c 整轮（含窗口到期那一轮）PG 日志该错误累计必须为 0",
   errs_all == 0, "近 8 分钟 %d 次" % errs_all)

# ===== R6 来信人语义：自动告警不许署「被处置用户」的名 =====
# 修复前 announce.rs 与 audit.rs 的 Ratio Watch 都把当事人绑成 user_id，
# 而 staff_answer 会答复给 user_id ⇒ 处置意图原话 PM 给被处置用户。
# 这里断言「本轮新增的告警里没有成员署名」，历史存量只数不删（数据修复待拍板）。
signed_member = psql(
    "SELECT count(*) FROM staffmessages sm JOIN users u ON u.id = sm.user_id "
    "WHERE u.class_id < 90")
fresh_member = psql(
    "SELECT count(*) FROM staffmessages sm JOIN users u ON u.id = sm.user_id "
    "WHERE u.class_id < 90 AND sm.created_at > now() - interval '15 min'")
ok("R6a 本轮新增自动告警的成员署名行数必须 0（历史存量=%s 行属数据修复，"
   "不在本判据内）" % signed_member, fresh_member == "0",
   "本轮=%s 行 历史=%s 行" % (fresh_member, signed_member))
# ===== R9 收尾 =====
finalize()
now = int(psql("SELECT count(*) FROM staffmessages") or 0)
probe_left = psql("SELECT count(*) FROM staffmessages WHERE body LIKE "
                  "'%s%%'" % PREFIX)
ok("R9a 探针告警与语料已清（探针前缀剩 %s 行、cheat_events 探针 0 行）；"
   "总数 %s→%s 的增量是修复后**本该存在**的真实告警"
   % (probe_left, BASE_ALERTS, now),
   probe_left == "0" and psql("SELECT count(*) FROM cheat_events WHERE "
                              "agent = '%s'" % AGENT) == "0",
   "探针剩=%s 总行=%s→%s" % (probe_left, BASE_ALERTS, now))
ok("R9b 探针成员已墓碑、本轮不留活跃账号",
   psql("SELECT count(*) FROM users WHERE status < 2 AND username LIKE "
        "'bqm%%'") == "0",
   "live=%s" % psql("SELECT count(*) FROM users WHERE status < 2 AND "
                    "username LIKE 'bqm%%'"))
summary()
