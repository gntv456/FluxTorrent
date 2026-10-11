# -*- coding: utf-8 -*-
"""批二十七闸门 AR：求字幕悬赏池与工作流（0146 pots / 0148 工作流）。

覆盖：发起冻结 → 凑赏入池 → 撤销退款 → 旧直交付（fulfill）→
认领工作流（crew 确认/交稿/验收分账）→ 弃单 → worker 超时回池与自动验收
→ 月度评选与字幕人认证 → 模块负控。

读写面（上传/列表/下载/评分/举报）在姊妹闸门 aq；两封共用
pt_audit_aq_util，**必须串行跑**（都会改 module_subtitles 等共享设定）。

本批修复的哨兵：
  P-43 悬赏池没有取消/退款出口（0146 词表 status=2「已撤销」从未实现），
       冻结的火花永久悬空——与已修的 P-26 求种悬赏蒸发同型；
  P-44（web 侧，取证见文档）弃单按钮 POST 的是 /accept —— 本闸门钉住
       /abandon 端点语义，防后端被改坏。
"""
import os
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402
from pt_audit_aq_util import make_exec  # noqa: E402

CLEAN = []
E = make_exec(CLEAN)
q = E["q"]
exec_sql = E["exec_sql"]
setting = E["setting"]
mkuser = E["mkuser"]
mkbackdated = E["mkbackdated"]
probe_torrent = E["probe_torrent"]
run_job = E["run_job"]
spark_of = E["spark_of"]
grant_spark = E["grant_spark"]
ledger_rows = E["ledger_rows"]
wait_row = E["wait_row"]
wait_http = E["wait_http"]
pre_clean = E["pre_clean"]
cleanup = E["cleanup"]
E["queue_final_sweep"]()

login()
TS = int(time.time()) % 100000
PW = "Gate#123456"
SET_KEYS = ("module_subtitles", "subtitle_workflow", "subtitle_award",
            "subtitle_claim_timeout_days", "subtitle_accept_timeout_days")

print("== AR-S0 前置：自愈残料 + 设定（串行改、收尾还原） ==")
print("  [pre] 求单=%s（自愈后）"
      % q("SELECT count(*) FROM subtitle_requests"))
pre_clean()
SET0 = {}
for k in SET_KEYS:
    SET0[k] = q("SELECT COALESCE((SELECT value FROM site_settings WHERE "
                "name = '%s'), '<absent>')" % k)
    CLEAN.append("DELETE FROM site_settings WHERE name = '%s'" % k
                 if SET0[k] == "<absent>"
                 else "UPDATE site_settings SET value = '%s' WHERE "
                      "name = '%s'" % (SET0[k], k))
setting("module_subtitles", "yes")
setting("subtitle_workflow", "0")
setting("subtitle_award", "1")
setting("subtitle_claim_timeout_days", "7")
setting("subtitle_accept_timeout_days", "3")
ok("S0 设定就位（module 开 / workflow=0 / award=1）",
   q("SELECT value FROM site_settings WHERE name='subtitle_workflow'")
   == "0")

uRef, tRef = mkuser("aqRef%d" % TS, PW)
uVet, tVet = mkbackdated("aqVet%d" % TS, PW + "x", src_uid=uRef)
uNew, tNew = mkbackdated("aqNew%d" % TS, PW + "x", src_uid=uRef)
uMe, tMe = mkbackdated("aqMe%d" % TS, PW + "x", src_uid=uRef)
uC1, tC1 = mkbackdated("aqC1%d" % TS, PW + "x", src_uid=uRef)
grant_spark(uMe, 5000, "me")
grant_spark(uC1, 5000, "c1")
grant_spark(uVet, 1000, "vet")
TID = probe_torrent(uMe, "AqPool.%d" % TS)
ok("S0 探针齐（发起/凑赏/老兵/第三人）", uMe > 0 and uC1 > 0)


def new_req(bounty, descr, tok=tMe, torrent=True):
    s, r = call("POST", "/subtitles/requests", {
        "lang": "zh", "torrent_id": TID if torrent else 0,
        "descr": descr, "bounty": bounty}, token=tok)
    return s, (r.get("data") or {}).get("id"), r


print("== AR-S1 悬赏池：冻结 / 凑赏 / 撤销退款（P-43 哨兵） ==")
bMe, bC1 = spark_of(uMe), spark_of(uC1)
s, rid, r = new_req(1000, "AqGate pool")
ok("S1 发起冻结 1000（余额减 + 流水入账）",
   s == 200 and rid and bMe - spark_of(uMe) == 1000
   and ledger_rows(uMe, "subtitle_request") == 1, str(r)[:110])
s, r = call("POST", "/subtitles/requests/%d/contribute" % (rid or 0),
            {"amount": 500}, token=tC1)
ok("S1 凑赏入池（池额 1500、C1 减 500）",
   s == 200 and q("SELECT bounty FROM subtitle_requests WHERE id = %d"
                  % (rid or 0)) == "1500" and bC1 - spark_of(uC1) == 500,
   str(r)[:110])
ok("S1 反例：负赏金被拒", new_req(-5, "AqGate neg")[0] == 400)
ok("S1 反例：0 赏金不必扣款也能建单", new_req(0, "AqGate zero")[0] == 200)
s, r = call("POST", "/subtitles/requests/%d/cancel" % (rid or 0), None,
            token=tMe)
ok("S1 哨兵：未被认领的单可撤销并按明细退款（P-43 修复点）",
   s == 200 and q("SELECT status FROM subtitle_requests WHERE id = %d"
                  % (rid or 0)) == "2" and spark_of(uMe) == bMe
   and spark_of(uC1) == bC1,
   "http=%s st=%s me=%+d c1=%+d" % (
       s, q("SELECT status FROM subtitle_requests WHERE id = %d"
            % (rid or 0)), spark_of(uMe) - bMe, spark_of(uC1) - bC1))
ok("S1 退款流水按人各一条",
   ledger_rows(uMe, "subtitle_request_refund") == 1
   and ledger_rows(uC1, "subtitle_request_refund") == 1,
   "me=%s c1=%s" % (ledger_rows(uMe, "subtitle_request_refund"),
                    ledger_rows(uC1, "subtitle_request_refund")))
ok("S1 反例：已撤销单不可再撤（CAS）",
   call("POST", "/subtitles/requests/%d/cancel" % (rid or 0), None,
        token=tMe)[0] == 400)
s, rid6, _ = new_req(700, "AqGate other")
ok("S1 反例：非发起人不能撤别人的单",
   call("POST", "/subtitles/requests/%d/cancel" % (rid6 or 0), None,
        token=tC1)[0] in (400, 403))

print("== AR-S2 交付通道（workflow=0 fulfill）+ 双发反例 ==")
s, rid2, _ = new_req(800, "AqGate ff")
sidV = (call("POST", "/subtitles", {"title": "AqSub ff", "lang": "zh",
                                    "file_ref": "https://x/c.srt",
                                    "torrent_id": TID},
             token=tVet)[1].get("data") or {}).get("id")
bVet = spark_of(uVet)
s, r = call("POST", "/subtitles/requests/%d/fulfill" % (rid2 or 0),
            {"subtitle_id": sidV}, token=tVet)
ok("S2 正控：他人交付整池给交付人（+800、status=1）",
   s == 200 and spark_of(uVet) - bVet == 800
   and q("SELECT status FROM subtitle_requests WHERE id = %d" % (rid2 or 0))
   == "1", "http=%s delta=%s" % (s, spark_of(uVet) - bVet))
ok("S2 反例：已处理单不可重复交付（双发被 CAS 挡）",
   call("POST", "/subtitles/requests/%d/fulfill" % (rid2 or 0),
        {"subtitle_id": sidV}, token=tVet)[0] == 400)
ok("S2 反例：非字幕作者不能代为交付",
   call("POST", "/subtitles/requests/%d/fulfill" % (rid2 or 0),
        {"subtitle_id": sidV}, token=tC1)[0] in (400, 404))

print("== AR-S3 认领工作流（workflow=1）：认领→crew→交稿→验收分账 ==")
setting("subtitle_workflow", "1")
s, rid3, r = new_req(1000, "AqGate wf")
s, r = call("POST", "/subtitles/requests/%d/claim" % (rid3 or 0),
            {"crew": [{"user_id": uC1, "share": 20, "role": "校对"},
                      {"user_id": uNew, "share": 10, "role": "时间轴"}]},
            token=tVet)
ok("S3 认领生效（status=3、crew 2 人、deadline 7 天）",
   s == 200 and q("SELECT status FROM subtitle_requests WHERE id = %d"
                  % (rid3 or 0)) == "3"
   and q("SELECT jsonb_array_length(crew) FROM subtitle_requests WHERE "
         "id = %d" % (rid3 or 0)) == "2", str(r)[:110])
ok("S3 反例：已被认领的单不能再认领",
   call("POST", "/subtitles/requests/%d/claim" % (rid3 or 0), None,
        token=tC1)[0] == 400)
s, rid4, _ = new_req(0, "AqGate sm")
ok("S3 反例：发起人不能认领自己的单",
   call("POST", "/subtitles/requests/%d/claim" % (rid4 or 0), None,
        token=tMe)[0] == 400)
ok("S3 反例：份额合计超 100 被拒",
   call("POST", "/subtitles/requests/%d/claim" % (rid4 or 0),
        {"crew": [{"user_id": uVet, "share": 60, "role": "a"},
                  {"user_id": uC1, "share": 60, "role": "b"}]},
        token=tC1)[0] == 400)
ok("S3 反例：crew 里写自己（认领人本人）被拒",
   call("POST", "/subtitles/requests/%d/claim" % (rid4 or 0),
        {"crew": [{"user_id": uC1, "share": 10, "role": "a"},
                  {"user_id": uC1, "share": 90, "role": "self"}]},
        token=tC1)[0] == 400)
call("POST", "/subtitles/requests/%d/crew-accept" % (rid3 or 0), None,
     token=tC1)
call("POST", "/subtitles/requests/%d/crew-accept" % (rid3 or 0), None,
     token=tNew)
ok("S3 两名 crew 均确认",
   q("SELECT count(*) FROM subtitle_requests r, jsonb_array_elements("
     "r.crew) m WHERE r.id = %d AND (m->>'accepted') = 'true'"
     % (rid3 or 0)) == "2")
ok("S3 反例：无协作邀请的人确认被拒",
   call("POST", "/subtitles/requests/%d/crew-accept" % (rid3 or 0), None,
        token=tRef)[0] == 400)
sidW = (call("POST", "/subtitles", {"title": "AqSub W", "lang": "zh",
                                    "file_ref": "https://x/w.srt",
                                    "torrent_id": TID},
             token=tVet)[1].get("data") or {}).get("id")
ok("S3 反例：非主认领人不能交稿",
   call("POST", "/subtitles/requests/%d/deliver" % (rid3 or 0),
        {"subtitle_id": sidW}, token=tC1)[0] == 400)
s, r = call("POST", "/subtitles/requests/%d/deliver" % (rid3 or 0),
            {"subtitle_id": sidW}, token=tVet)
ok("S3 主认领人交稿（status=4）",
   s == 200 and q("SELECT status FROM subtitle_requests WHERE id = %d"
                  % (rid3 or 0)) == "4", str(r)[:90])
bV0, bC0, bN0 = spark_of(uVet), spark_of(uC1), spark_of(uNew)
s, r = call("POST", "/subtitles/requests/%d/accept" % (rid3 or 0), None,
            token=tMe)
ok("S3 验收分账：c1 +200、new +100、主认领人(老兵) +700、status=1",
   s == 200 and spark_of(uC1) - bC0 == 200
   and spark_of(uNew) - bN0 == 100 and spark_of(uVet) - bV0 == 700
   and q("SELECT status FROM subtitle_requests WHERE id = %d"
         % (rid3 or 0)) == "1",
   "http=%s vet=+%d c1=+%d new=+%d" % (s, spark_of(uVet) - bV0,
                                       spark_of(uC1) - bC0,
                                       spark_of(uNew) - bN0))
ok("S3 分账幂等键每人一条（crew2 + 认领人 = 3 条）",
   q("SELECT count(*) FROM spark_ledger WHERE kind = 'subtitle_bounty' "
     "AND idempotency_key LIKE 'subtitle-bounty-pay:%d:%%'"
     % (rid3 or 0)) == "3")
ok("S3 反例：已验收单再验收被拒（不双发）",
   call("POST", "/subtitles/requests/%d/accept" % (rid3 or 0), None,
        token=tMe)[0] == 400)

print("== AR-S4 弃单端点语义（P-44 的后端面） ==")
s, rid5, _ = new_req(0, "AqGate ab")
ok("S4 正控：第三人可认领空池单",
   call("POST", "/subtitles/requests/%d/claim" % (rid5 or 0), None,
        token=tC1)[0] == 200
   and q("SELECT status FROM subtitle_requests WHERE id = %d" % (rid5 or 0))
   == "3", str(s))
s, r = call("POST", "/subtitles/requests/%d/abandon" % (rid5 or 0), None,
            token=tC1)
ok("S4 弃单回池（status=0、claimed_by 清空）",
   s == 200 and q("SELECT status::text||'|'||COALESCE(claimed_by::text,"
                  "'null') FROM subtitle_requests WHERE id = %d"
                  % (rid5 or 0)) == "0|null", str(r)[:90])
ok("S4 弃单落审计（禁令的计数来源）",
   int(q("SELECT count(*) FROM audit_log WHERE action = 'subreq.abandon' "
         "AND actor_id = %d" % uC1)) >= 1)
ok("S4 反例：非认领人不能代弃",
   call("POST", "/subtitles/requests/%d/abandon" % (rid5 or 0), None,
        token=tVet)[0] == 400)

print("== AR-S5 worker 超时线：认领过期回池 + 交稿逾期自动验收 ==")
# 先把单真的推到「已认领且逾期」：只改 deadline_at 会让 sweep 无事可做
exec_sql("UPDATE subtitle_requests SET status = 3, claimed_by = %d, "
         "claimed_at = now() - interval '8 days', deadline_at = now() - "
         "interval '1 hour' WHERE id = %d" % (uC1, rid4))
jok, jinfo = run_job("subreq_sweep")
ok("S5 subreq_sweep 真跑", jok is True, jinfo)
ok("S5 认领超时→回池 status=0、claimed_by 清空",
   q("SELECT status::text||'|'||COALESCE(claimed_by::text,'null') FROM "
     "subtitle_requests WHERE id = %d" % rid4) == "0|null",
   q("SELECT status FROM subtitle_requests WHERE id = %d" % rid4))
ok("S5 超时回池落 subreq.abandon 审计（拖到超时也记账）",
   int(q("SELECT count(*) FROM audit_log WHERE action = 'subreq.abandon' "
         "AND ref::text LIKE '%%\"id\":%d%%'" % rid4)) >= 1)
ok("S5 超时通知认领人 x1",
   wait_row("SELECT count(*) FROM messages WHERE receiver_id = %d AND "
            "subject = '字幕认领已超时'" % uC1, "1"), jinfo)
exec_sql("UPDATE subtitle_requests SET status = 4, deliver_at = now() - "
         "interval '10 days', claimed_by = %d, bounty = 600 WHERE id = %d"
         % (uC1, rid4))
jok, jinfo = run_job("subreq_sweep")
ok("S5 交稿逾期→自动验收 status=1（与人工同键域不双发）",
   q("SELECT status FROM subtitle_requests WHERE id = %d" % rid4) == "1"
   and q("SELECT count(*) FROM spark_ledger WHERE idempotency_key = "
         "'subtitle-bounty-pay:%d:%d'" % (rid4, uC1)) == "1", jinfo)
ok("S5 自动验收通知发起人 x1",
   wait_row("SELECT count(*) FROM messages WHERE receiver_id = %d AND "
            "subject = '字幕已自动验收'" % uMe, "1"), jinfo)

print("== AR-S6 月度评选与字幕人认证（回档月） ==")
PERIOD = q("SELECT to_char(date_trunc('month', now()) - interval '1 day', "
           "'YYYY-MM')")
sidOld = int(q("INSERT INTO subtitles (user_id, title, lang, file_ref, "
               "size, ext, status, deleted_at, created_at, rating_count, "
               "rating_sum, downloads, machine_translated) VALUES (%d, "
               "'AqSub back', 'zh', 'https://x/old.srt', 10, 'srt', 1, "
               "NULL, (date_trunc('month', now()) - interval '1 day')::date,"
               " 6, 54, 40, FALSE) RETURNING id" % uVet))
s, r = call("POST", "/admin/subtitles/awards/build?period=%s" % PERIOD,
            None, token=login())
ok("S6 候选生成命中回档字幕（period=%s）" % PERIOD,
   s == 200 and int(((r.get("data") or {}).get("added")) or 0) >= 1,
   "http=%s %s" % (s, str(r)[:100]))
aid = q("SELECT id FROM subtitle_awards WHERE subtitle_id = %d LIMIT 1"
        % sidOld)
bV1 = spark_of(uVet)
s, r = call("POST", "/admin/subtitles/awards/grant",
            {"period": PERIOD, "grants": [{"id": int(aid or 0),
                                           "rank": 1}]}, token=login())
ok("S6 授金 rank1：火花 5000 + rank 落库",
   s == 200 and spark_of(uVet) - bV1 == 5000
   and q("SELECT rank FROM subtitle_awards WHERE id = %s" % (aid or "0"))
   == "1", "http=%s delta=%s" % (s, spark_of(uVet) - bV1))
ok("S6 揭晓信 x1",
   q("SELECT count(*) FROM messages WHERE receiver_id = %d AND subject = "
     "'金字幕评选揭晓'" % uVet) == "1")
ok("S6 反例：重复授金不双发（幂等键 + rank=0 卫语句）",
   call("POST", "/admin/subtitles/awards/grant",
        {"period": PERIOD, "grants": [{"id": int(aid or 0),
                                       "rank": 1}]},
        token=login())[1].get("data", {}).get("granted", [{}])[0].get(
            "skipped") is True)
jok, jinfo = run_job("subcert_sweep")
gold = wait_row("SELECT tier FROM user_subtitle_certs WHERE user_id = %d "
                "AND revoked_at IS NULL ORDER BY id DESC LIMIT 1" % uVet,
                "gold")
ok("S6 subcert_sweep 授 gold（获奖≥1）", jok is True and gold, jinfo)
jok, jinfo = run_job("subcert_sweep")
ok("S6 复扫幂等（不重复授、行不抖）",
   jok is True and q("SELECT count(*) FROM user_subtitle_certs WHERE "
                     "user_id = %d AND tier='gold'" % uVet) == "1", jinfo)

print("== AR-S7 模块负控：关 subtitles 后读写皆被网关拦 ==")
setting("module_subtitles", "no")
ok("S7 模块关：读口 404（4101 未开放）",
   call("GET", "/subtitles", token=tVet)[0] == 404)
ok("S7 模块关：建求单被拦",
   new_req(0, "AqGate gated")[0] == 404)
setting("module_subtitles", "yes")
ok("S7 模块开：读口在一拍内恢复", wait_http("GET", "/subtitles", tVet, 200))

print("== AR-S8 收尾：设定逐项还原 + 探针清零 ==")
cleanup()
back = [k for k in SET_KEYS if q(
    "SELECT COALESCE((SELECT value FROM site_settings WHERE name = '%s'), "
    "'<absent>')" % k) != SET0[k]]
ok("S8 共享设定全部还原（%s）" % "/".join(SET_KEYS), not back, str(back))
ok("S8 探针求单/字幕/池额清零",
   q("SELECT (SELECT count(*) FROM subtitle_requests WHERE descr LIKE "
     "'AqGate %%')::text||'|'||(SELECT count(*) FROM subtitles WHERE title "
     "LIKE 'Aq%%')::text||'|'||COALESCE((SELECT sum(bounty) FROM "
     "subtitle_requests),0)::text") == "0|0|0")
ok("S8 探针账号全部停用",
   int(q("SELECT count(*) FROM users WHERE username LIKE 'aq%%' AND "
         "status < 2")) == 0)
ok("S8 分账/退款流水不残留",
   q("SELECT count(*) FROM spark_ledger WHERE kind IN ('subtitle_bounty',"
     "'subtitle_request','subtitle_request_refund') AND user_id IN "
     "(SELECT id FROM users WHERE username LIKE 'aq%%')") == "0")
summary()
