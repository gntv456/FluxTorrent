# -*- coding: utf-8 -*-
"""闸门 GU：定向众筹（0078）——参与、幂等、追投、取消退款的整条链。

缘起（批二百三十九 P-480 实测）：管理员取消众筹并勾选退款时，接口回
`200 {refund:true, refunded:1}`，`spark_ledger` 里也多了一条 +1000 的
`funding_refund` 流水，但**会员余额一分没变**。根因是这个口自己拼两条语句：
INSERT 带 `WHERE NOT EXISTS(key)` 幂等闸门，紧接着的 UPDATE 又带**同款**
NOT EXISTS——上一条已经把那个键插进本会话可见的表，UPDATE 恒 0 行。
同族的 worker 版（`apps/worker/src/jobs/settle.rs` 到期退款）与参与口
（`funding_contribute.rs`）都是「事务内 INSERT 决定 inserted，UPDATE 不再带
闸门」，唯独后台取消口没跟上。现网活体：参与 5000→4000，取消后仍 4000，
流水 1 行 +1000 且 balance_after 为 NULL；机制复现（事务内）b0=4000 / b1=4000。
修法是改走共用入账原语 `earn_spark_tx`（行锁→查幂等→落流水含 balance_after→
加余额），所以本闸门把「取消那一刻余额就回来了」当成断言——旧镜像这条红，
镜像重建后转绿。

判据（全部用本轮专属前缀，夹具自建自删；流水是历史，不删）：
  GU1 建众筹（goal≥1000 校验 + 落库 status=0）；
  GU2 参与一笔：扣款含税、raised 只计净额、`funding` 流水恰 1 行且带
      balance_after（参与口本来就是对的，这一条是它的正控对照）；
  GU3 同幂等键重放被拒，且不双扣；
  GU4 追投：contribs 累加、raised 累加；
  GU5 取消退款：接口回 refunded=N 的同时**余额即时**回到参与前，退款流水
      恰 N 行、净额=实付合计、每行都带 balance_after；
  GU6 状态守卫：已关闭的众筹不能再次取消（防误退已挂促销的）；
  GU7 收尾：本轮夹具（众筹行 + 参与记录）删除，探针停用，流水残留报数。

跑法：PYTHONIOENCODING=utf-8 python scripts/pt_audit_gu_funding_refund.py
"""
import inspect
import os
import sys
import time

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, psql_stdin, summary  # noqa: E402

RUN = int(time.time())
MARK = "gu%d" % RUN
PAID, MORE, GOAL = 1000, 300, 2000


def sval(q):
    return psql_stdin(q).strip()


def bal(uid):
    return int(sval("SELECT spark_balance FROM users WHERE id = %d" % uid))


def rows(uid, kind):
    return sval("SELECT count(*)||'/'||coalesce(sum(amount),0) FROM "
                "spark_ledger WHERE user_id = %d AND kind = '%s'"
                % (uid, kind))


ROOT = login()
PW = inspect.signature(login).parameters["pw"].default
tid = sval("SELECT id FROM torrents WHERE approval_status = 1 "
           "ORDER BY id LIMIT 1")
st, r = call("POST", "/fundings", {"torrent_id": int(tid), "goal": GOAL,
                                  "hours": 168, "days": 14}, token=ROOT)
fid = ((r or {}).get("data") or {}).get("id")
frow = sval("SELECT status||'/'||goal||'/'||raised FROM fundings WHERE id = %s"
            % fid)
ok("GU1 建众筹 code=0 且落库 status=0/goal=%s/raised=0（实测 %s）"
   % (GOAL, frow), (r or {}).get("code") == 0 and frow.startswith("0/%s/0"
                                                                  % GOAL),
   "http=%s 种子=%s" % (st, tid))

user = "%su" % MARK
call("POST", "/admin/adduser", {"username": user, "password": PW,
                                "email": "%s@gu.local" % MARK}, token=ROOT)
uid = int(sval("SELECT id FROM users WHERE username = '%s'" % user))
t0 = login(user)
call("POST", "/me/password/change", {"old_password": PW,
                                     "new_password": PW + "x"}, token=t0)
mtok = login(user, PW + "x")
sf, rf = call("POST", "/admin/amountbonus", {"amount": 5000, "user_id": uid,
                                             "idempotency_key":
                                             "%s-fund" % MARK}, token=ROOT)
b_before = bal(uid)
ok("GU0 探针成员可用且已注资（uid=%d 余额=%d http=%s）"
   % (uid, b_before, sf), mtok is not None and b_before >= 5000,
   "code=%s 注资走发放口（不落直写 SQL，否则 seeding 重算会把余额刷回去）"
   % (rf or {}).get("code"))

s2, r2 = call("POST", "/fundings/contribute",
              {"funding_id": int(fid), "amount": PAID,
               "idempotency_key": "%s-c1" % MARK}, token=mtok)
d2 = (r2 or {}).get("data") or {}
b_paid = bal(uid)
tax = d2.get("tax", 0)
led_f = rows(uid, "funding")
ba_null = int(sval("SELECT count(*) FROM spark_ledger WHERE user_id = %d "
                   "AND kind='funding' AND balance_after IS NULL" % uid))
ok("GU2 参与一笔：扣款=实付 %d、raised=净额 %d、税 %d 自洽，"
   "`funding` 流水恰 1 行且带 balance_after（实测 %s ba_null=%d）"
   % (PAID, d2.get("raised"), tax, led_f, ba_null),
   (r2 or {}).get("code") == 0 and b_before - b_paid == PAID
   and d2.get("paid") == PAID and d2.get("raised") == PAID - tax
   and led_f.startswith("1/") and ba_null == 0,
   "响应=%s 余额 %d→%d" % (str(d2)[:110], b_before, b_paid))

s3, r3 = call("POST", "/fundings/contribute",
              {"funding_id": int(fid), "amount": PAID,
               "idempotency_key": "%s-c1" % MARK}, token=mtok)
b_after_replay = bal(uid)
ok("GU3 同幂等键重放被拒且不双扣（http=%s 余额 %d→%d）"
   % (s3, b_paid, b_after_replay),
   (r3 or {}).get("code") != 0 and b_after_replay == b_paid,
   str(r3)[:110])

s4, r4 = call("POST", "/fundings/contribute",
              {"funding_id": int(fid), "amount": MORE,
               "idempotency_key": "%s-c2" % MARK}, token=mtok)
b2 = bal(uid)
contrib = sval("SELECT amount||'/'||tax FROM funding_contribs WHERE "
               "funding_id = %s AND user_id = %d" % (fid, uid))
paid_total = PAID + MORE
ok("GU4 追投累加：contribs 记 %d（实付 %d）、余额再减 %d（实测 %s）"
   % (paid_total, MORE, MORE, contrib),
   (r4 or {}).get("code") == 0 and contrib.split("/")[0] == str(paid_total)
   and b_paid - b2 == MORE, "raised=%s" % str(r4)[:80])

s5, r5 = call("POST", "/admin/fundings/cancel", {"id": int(fid),
                                                 "refund": True}, token=ROOT)
b3 = bal(uid)
led_ref = rows(uid, "funding_refund")
ba_null2 = int(sval("SELECT count(*) FROM spark_ledger WHERE user_id = %d "
                    "AND kind='funding_refund' AND balance_after IS NULL"
                    % uid))
st_row = sval("SELECT status FROM fundings WHERE id = %s" % fid)
ok("GU5 取消退款：接口回 refunded=1 的**同一刻**余额回到参与前 %d→%d，"
   "退款流水 1 行 / +%d 且带 balance_after，众筹置为已关闭（status=%s）"
   % (b2, b3, paid_total, st_row),
   (r5 or {}).get("code") == 0 and b3 == b_before and st_row == "2"
   and led_ref == "1/%d" % paid_total and ba_null2 == 0,
   "P-480 本体：旧镜像此处余额不动（UPDATE 恒 0 行）且 ba_null=1；"
   "响应=%s 流水=%s 余额 %d→%d" % (str((r5 or {}).get("data"))[:70],
                                    led_ref, b2, b3))

s6, r6 = call("POST", "/admin/fundings/cancel", {"id": int(fid)}, token=ROOT)
ok("GU6 已关闭的众筹不能再次取消（http=%s %s）"
   % (s6, str((r6 or {}).get("message"))[:26]),
   (r6 or {}).get("code") != 0 and "进行中" in str((r6 or {}).get("message")),
   str(r6)[:110])

print("== GU7 收尾 ==")
sval("DELETE FROM funding_contribs WHERE funding_id = %s" % fid)
sval("DELETE FROM fundings WHERE id = %s" % fid)
sval("UPDATE users SET status = 2 WHERE id = %d" % uid)
gone = sval("SELECT count(*) FROM fundings WHERE id = %s" % fid)
left = sval("SELECT string_agg(id::text, ',') FROM spark_ledger WHERE "
            "user_id = %d" % uid)
ok("GU7 夹具已清（众筹行剩 %s 条）；探针 %d 停用，其流水 %s 保留为历史账"
   % (gone, uid, left), gone == "0"
   and sval("SELECT status FROM users WHERE id = %d" % uid) == "2",
   "参与/退款/注资流水不删；会员侧余额最终为 %d" % bal(uid))

summary()
