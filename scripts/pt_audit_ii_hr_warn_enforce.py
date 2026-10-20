# -*- coding: utf-8 -*-
"""II 闸门：H&R 警告阶梯的「禁传 / 禁邀」必须真的执法（P-444）。

0388 把 block_upload / block_invite 写进 hr_warn_states，domain 的
LADDER 第 3/4 档也声明了「禁发新种 / 禁发邀请」，但全仓没有读取点：
worker 只落库，发种口与邀请口从不看这两列——三四档此前是摆设。
本闸门用一行 fixture 逐个出口取证（不依赖 worker tick）：
  II1 发种口：block_upload → 400「禁传档」，且未拦时基线不是这句
  II2 生成邀请码：block_invite → 400「禁邀档」，邀请行数与配额均未动
  II3 魔力兑换：拦在 spend_spark 之前（余额/流水/幂等键零变化）
  II4 总开关关闭：同一行 fixture 不再拦人（「回到旧单档」为真）
  II5 清理：fixture / 邀请 / 配额 / 探针号全部归零
"""
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, call, login, ok, summary  # noqa: E402

ROOT = login()
RUN = int(time.time()) % 100000
PW = "Gate#123456"
UNAME = "iim%d" % RUN


def q(sql):
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8")
    return p.stdout.strip()


def x(sql):
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-c", sql],
        capture_output=True, text=True, encoding="utf-8")
    if p.returncode:
        raise RuntimeError("psql 失败: %s" % (p.stderr or "")[:200])


def msg(resp):
    return json.dumps(resp, ensure_ascii=False)


def upload_empty(tok):
    req = urllib.request.Request(BASE + "/torrents", method="POST",
                                 data=b"")
    req.add_header("Content-Type",
                   "multipart/form-data; boundary=IIBOUNDARY")
    req.add_header("Authorization", "Bearer " + tok)
    try:
        r = urllib.request.urlopen(req, timeout=30)
        return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        payload = e.read()
        try:
            return e.code, json.loads(payload)
        except Exception:
            return e.code, {
                "raw": payload[:200].decode("utf-8", "replace")}


def inv_rows(uid):
    return int(q("SELECT count(*) FROM invites WHERE inviter_id = %d"
                 % uid))


def quota_used(uid):
    return int(q("SELECT COALESCE((SELECT used FROM invite_quota "
                 "WHERE user_id = %d AND period = date_trunc('week', "
                 "now())::date), 0)" % uid))


def run_job(job, wait_s=170):
    """等 worker 真跑完一轮该任务（last_finished_at 前移才算）。

    只看 last_started_at 会在 SQL 还没跑完时就返回——AH 的 G5 曾因此假红。
    """
    t0 = q("SELECT to_char(now(), 'YYYY-MM-DD\"T\"HH24:MI:SS.US')")
    call("POST", "/admin/jobs/run", {"job": job}, token=ROOT)
    deadline = time.time() + wait_s
    while time.time() < deadline:
        row = q("SELECT COALESCE(to_char(last_finished_at, "
                "'YYYY-MM-DD\"T\"HH24:MI:SS.US'),'-')||'|'||"
                "COALESCE(last_ok::text,'-') FROM job_status "
                "WHERE job = '%s'" % job)
        if "|" in row:
            fin, _okv = row.split("|", 1)
            if fin not in ("-", "") and fin > t0:
                return fin
        time.sleep(3)
    return None


print("== II0 探针成员与站点设定 ==")
ORIG_EN = q("SELECT value FROM site_settings WHERE name = "
            "'hr_warn_enable'")
call("POST", "/admin/adduser",
     {"username": UNAME, "password": PW,
      "email": "%s@ii.local" % UNAME}, token=ROOT)
UID = int(q("SELECT id::text FROM users WHERE username = '%s'" % UNAME))
# 档位要在取 token 之前改：邀请配额按 class>=3 判
x("UPDATE users SET class_id = 3 WHERE id = %d" % UID)
t0 = login(UNAME, PW)
call("POST", "/me/password/change",
     {"old_password": PW, "new_password": PW + "x"}, token=t0)
TOK = login(UNAME, PW + "x")
ok("II0 探针成员就绪（class 3，无警告行）",
   q("SELECT count(*) FROM hr_warn_states WHERE user_id = %d" % UID)
   == "0", "uid=%s" % UID)

print("== II1 发种口 ==")
bs, br = upload_empty(TOK)
ok("II1a 基线：无警告行时不是「禁传档」拦截（前置正控）",
   not (bs == 400 and "禁传档" in msg(br)),
   "baseline http=%s %s" % (bs, msg(br)[:120]))
x("INSERT INTO hr_warn_states (user_id, step, base_days, expires_at, "
  "restrict_download, block_upload, block_invite) VALUES "
  "(%d, 3, 7, now() + interval '1 day', true, true, true)" % UID)
s, r = upload_empty(TOK)
ok("II1b block_upload=true 时发种口必须 400 且说明「禁传档」",
   s == 400 and "禁传档" in msg(r),
   "http=%s %s" % (s, msg(r)[:160]))

print("== II2 生成邀请码 ==")
s, r = call("POST", "/invites", {}, TOK)
ok("II2a 基线：同一探针（无拦截）能生成邀请码（前置正控）",
   s == 200 and r.get("code") == 0, "http=%s %s" % (s, msg(r)[:120]))
n0, used0 = inv_rows(UID), quota_used(UID)
s, r = call("POST", "/invites", {}, TOK)
ok("II2b block_invite=true 时生成邀请码必须 400「禁邀档」",
   s == 400 and "禁邀档" in msg(r),
   "http=%s %s" % (s, msg(r)[:160]))
ok("II2c 被拒时没有多发邀请行、也没有吃掉配额",
   inv_rows(UID) == n0 and quota_used(UID) == used0,
   "inv=%s->%s quota=%s->%s" % (n0, inv_rows(UID), used0,
                                quota_used(UID)))

print("== II3 魔力兑换（拦在扣钱之前）==")
PRICE = int(q("SELECT price FROM shop_items WHERE kind = 'invite' "
              "AND active ORDER BY price LIMIT 1"))
x("UPDATE users SET spark_balance = %d WHERE id = %d"
  % (PRICE + 100000, UID))
BAL = q("SELECT spark_balance::text FROM users WHERE id = %d" % UID)
KEY = "ii-%d" % RUN
LED = "invite_redeem:%d:%s" % (UID, KEY)
s, r = call("POST", "/invites/redeem", {"idempotency_key": KEY}, TOK)
ok("II3a block_invite=true 时魔力兑换必须 400「禁邀档」",
   s == 400 and "禁邀档" in msg(r),
   "http=%s %s" % (s, msg(r)[:160]))
ok("II3b 被拒不得扣火花（余额与流水零变化）",
   q("SELECT spark_balance::text FROM users WHERE id = %d" % UID) == BAL
   and q("SELECT count(*) FROM spark_ledger WHERE idempotency_key = '%s'"
         % LED) == "0",
   "bal=%s->%s led=%s" % (BAL,
                          q("SELECT spark_balance::text FROM users "
                            "WHERE id = %d" % UID),
                          q("SELECT count(*) FROM spark_ledger "
                            "WHERE idempotency_key = '%s'" % LED)))

print("== II4 总开关 ==")
# 前面 II2/II3 会因旧实现把周配额吃满（正是被测缺陷），这里先清零，
# 否则「关掉开关应放行」会红在配额耗尽上，红错理由。
x("DELETE FROM invite_quota WHERE user_id = %d" % UID)
x("UPDATE site_settings SET value = '0' WHERE name = 'hr_warn_enable'")
n1 = inv_rows(UID)
s, r = call("POST", "/invites", {}, TOK)
ok("II4 关掉 hr_warn_enable 后同一行 fixture 不再拦人"
   "（回到旧单档行为）",
   s == 200 and inv_rows(UID) == n1 + 1,
   "http=%s %s" % (s, msg(r)[:120]))
x("UPDATE site_settings SET value = '1' WHERE name = 'hr_warn_enable'")
s, r = call("POST", "/invites", {}, TOK)
ok("II4b 重新打开后拦截恢复（翻转正控）",
   s == 400 and "禁邀档" in msg(r),
   "http=%s %s" % (s, msg(r)[:120]))

print("== II6 限下锁不被「低于阈值」提前解锁 ==")
# fixture：无未解决违规（阶梯不会重算这一行），到期在未来（restore_due
# 不清），此时唯一会碰这把锁的就是 hr_punish 的 restore_hr_locks。
x("UPDATE users SET download_enabled = FALSE, "
  "download_locked_by = 'hr' WHERE id = %d" % UID)
x("INSERT INTO hr_warn_states (user_id, step, base_days, expires_at, "
  "restrict_download, block_upload, block_invite) VALUES "
  "(%d, 2, 7, now() + interval '1 day', true, false, false) "
  "ON CONFLICT (user_id) DO UPDATE SET step = 2, "
  "expires_at = now() + interval '1 day', restrict_download = true, "
  "block_upload = false, block_invite = false" % UID)
FIN = run_job("hr_punish")
EN = q("SELECT download_enabled::text FROM users WHERE id = %d" % UID)
ok("II6 警告到期前 hr_punish 不得解锁（旧实现 <阈值即放人）",
   FIN is not None and EN == "false",
   "download_enabled=%s finished=%s" % (EN, FIN))

print("== II5 收尾 ==")
if ORIG_EN is None:
    x("DELETE FROM site_settings WHERE name = 'hr_warn_enable'")
else:
    x("UPDATE site_settings SET value = '%s' WHERE name = "
      "'hr_warn_enable'" % ORIG_EN.replace("'", "''"))
x("DELETE FROM hr_warn_states WHERE user_id = %d" % UID)
x("DELETE FROM invites WHERE inviter_id = %d" % UID)
x("DELETE FROM invite_quota WHERE user_id = %d" % UID)
call("POST", "/admin/users/status", {"user_id": UID, "status": 2}, ROOT)
call("DELETE", "/admin/users/%d" % UID, None, token=ROOT)
x("DELETE FROM spark_ledger WHERE user_id = %d" % UID)
x("DELETE FROM traffic_ledger WHERE user_id = %d" % UID)
ok("II5a 探针成员已走产品墓碑",
   q("SELECT count(*) FROM users WHERE id = %d AND status >= 2 "
     "AND passkey LIKE 'deleted-%%'" % UID) == "1", "uid=%s" % UID)
ok("II5b 警告行/邀请/配额/火花流水无残留",
   q("SELECT (SELECT count(*) FROM hr_warn_states WHERE user_id = %d) + "
     "(SELECT count(*) FROM invites WHERE inviter_id = %d) + "
     "(SELECT count(*) FROM invite_quota WHERE user_id = %d) + "
     "(SELECT count(*) FROM spark_ledger WHERE user_id = %d)"
     % (UID, UID, UID, UID)) == "0", "residue")
ok("II5c hr_warn_enable 已复原",
   q("SELECT value FROM site_settings WHERE name = 'hr_warn_enable'")
   == (ORIG_EN or ""), "now=%s" % q(
       "SELECT value FROM site_settings WHERE name = 'hr_warn_enable'"))
summary()
