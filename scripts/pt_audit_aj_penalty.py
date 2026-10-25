# -*- coding: utf-8 -*-
"""批二十七闸门 AJ：考核不达标处置菜单（0397）端到端。

链路：后台建考核任务（fail_penalty 多选菜单）→ 会员认领 → 到期失败 →
worker task_settle 真跑 → 逐项处置落地（禁下载/禁权限位/扣魔力）+ 站内信
写明处置内容 + 重跑不双罚（幂等）。

时间旅行：认领行 claimed_at 前移（duration_days=1，期限已过），不靠等真到期；
其余全走产品自己的接口与 worker 作业（不手改结算结果）。
"""
import os
import re
import sys
import time
from datetime import datetime, timedelta, timezone

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, psql_stdin, summary,
)

ROOT = login()
TS = int(time.time()) % 100000
USER = "ajp%d" % TS
PW = "Gate#123456"
CREATED = []


def q(sql):
    return psql_stdin(sql)


def setting(name, value):
    q("UPDATE site_settings SET value = '%s' WHERE name = '%s'"
      % (value, name))


def member():
    """造一个探针会员（走真注册流：邀请码 + 验证码）。"""
    codes = (call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                   token=ROOT)[1] or {}).get("data", {}).get("codes") or []
    code = codes[0]["code"] if codes else ""
    cap = (call("GET", "/auth/captcha", token=ROOT)[1] or {}).get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    ans = int(nums[0]) + int(nums[1]) if len(nums) >= 2 else 0
    call("POST", "/auth/register", {
         "username": USER, "email": "%s@aj.local" % USER,
         "password": PW, "invite_code": code,
         "captcha_id": cap.get("captcha_id"), "captcha_answer": ans,
         "captcha_token": "", "fields": {}}, token=ROOT)
    s, r = call("POST", "/auth/login",
                {"username": USER, "password": PW})
    tok = (r.get("data") or {}).get("token") if s == 200 else None
    s, r = call("GET", "/me", None, token=tok)
    uid = (r.get("data") or {}).get("id", 0)
    CREATED.append("DELETE FROM invites WHERE used_by = %d" % uid)
    CREATED.append("UPDATE users SET status = 2 WHERE id = %d" % uid)
    return tok, uid


def run_job(job, wait_s=200):
    """触发 worker 作业并等它真跑一轮（判据：job_status.last_started_at 前移）。"""
    t0 = q("SELECT to_char(now(), 'YYYY-MM-DD\"T\"HH24:MI:SS.US')")
    call("POST", "/admin/jobs/run", {"job": job}, token=ROOT)
    deadline = time.time() + wait_s
    while time.time() < deadline:
        row = q("SELECT to_char(last_started_at, "
                "'YYYY-MM-DD\"T\"HH24:MI:SS.US')||'|'||"
                "COALESCE(last_ok::text,'-') FROM job_status "
                "WHERE job = '%s'" % job)
        if "|" in row:
            started, okv = row.split("|", 1)
            if started > t0:
                return okv == "true", "started=%s ok=%s" % (started, okv)
        time.sleep(3)
    return False, "超时未开工"


print("== G0 前置：模块开关 + 探针会员 ==")
clear_register_rl()
orig_mod = q("SELECT COALESCE((SELECT value FROM site_settings "
               "WHERE name = 'module_exams'),'1')")
setting("module_exams", "1")
CREATED.append("__restore_module_exams__")
TOK, UID = member()
ok("G0 探针会员可用", UID > 0 and TOK is not None, "uid=%s" % UID)

print("== G1 建考核任务（处置菜单三项）==")
# starts_at/ends_at 是 DateTime<Utc>：必须带时区偏移（RFC3339），
# 只给本地时刻会被 serde 判成「请求参数格式不正确」（400）。
now = (datetime.now(timezone.utc) - timedelta(hours=1)).strftime(
    "%Y-%m-%dT%H:%M:%SZ")
end = (datetime.now(timezone.utc) + timedelta(days=30)).strftime(
    "%Y-%m-%dT%H:%M:%SZ")
body = {
    "name": "AJ探针考核 %d" % TS,
    "metric": {"seed_seconds_delta": 999999},
    "starts_at": now, "ends_at": end,
    "kind": "periodic", "auto_assign": False, "period": "once",
    "duration_days": 1, "subtitle": "AJ 探针考核", "tier": "",
    "fee": 0, "quota_total": 100, "sort": 0,
    "fail_penalty": [
        {"kind": "restrict_download"},
        {"kind": "ban_pages", "pages": ["torrent.upload"]},
        {"kind": "spark_fine", "amount": 3},
    ],
}
s, r = call("POST", "/admin/tasks", body, token=ROOT)
TID = (r.get("data") or {}).get("id") if s == 200 else None
ok("G1 任务建成（处置菜单落库）", s == 200 and TID is not None,
   "status=%s %s" % (s, str(r)[:120]))
if not TID:
    summary()
CREATED.append("DELETE FROM task_claims WHERE task_id = %d" % TID)
CREATED.append("DELETE FROM exam_penalties WHERE task_id = %d" % TID)
CREATED.append("DELETE FROM tasks WHERE id = %d" % TID)

print("== G2 会员认领 ==")
s, r = call("POST", "/tasks/claim", {"task_id": TID}, token=TOK)
CID = q("SELECT id FROM task_claims WHERE task_id = %d AND user_id = %d "
         "ORDER BY id DESC LIMIT 1" % (TID, UID))
ok("G2 认领落库（status=0）", s == 200 and CID.isdigit(),
   "status=%s claim=%s %s" % (s, CID, str(r)[:110]))
if not CID.isdigit():
    summary()

print("== G3 时间旅行：认领已过期 + worker 真结算 ==")
q("UPDATE task_claims SET claimed_at = now() - interval '3 day' "
  "WHERE id = %d" % int(CID))
jok, jdet = run_job("task_settle")
ok("G3 task_settle 真跑一轮", jok, jdet)
st = q("SELECT status FROM task_claims WHERE id = %d" % int(CID))
ok("G3 认领判失败（status=2）", st == "2", "status=%s" % st)

print("== G4 处置逐项落地 ==")
de = q("SELECT COALESCE(download_enabled::text,'?')||'|'||"
        "COALESCE(download_locked_by,'-') FROM users WHERE id = %d" % UID)
ok("G4 禁下载落地（download_enabled=false + 来源 exam）",
   de == "false|exam", "got=%s" % de)
perm = q("SELECT count(*) FROM user_permissions WHERE user_id = %d "
          "AND permission_key = 'torrent.upload' AND granted = false"
          % UID)
ok("G4 禁发布权限位落地（granted=false）", perm == "1",
   "rows=%s" % perm)
pen = q("SELECT count(*) FROM exam_penalties WHERE claim_id = %d "
         "AND user_id = %d" % (int(CID), UID))
ok("G4 处置落账（exam_penalties 3 行）", pen == "3", "rows=%s" % pen)
kinds = q("SELECT string_agg(kind, ',' ORDER BY id) FROM exam_penalties "
           "WHERE claim_id = %d" % int(CID))
ok("G4 处置项与配置一致",
   kinds == "restrict_download,ban_pages,spark_fine", "got=%s" % kinds)
msg = q("SELECT count(*) FROM messages WHERE receiver_id = %d "
         "AND body LIKE '%%处置%%'" % UID)
ok("G4 站内信写明处置内容（非黑箱）", msg == "1", "rows=%s" % msg)

print("== G5 幂等：重跑结算不重复处置 ==")
before = q("SELECT count(*) FROM exam_penalties WHERE claim_id = %d"
            % int(CID))
run_job("task_settle")
after = q("SELECT count(*) FROM exam_penalties WHERE claim_id = %d"
           % int(CID))
ok("G5 重跑不双罚", before == after == "3",
   "before=%s after=%s" % (before, after))

print("== G6 收尾 ==")
for stmt in reversed(CREATED):
    if stmt == "__restore_module_exams__":
        setting("module_exams", orig_mod)
        continue
    try:
        q(stmt)
    except Exception as e:  # noqa: BLE001
        print("[CLEAN-WARN] %s %s" % (stmt[:60], e))
# 会员走产品自己的删除链（先处置 → 再级联删），不用裸 SQL：
# 裸删会被 invites_used_by / messages_receiver_id 等外键挡住
# （P-201 的老坑），顺带把这条删除链本身也当判据跑一遍。
call("POST", "/admin/users/status",
     {"user_id": UID, "status": 2, "reason": "AJ 闸门探针清理"},
     token=ROOT)
sd, rd = call("DELETE", "/admin/users/%d" % UID, None, token=ROOT)
ok("G6 探针会员已删（产品删除链）", sd == 200,
   "status=%s %s" % (sd, str(rd)[:110]))
ok("G6 探针种无残留",
   q("SELECT count(*) FROM tasks WHERE name LIKE 'AJ探针考核%%'") == "0",
   "")
# 后台「删除」= 墓碑（status=3 + 用户名匿名化），不是物理删行：
# 判据按产品的真实口径写（删行的话审计留痕和申诉凭据会一起没了）。
tomb = q("SELECT status||'|'||username FROM users WHERE id = %d" % UID)
ok("G6 探针会员已墓碑（status=3 + 匿名化）",
   tomb.startswith("3|deleted-"), "got=%s" % tomb)
summary()