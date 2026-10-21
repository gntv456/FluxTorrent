# -*- coding: utf-8 -*-
"""闸门 GQ：会员自购「置顶 / 限时免费」这条收钱链路（批二百三十六）。

为什么要有这条：`POST /promo/buy` 是会员花魔力改货架状态的唯一入口，
189 支 pt_audit 里没有一支覆盖它（BA 量的是促销生效后的计费倍率，不是购买侧）。
仓里只有一条 legacy 脚本 `e2e_promo_buy.py`，而它在 HEAD 里就是坏的：
  - 4 处 SQL 跨行拼接丢空格（`ORDERBY` / `WHEREid=` / `free'AND` / `-interval`）
    ⇒ psql 报错、helper 只回 stdout ⇒ `int('')` 崩在断言之前；
  - 3 处 SQL 少了 f 前缀 ⇒ 语句里是字面量 `{tid}`；
  - 幂等键查的是客户原键，而服务端存的是 `promo:<uid>:<客户键>`（10-21 实测）
    ⇒ 「没落流水」是假红；收尾同错 ⇒ 「环境还原」是空判据；
  - 拿 root 余额相等当幂等判据，而 root 会被 worker 的做种奖励随时充值
    （本轮实测 20 分钟内 7575 → 95000+）⇒ 竞态判据。
那条脚本按「未提交件不代改」的规矩留给站方处置（修或删），本支闸门自己把
这条链钉住。判据全部用**行数/状态**，不用绝对余额；扣费额用两次读数之差。

五条判据：
  GQ1 价目表可读（enabled + sticky1/free 两档价），并据此算出本批要花的总额，
      root 余额不够就前置 FAIL 明说（不擅自给 staff 号加钱）。
  GQ2 买一级置顶：code=0、余额差=价目、该幂等键恰 1 行流水 + 1 行购买记录、
      种上 pos_state=1 且 pos_state_until 在未来。
  GQ3 同键重试不双扣：再发一次 ⇒ 流水/购买行仍各 1、余额不再变。
  GQ4 续购顺延 + 档位切换：换 sticky2 买 72h ⇒ pos_state=2 且到期时间变大。
  GQ5 买限时免费：promotions 里确有生效中的 free 行，且列表角标回显 free。
  GQ9 收尾：种状态归 0、促销行与购买行删净并回读残留 0；**流水按设计保留**
      （账务历史不删），断言里同时报「删之前确有行」，免得又是一条空判据。

跑法：PYTHONIOENCODING=utf-8 python scripts/pt_audit_gq_promo_buy.py
需要：栈在跑；root 名下有至少一枚过审种。
"""
import os
import sys
import time

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (call, login, ok, psql_stdin, summary  # noqa: E402
                          )

MARK = "gq-promo-%d" % int(time.time())
# LIKE 的键形：服务端存 `promo:<uid>:<客户键>`，且通配符不能用 %-格式写（`'%s'` 会被格式化掉 ⇒ 等价于等值查，永远 0 行）
PAT = "'%" + MARK + "'"


def q(sql):
    return psql_stdin(sql).strip()


tok = login()
print("== GQ1 价目表与地基 ==")
s, r = call("GET", "/promo/plans", token=tok)
plans = (r.get("data") or {}).get("plans") or {}


def price_of(kind):
    rows = plans.get(kind) or []
    return int(rows[0].get("price")) if rows and rows[0].get("price") else 0


p1, pfree = price_of("sticky1"), price_of("free")
p2 = price_of("sticky2") or p1
need = p1 + p2 + pfree
bal0 = int(q("SELECT spark_balance FROM users WHERE id = 1"))
tid_row = q("SELECT id FROM torrents WHERE owner_id = 1 "
            "AND approval_status = 1 ORDER BY id LIMIT 1")
ok("GQ1 价目表可读且 root 名下有探针种（sticky1=%s sticky2=%s free=%s "
   "root 火花=%s）" % (p1, p2, pfree, bal0),
   (r.get("data") or {}).get("enabled") is True and p1 > 0 and pfree > 0
   and tid_row.isdigit(),
   "enabled=%s http=%s" % ((r.get("data") or {}).get("enabled"), s))
if not (p1 and pfree and tid_row.isdigit()):
    summary()
    sys.exit(1)
tid = int(tid_row)
if bal0 < need:
    ok("GQ1 root 火花够付本批三档（需 %d，实有 %d）" % (need, bal0), False,
       "闸门不给 staff 号擅自加钱：要么先由运营补足，要么这条不该跑")
    summary()
    sys.exit(1)

print("== GQ2 买一级置顶 ==")
s2, b2 = call("POST", "/promo/buy", {"torrent_id": tid, "kind": "sticky1",
                                     "hours": 24, "idempotency_key": MARK},
              token=tok)
bal1 = int(q("SELECT spark_balance FROM users WHERE id = 1"))
led = q("SELECT count(*) FROM spark_ledger WHERE idempotency_key LIKE "
        + PAT)
pur = q("SELECT count(*) FROM promo_purchases WHERE idempotency_key LIKE "
        + PAT)
state = q("SELECT pos_state || '/' || (pos_state_until > now())::text "
          "FROM torrents WHERE id = %d" % tid)
ok("GQ2 扣费=价目 %d 且该键流水 1 行 / 购买记录 1 行（实扣 %s 流水=%s 购=%s）"
   % (p1, bal0 - bal1, led, pur),
   (b2 or {}).get("code") == 0 and bal0 - bal1 == p1 and led == "1"
   and pur == "1", "%s %s" % (s2, str(b2)[:110]))
ok("GQ2 种上生效：pos_state=1 且到期在未来（实测 %s）" % state,
   state.startswith("1/true"), "服务端幂等键前缀=promo:<uid>:（故上面用 LIKE）")

print("== GQ3 同幂等键重试不双扣 ==")
s3, b3 = call("POST", "/promo/buy", {"torrent_id": tid, "kind": "sticky1",
                                     "hours": 24,
                                     "idempotency_key": MARK}, token=tok)
bal2 = int(q("SELECT spark_balance FROM users WHERE id = 1"))
led2 = q("SELECT count(*) FROM spark_ledger WHERE idempotency_key LIKE "
         + PAT)
pur2 = q("SELECT count(*) FROM promo_purchases WHERE idempotency_key LIKE "
         + PAT)
ok("GQ3 重试后该键仍各 1 行、余额不再变（余额 %s→%s 流水=%s 购=%s）"
   % (bal1, bal2, led2, pur2), bal1 == bal2 and led2 == "1" and pur2 == "1",
   "拿余额相等当唯一判据是竞态判据：root 会被做种奖励随时充值，故同时数行")

print("== GQ4 续购顺延 + 档位切换 ==")
until0 = q("SELECT extract(epoch from pos_state_until)::bigint "
           "FROM torrents WHERE id = %d" % tid)
s4, b4 = call("POST", "/promo/buy", {"torrent_id": tid, "kind": "sticky2",
                                     "hours": 72,
                                     "idempotency_key": MARK + "-x2"},
              token=tok)
until1 = q("SELECT extract(epoch from pos_state_until)::bigint "
           "FROM torrents WHERE id = %d" % tid)
pos = q("SELECT pos_state FROM torrents WHERE id = %d" % tid)
ok("GQ4 到期顺延（%s→%s）且档位切到 sticky2（pos_state=%s，应 2）"
   % (until0, until1, pos),
   (b4 or {}).get("code") == 0 and until0.isdigit() and until1.isdigit()
   and int(until1) > int(until0) and pos == "2", "%s %s" % (s4, str(b4)[:100]))

print("== GQ5 买限时免费 ==")
s5, b5 = call("POST", "/promo/buy", {"torrent_id": tid, "kind": "free",
                                     "hours": 24,
                                     "idempotency_key": MARK + "-free"},
              token=tok)
prow = q("SELECT count(*) FROM promotions WHERE torrent_id = %d "
         "AND kind = 'free' AND starts_at <= now() AND ends_at > now()" % tid)
s6, r6 = call("GET", "/torrents?alive=0&limit=100", token=tok)
me = next((i for i in ((r6.get("data") or {}).get("items") or [])
           if i.get("id") == tid), {})
ok("GQ5 促销行生效（%s 行）且列表角标回显 free（实测 %s）"
   % (prow, me.get("promotion")),
   (b5 or {}).get("code") == 0 and prow.isdigit() and int(prow) >= 1
   and me.get("promotion") == "free",
   "code=%s 角标=%s 位置=%s" % ((b5 or {}).get("code"), me.get("promotion"),
                              list(me.keys())[:6]))

print("== GQ9 收尾 ==")
before = q("SELECT count(*) FROM promo_purchases WHERE idempotency_key LIKE "
           + PAT)
q("UPDATE torrents SET pos_state = 0, pos_state_until = NULL WHERE id = %d"
  % tid)
q("DELETE FROM promotions WHERE torrent_id = %d AND starts_at > "
  "now() - interval '10 minutes'" % tid)
q("DELETE FROM promo_purchases WHERE idempotency_key LIKE " + PAT)
after = q("SELECT count(*) FROM promo_purchases WHERE idempotency_key LIKE "
          + PAT)
res_led = q("SELECT count(*) FROM spark_ledger WHERE idempotency_key LIKE "
            + PAT)
t0 = q("SELECT pos_state FROM torrents WHERE id = %d" % tid)
ok("GQ9 还原（删前 %s 行→删后 %s，种状态=%s；流水按设计保留 %s 行）"
   % (before, after, t0, res_led),
   before.isdigit() and int(before) >= 1 and after == "0" and t0 == "0"
   and res_led.isdigit() and int(res_led) >= int(before),
   "「删前 ≥1」是让这条断言不空——旧脚本只查删后=0，永远成立")

summary()
