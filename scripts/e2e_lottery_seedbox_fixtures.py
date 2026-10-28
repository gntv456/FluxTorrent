#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""抽奖 / 种子机凭证 两条零数据链的端到端夹具（自带回收 + 净额核对）。

背景（2026-10-28）：这两条链现网一直是 0 行，等于从没被走过。夹具刻意走**真接口**
而不是手插行——抽奖建帖会冻结魔力、删帖应原路退还，这两步本身就是钱的部分，
手插行绕过后那条链永远验不到。

做法与口径：
- 专属前缀 `[fxlottNNNNN]` 的标题、TEST-NET-2（198.51.100.0/24，文档专用段）的
  CIDR，绝不与真实数据混淆；`finally` 里无论成败都回收。
- 抽奖：建帖（type=lottery, winners=1, prize=1, ticket=0, 24h）→ 手插一张夹具票
  （用户 7482，非真实会员；非公平模式不参与哈希链）→ 读详情确认 `entries>=1`
  → 删帖；核对 `topics/topic_lotteries/孤儿票` 全 0，且 root 流水净额变化
  = 发帖行为奖励（冻结与退还应相互抵消）。
- 凭证：提交 → 库里应有 1 行 → 列表里能看到该 claim → `/me/seedbox/status`
  在 pending 期必须**不放宽**（bonus_factor=1.0、hnr_exempt=false）→ 清理。
"""
import json
import random
import sys

sys.path.insert(0, "scripts")
from pt_audit_lib import call, jwt_token, psql_stdin, summary  # noqa: E402

TOK = jwt_token("root")
TAG = "fxlott%05d" % random.randint(1, 99999)
CIDR = "198.51.100.0/24"
BOT = 7482


def q(sql):
    return psql_stdin(sql).strip()


def ledger_sum(uid):
    return int(q("SELECT COALESCE(sum(amount),0) FROM spark_ledger "
                 "WHERE user_id = %d" % uid) or 0)


def ok(name, cond, detail=""):
    print(("[PASS] " if cond else "[FAIL] ") + name + ((" - " + detail)
                                                       if detail else ""))
    return cond


forum = q("SELECT id FROM forums ORDER BY id LIMIT 1")
sum_before = ledger_sum(1)
tid = None
res = []
try:
    st, body = call("POST", "/forums/topics", token=TOK, body={
        "forum_id": int(forum), "title": "[%s] 夹具：抽奖链走查" % TAG,
        "body": "fixture", "topic_type": "lottery",
        "lottery_winners": 1, "lottery_prize": 1, "lottery_ticket": 0,
        "lottery_hours": 24})
    d0 = body.get("data") if isinstance(body, dict) else None
    tid = (d0 or {}).get("topic_id") if isinstance(d0, dict) else None
    lot = q("SELECT status||'|'||prize_per_winner||'|'||winners FROM "
            "topic_lotteries WHERE topic_id = %s" % tid) if tid else "-"
    res.append(ok("抽奖 建帖入池（status|prize|winners）", st == 200 and
                  lot.startswith("open|1|1"), "status=%s lot=%s" % (st, lot)))
    if tid:
        q("INSERT INTO lottery_entries (topic_id, user_id) VALUES (%s, %d) "
          "ON CONFLICT DO NOTHING" % (tid, BOT))
        st2, b2 = call("GET", "/forums/topics/%s" % tid, token=TOK)
        lo = ((b2.get("data") or {}).get("lottery") or {}) \
            if isinstance(b2, dict) else {}
        res.append(ok("抽奖 详情渲染奖池与票数", st2 == 200
                      and int(lo.get("entries") or 0) >= 1,
                      "entries=%s prize=%s status=%s"
                      % (lo.get("entries"), lo.get("prize"),
                         lo.get("status"))))
finally:
    if tid:
        st3, b3 = call("DELETE", "/forums/topics/%s" % tid, token=TOK)
        res.append(ok("抽奖 删帖", st3 == 200
                      and (b3.get("code") if isinstance(b3, dict) else 0) == 0,
                      "status=%s" % st3))
    left = q("SELECT count(*) FROM topics WHERE title LIKE '[fxlott%%'")
    orph = q("SELECT count(*) FROM topic_lotteries tl LEFT JOIN topics t "
             "ON t.id=tl.topic_id WHERE t.id IS NULL")
    ent = q("SELECT count(*) FROM lottery_entries e WHERE NOT EXISTS "
            "(SELECT 1 FROM topics t WHERE t.id=e.topic_id)")
    net = ledger_sum(1) - sum_before
    res.append(ok("抽奖 零残留（帖/孤儿奖池/孤儿票）", left == "0" and orph
                  == "0" and ent == "0",
                  "left=%s orph=%s ent=%s" % (left, orph, ent)))
    res.append(ok("抽奖 冻结与退还抵消，净额只剩发帖奖励（>=0 且不大）",
                  0 <= net <= 5, "root 流水净额变化=%+d" % net))

st, body = call("POST", "/me/seedbox/claim", token=TOK, body={"cidrs": [CIDR]})
cdata = (body.get("data") or {}) if isinstance(body, dict) else {}
res.append(ok("凭证 提交受理", st == 200 and cdata.get("status") == "pending",
              "status=%s claim=%s" % (st, cdata.get("claim_id"))))
rows = q("SELECT count(*) FROM seedbox_claims WHERE cidrs::text LIKE "
         "'%%198.51.100.0%%'")
st, body = call("GET", "/me/seedbox/claims", token=TOK)
claims = ((body.get("data") or {}).get("claims") or []) \
    if isinstance(body, dict) else []
hit = [c for c in claims if CIDR in json.dumps(c, ensure_ascii=False)]
res.append(ok("凭证 库里 1 行且列表可见", rows == "1" and len(hit) == 1,
              "行=%s 列表命中=%d" % (rows, len(hit))))
st, body = call("GET", "/me/seedbox/status", token=TOK)
sd = (body.get("data") or {}) if isinstance(body, dict) else {}
res.append(ok("凭证 pending 期不放宽（bonus=1.0 / hnr_exempt=false）",
              float(sd.get("bonus_factor") or 0) == 1.0
              and sd.get("hnr_exempt") is False,
              "bonus=%s exempt=%s" % (sd.get("bonus_factor"),
                                      sd.get("hnr_exempt"))))
q("DELETE FROM seedbox_claims WHERE cidrs::text LIKE '%%198.51.100.0%%'")
res.append(ok("凭证 零残留", q("SELECT count(*) FROM seedbox_claims WHERE "
                             "cidrs::text LIKE '%%198.51.100.0%%'") == "0"))

print("== %d/%d 绿" % (sum(1 for r in res if r), len(res)))
summary()
