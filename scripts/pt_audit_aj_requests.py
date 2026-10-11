# -*- coding: utf-8 -*-
"""批十九闸门 AJ：求种（requests）钱的链路——冻结 / 应种 / 转移 / 通知。

盯的是「悬赏这笔魔力在求种↔应种之间走完全程时会不会掉」：
建单冻结 → 非发布者应种被拒 → 发布者应种转移 + 通知求种人 →
发奖失败必须整体回滚。R3 是这条链上唯一能造出「钱消失」的位置，
用把应种人余额推到 int64 顶的方式让入账必然失败，再断言
「要么两样都改，一样都不改」（旧实现先落 status=1 再发钱，
发钱失败就留下已完结、不可重试、悬赏蒸发三重后果）。

跑前把 `requests` 模块打开，收尾原样还原（模块开关、种子的
owner、探针余额都是共享设定）。
"""
import os
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402

ROOT = login()
BOUNTY = 300
CLEAN = []
TS = int(time.time()) % 100000
A = "ajA%d" % TS          # 求种人
B = "ajB%d" % TS          # 应种人（种子发布者）
C = "ajC%d" % TS          # 路人（试图冒领）
PW = "Gate#123456"
SUBJ = "你求的种已被应求"


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


def uid_of(name):
    return int(q("SELECT id::text FROM users WHERE username = '%s'" % name))


def mkuser(name, balance):
    call("POST", "/admin/adduser", {"username": name, "password": PW,
                                    "email": "%s@aj.local" % name},
         token=ROOT)
    u = uid_of(name)
    tok = login(name, PW)
    # 管理组建的号带 must_reset_password，不改掉临时密码前所有业务端点都
    # 回「账号正在使用临时密码」——探针必须先改密再拿 token。
    call("POST", "/me/password/change", {"old_password": PW,
                                         "new_password": PW + "x"},
         token=tok)
    exec_sql("UPDATE users SET spark_balance = %d WHERE id = %d"
             % (balance, u))
    CLEAN.append("UPDATE users SET status = 2 WHERE id = %d" % u)
    return u, login(name, PW + "x")


print("== R0 前置：模块开关 + 探针角色 + 一颗可应的种 ==")
MOD = q("SELECT COALESCE((SELECT value FROM site_settings "
        "WHERE name = 'module_requests'), '<absent>')")
if MOD != "yes":
    exec_sql("INSERT INTO site_settings (name, value) "
             "VALUES ('module_requests', 'yes') "
             "ON CONFLICT (name) DO UPDATE SET value = 'yes'")
    if MOD == "<absent>":
        CLEAN.append("DELETE FROM site_settings WHERE name = "
                     "'module_requests'")
    else:
        CLEAN.append("UPDATE site_settings SET value = '%s' "
                     "WHERE name = 'module_requests'" % MOD)
ok("R0 requests 模块已开（收尾原样还原）",
   q("SELECT value FROM site_settings WHERE name = 'module_requests'")
   == "yes" or MOD == "yes", "before=%s" % MOD)

ua, ta = mkuser(A, 5000)
ub, tb = mkuser(B, 1000)
uc, tc = mkuser(C, 1000)
TID = int(q("SELECT id::text FROM torrents WHERE approval_status = 1 "
            "ORDER BY id DESC LIMIT 1"))
OWNER0 = q("SELECT COALESCE(owner_id::text, 'NULL') FROM torrents "
            "WHERE id = %d" % TID)
exec_sql("UPDATE torrents SET owner_id = %d WHERE id = %d" % (ub, TID))
CLEAN.append("UPDATE torrents SET owner_id = %s WHERE id = %d"
             % (OWNER0, TID))
ok("R0 探针种齐（求种/应种/路人 + 过审种 %d，原 owner=%s）"
   % (TID, OWNER0), TID > 0 and OWNER0.isdigit(), "tid=%s" % TID)

s, r = call("POST", "/requests", {"title": "aj 求种 %d" % TS,
                                  "descr": "闸门探针", "bounty": BOUNTY},
            token=ta)
rid = (r.get("data") or {}).get("id")
ok("R0 带悬赏建单成功", s == 200 and rid, "http=%s %s" % (s, str(r)[:90]))
ok("R0 悬赏即时冻结（余额 -300 且流水落 req-bounty）",
   q("SELECT spark_balance::text FROM users WHERE id = %d" % ua)
   == str(5000 - BOUNTY)
   and q("SELECT count(*) FROM spark_ledger WHERE user_id = %d AND "
         "idempotency_key = 'req-bounty:%d:%s'" % (ua, ua, rid)) == "1",
   "balance=%s" % q("SELECT spark_balance::text FROM users WHERE id = %d"
                    % ua))
CLEAN.append("DELETE FROM spark_ledger WHERE user_id IN (%d,%d,%d)"
             % (ua, ub, uc))
CLEAN.append("DELETE FROM messages WHERE receiver_id IN (%d,%d,%d)"
             % (ua, ub, uc))
if rid:
    CLEAN.append("DELETE FROM requests WHERE id = %s" % rid)


def fulfill(tok, req_id, torrent_id):
    return call("POST", "/requests/fulfill",
                {"request_id": req_id, "torrent_id": torrent_id},
                token=tok)


print("== R1 路人冒领必须被拒（旧口径任何登录用户可领任意悬赏） ==")
s, r = fulfill(tc, rid, TID)
ok("R1 非发布者应种被拒", s == 400 and r.get("code") != 0,
   "http=%s %s" % (s, str(r)[:90]))
ok("R1 拒绝是真拒绝：单仍待应、路人零入账",
   q("SELECT status::text FROM requests WHERE id = %s" % rid) == "0"
   and q("SELECT count(*) FROM spark_ledger WHERE user_id = %d AND "
         "kind = 'task_reward'" % uc) == "0",
   "status=%s" % q("SELECT status::text FROM requests WHERE id = %s" % rid))

print("== R2 发布者应种：状态 / 到账 / 流水 / 通知 四出口 ==")
s, r = fulfill(tb, rid, TID)
ok("R2 应种成功", s == 200 and r.get("code") == 0,
   "http=%s %s" % (s, str(r)[:90]))
ok("R2 单据完结且关联到种",
   q("SELECT status::text||'/'||"
     "COALESCE(fulfilled_torrent_id::text,'-') FROM requests WHERE id = %s"
     % rid) == "1/%d" % TID,
   q("SELECT status::text||'/'||COALESCE(fulfilled_torrent_id::text,'-') "
     "FROM requests WHERE id = %s" % rid))
ok("R2 悬赏转到应种人账上",
   q("SELECT spark_balance::text FROM users WHERE id = %d" % ub)
   == str(1000 + BOUNTY)
   and q("SELECT count(*) FROM spark_ledger WHERE user_id = %d AND "
         "kind = 'task_reward' AND idempotency_key = 'req-payout:%s'"
         % (ub, rid)) == "1",
   "balance=%s" % q("SELECT spark_balance::text FROM users WHERE id = %d"
                    % ub))
ok("R2 求种人收到应求通知",
   q("SELECT count(*) FROM messages WHERE receiver_id = %d AND "
     "subject = '%s'" % (ua, SUBJ)) == "1",
   q("SELECT COALESCE(string_agg(subject, ','), '无') FROM messages "
     "WHERE receiver_id = %d" % ua))
s, r = fulfill(tb, rid, TID)
ok("R2 重复应求被拒（状态机不许二次发钱）",
   s == 400 and q("SELECT spark_balance::text FROM users WHERE id = %d"
                  % ub) == str(1000 + BOUNTY),
   "http=%s balance=%s" % (s, q("SELECT spark_balance::text FROM users "
                               "WHERE id = %d" % ub)))

print("== R2b 列表口径：无能力的列回 null，不拿 0/等值冒充数据 ==")
s, r = call("GET", "/requests?finished=all", None, token=ta)
mine = [x for x in (r.get("data") or []) if str(x.get("id")) == str(rid)]
row0 = mine[0] if mine else {}
ok("R2b 命中列表能读到自己那张单", bool(row0), "http=%s n=%s"
   % (s, len((r.get("data") or []))))
ok("R2b 评论数/应求数/最新出价为 null（加价与评论线都不存在）",
   row0.get("comments") is None and row0.get("bids") is None
   and row0.get("latest_bounty") is None, str(row0)[:150])
ok("R2b 原始出价仍是真值（null 只给「没有的东西」，不给有的）",
   row0.get("bounty") == BOUNTY, "bounty=%s" % row0.get("bounty"))

print("== R3 原子性：发奖失败必须连状态一起回滚 ==")
s, r = call("POST", "/requests", {"title": "aj 悬赏失败探针 %d" % TS,
                                  "bounty": 500}, token=ta)
rid2 = (r.get("data") or {}).get("id")
if rid2:
    CLEAN.append("DELETE FROM requests WHERE id = %s" % rid2)
ok("R3 第二单建好（已冻结 500）", s == 200 and rid2,
   "http=%s %s" % (s, str(r)[:80]))
# 把应种人余额推到 int64 顶：入账必然 bigint 溢出 → earn 失败
exec_sql("UPDATE users SET spark_balance = 9223372036854775807 "
         "WHERE id = %d" % ub)
s, r = fulfill(tb, rid2, TID)
ok("R3 发奖失败（接口报错，不是静默成功）", s != 200 or r.get("code") != 0,
   "http=%s %s" % (s, str(r)[:90]))
ok("R3 回滚完整：单仍待应、无 payout 流水、求种人无通知",
   q("SELECT status::text FROM requests WHERE id = %s" % rid2) == "0"
   and q("SELECT count(*) FROM spark_ledger WHERE kind = 'task_reward' "
         "AND idempotency_key = 'req-payout:%s'" % rid2) == "0"
   and q("SELECT count(*) FROM messages WHERE receiver_id = %d AND "
         "subject = '%s'" % (ua, SUBJ)) == "1",
   "status=%s ledger=%s pm=%s"
   % (q("SELECT status::text FROM requests WHERE id = %s" % rid2),
      q("SELECT count(*) FROM spark_ledger WHERE kind = 'task_reward' "
        "AND idempotency_key = 'req-payout:%s'" % rid2),
      q("SELECT count(*) FROM messages WHERE receiver_id = %d AND "
        "subject = '%s'" % (ua, SUBJ))))
# 还原余额后同一单应能正常走完（失败不留「不可重试」的死单）
exec_sql("UPDATE users SET spark_balance = %d WHERE id = %d"
         % (1000 + BOUNTY, ub))
s, r = fulfill(tb, rid2, TID)
ok("R3 障碍排除后同一单可重试成功（没有死单）",
   s == 200 and q("SELECT status::text FROM requests WHERE id = %s" % rid2)
   == "1", "http=%s status=%s"
   % (s, q("SELECT status::text FROM requests WHERE id = %s" % rid2)))

print("== R4 收尾：残料清零 ==")
for stmt in reversed(CLEAN):
    exec_sql(stmt)
ok("R4 探针成员已墓碑化",
   q("SELECT count(*) FROM users WHERE email LIKE '%@aj.local' AND "
     "status < 2") == "0",
   q("SELECT count(*) FROM users WHERE email LIKE '%@aj.local' AND "
     "status < 2"))
ok("R4 探针求种单、流水、站内信已清空",
   q("SELECT count(*) FROM requests WHERE title LIKE 'aj %'") == "0"
   and q("SELECT count(*) FROM spark_ledger WHERE user_id IN (%d,%d,%d)"
         % (ua, ub, uc)) == "0"
   and q("SELECT count(*) FROM messages WHERE receiver_id IN (%d,%d,%d)"
         % (ua, ub, uc)) == "0",
   "req=%s ledger=%s pm=%s"
   % (q("SELECT count(*) FROM requests WHERE title LIKE 'aj %'"),
      q("SELECT count(*) FROM spark_ledger WHERE user_id IN (%d,%d,%d)"
        % (ua, ub, uc)),
      q("SELECT count(*) FROM messages WHERE receiver_id IN (%d,%d,%d)"
        % (ua, ub, uc))))
summary()
