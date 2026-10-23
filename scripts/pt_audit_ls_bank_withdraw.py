# -*- coding: utf-8 -*-
"""LS 闸门：银行定存「支取后 interest 列必须等于实付利息」（P-639）。

病形（2026-10-24 现网取证）：`/bank/withdraw` 提前支取不计息，到账公式是
`amount − penalty`，**利息一分不发**；但它只写 `status/penalty/withdrawn_at`，
`bank_deposits.interest` 留着建单时的**预计值**。而这个列有人读：
  · `GET /bank/deposits`（bank.rs:147）`WHERE user_id = $1 ORDER BY id DESC
    LIMIT 50` —— 不按 status 过滤，已结清的旧单照样返回；
  · `components/bank-actions-parts.tsx:47` —— `${dict.bank.interest}:
    ${d.interest.toLocaleString()}`，文案是「到期利息」，且不看 status。
于是页面上长期挂着一张「已结清 + 到期利息: 12」的单，而 12 从没进过任何人
的余额。现网实证（两条都是提前支取）：
    id=1  amount 5000 interest 12 penalty 25 → 流水实付 4975（=本金−罚金）
    id=2  amount 50000 interest 123 penalty 250 → 流水实付 49750

同族记忆：「A 处校验 B 处展示的字段要比对是不是同一张表」「失败/未发放不许
演成账面数字」。修法取「写者把展示字段算准」：支取事务里把 interest 写成
**实付利息**（与回执 `interest_earned` 同一个变量），列的语义从「预计」收到
「实付」，前端与任何后来的 SUM 都不用各自猜。

判据两层：
  静态：支取 UPDATE 必须带 interest、回执必须复用同一变量（不许两处表达式）、
        worker 到期兑付**不许**被误抄成清零（那条 interest 是真发出去的）、
        前端渲染 interest 的那行仍不看 status（依赖关系钉住：列必须等于实付）、
        `withdrawn_at` 至今零读者＝写了没人读的待裁桩（有人接上读者会红一次）。
  活体：探针号存 2000/365 天（利息必 >0）→ 支取 → 回执 paid=本金−罚金、
        interest_earned=0 → 库里 interest=0 → **读口回显 interest=0**（渲染层
        证据）→ 流水恰一行且金额=paid → 重放第二次被挡且流水仍一行 →
        余额净损只有罚金（不多发不少发）。
正控：LS1/LS2 跑在 CTRL_REV（修复前提交）的 bank_withdraw.rs 上必须报洞。
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, "scripts")
from pt_audit_lib import (call, dig, login, ok, psql_stdin, FAILURES)  # noqa: E402

API = "apps/api/src"
WORKER = "apps/worker/src"
WEB = "apps/web/components"
CTRL_REV = "a3affae2"  # 本批修复的前一个提交（绝对钉，别用 HEAD~1）
WITHDRAW = API + "/economy_http/bank_withdraw.rs"
AMOUNT = 2000          # >= bank_min_deposit(100)，且净损只有罚金 10
TERM = 365


def read(rel, rev=None):
    if rev:
        out = subprocess.run(["git", "show", "%s:%s" % (rev, rel)],
                             capture_output=True, encoding="utf-8",
                             errors="replace")
        return out.stdout if out.returncode == 0 else None
    try:
        with open(rel, encoding="utf-8") as f:
            return f.read()
    except OSError:
        return None


def sql_of(src, needle):
    """取包含 needle 的那条 SQL 字面量。

    先把 Rust 字符串内的续行 `\\`+换行 接上，再**从 needle 往两边找引号**——
    不能用「正则扫全部字面量再筛」：像 "daily" 这种内容不足 10 字的字面量
    会让 `{10,}?` 配对漂移，把后一条字面量的开引号吃掉（LS1 第一次就是这么
    读成空串、把「已经修好」报成 FAIL 的）。
    """
    joined = src.replace("\\\n", "")
    i = joined.find(needle)
    if i < 0:
        return ""
    left = joined.rfind('"', 0, i)
    right = joined.find('"', i + len(needle))
    if left < 0 or right < 0:
        return ""
    return " ".join(joined[left + 1:right].split())


# ---------------------------------------------------------------- 静态层
src_w = read(WITHDRAW) or ""
upd = sql_of(src_w, "UPDATE bank_deposits SET status = 1")
ok("LS1 支取的销单 UPDATE 必须同时归置 interest（列=实付，不留预计值）",
   "interest =" in upd, upd[:150])
ok("LS2 回执与列同源（interest_earned 复用同一个变量，不许两处各写各的）",
   '"interest_earned": paid_interest_final' in src_w
   and "interest_earned\": if mode" not in src_w,
   "还在用第二份表达式")

ctrl_w = read(WITHDRAW, CTRL_REV)
upd_old = sql_of(ctrl_w, "UPDATE bank_deposits SET status = 1") if ctrl_w else ""
ok("LS3 正控：%s 那版必须被 LS1 报洞（判据承重）" % CTRL_REV,
   ctrl_w is not None and "interest =" not in upd_old,
   "旧版 UPDATE=%s" % upd_old[:120])

# worker 到期兑付：interest 是真发出去的钱，绝不许被误抄成清零
wd_dep = read(WORKER + "/bank_jobs/deposit.rs") or ""
wins = []
for m in re.finditer(r"UPDATE bank_deposits SET status = 1", wd_dep):
    wins.append(wd_dep[m.start():m.start() + 260])
ok("LS4 worker 到期兑付的销单语句不动 interest（那条利息确实发了）",
   bool(wins) and all("interest =" not in w for w in wins),
   "命中 %d 段：%s" % (len(wins), [w[:80] for w in wins][:1]))
ok("LS5 worker 兑付的入账额=本金+利息（与 API 的 maturity 分支同尺）",
   re.search(r"payable = amount \+ final_interest", wd_dep) is not None,
   "读不到 payable 公式")

# 前端依赖：那一行不看 status ⇒ 列必须等于实付（谁改了前端就红一次提醒）
parts = read(WEB + "/bank-actions-parts.tsx") or ""
line = next((l for l in parts.splitlines()
             if "dict.bank.interest" in l), "")
guarded = "d.status" in line or "status ===" in line
ok("LS6 前端仍无条件渲染 interest（依赖在册：列=实付才不撒谎）",
   line != "" and not guarded, "该行=%s" % line.strip()[:120])

# 待裁桩：withdrawn_at 写了没人读（手动路径写、自动兑付不写）
rd = subprocess.run(["git", "grep", "-n", "withdrawn_at", "--", API, WORKER],
                    capture_output=True, encoding="utf-8", errors="replace")
hits = [l for l in (rd.stdout or "").splitlines()
        if "withdrawn_at = now()" not in l and "ADD COLUMN" not in l]
ok("LS7 待裁桩在册：withdrawn_at 仍是「写了没人读」的列（接上读者请销桩）",
   len(hits) == 0, "读者=%s" % hits[:3])

print()
if os.environ.get("LS_SKIP_LIVE"):
    print("（LS_SKIP_LIVE=1：跳过活体层）")
    print("红账：%s" % (FAILURES or "无"))
    sys.exit(1 if FAILURES else 0)
print("=" * 68)
print("二、活体链（探针号存一笔再提前支取，打的是 %s）"
      % os.environ.get("FLUX_API_BASE", "http://127.0.0.1:8080/api/v1"))
print("=" * 68)
tok = login()
bal0 = int(psql_stdin("SELECT spark_balance FROM users WHERE id=1"))
rows0 = psql_stdin("SELECT count(*) FROM bank_deposits")
did, pen = None, None
try:
    s, r = call("POST", "/bank/deposit", {"amount": AMOUNT,
                                          "term_days": TERM}, tok)
    did = dig(r, "data", "id")
    est = dig(r, "data", "interest")
    ok("LS8 探针存单成立（%d 魔力 / %d 天，预计利息必须 >0 才算正控）"
       % (AMOUNT, TERM), s == 200 and did and est and est > 0,
       "http=%s 回执=%s" % (s, dig(r, "data") or r))
    s, r = call("GET", "/bank/deposits", None, tok)
    mine = [d for d in (dig(r, "data") or []) if d.get("id") == did]
    ok("LS8b 未结清时读口回显预计利息（这就是缺陷本体的入口状态）",
       len(mine) == 1 and mine[0]["status"] == 0
       and mine[0]["interest"] == est,
       "行=%s" % mine)
    s, r = call("POST", "/bank/withdraw", {"deposit_id": did}, tok)
    paid, earned = dig(r, "data", "paid"), dig(r, "data", "interest_earned")
    pen = dig(r, "data", "penalty")
    ok("LS9 提前支取只付本金−罚金（interest_earned 必须 0）",
       s == 200 and pen and paid == AMOUNT - pen and earned == 0,
       "http=%s paid=%s penalty=%s earned=%s" % (s, paid, pen, earned))
    db = psql_stdin(
        "SELECT status::text||'|'||interest::text||'|'||penalty::text||'|'||"
        "coalesce((withdrawn_at IS NOT NULL)::int,-1)::text FROM bank_deposits "
        "WHERE id=%d" % did).split("|")
    ok("LS10 库里该单 interest 已归零（列语义收到「实付」）",
       db[0] == "1" and db[1] == "0" and int(db[2]) > 0,
       "status/interest/penalty=%s" % db)
    s, r = call("GET", "/bank/deposits", None, tok)
    mine = [d for d in (dig(r, "data") or []) if d.get("id") == did]
    ok("LS11 渲染层证据：读口不再把没发的利息吐给界面",
       len(mine) == 1 and mine[0]["interest"] == 0,
       "行=%s" % mine)
    led = psql_stdin(
        "SELECT count(*)||'|'||coalesce(sum(amount),-1) FROM spark_ledger "
        "WHERE idempotency_key='withdraw:%d'" % did).split("|")
    ok("LS12 流水恰一行且金额=实付（钱货两讫，不含利息）",
       led[0] == "1" and int(led[1]) == paid, "行数/合计=%s" % led)
    s2, r2 = call("POST", "/bank/withdraw", {"deposit_id": did}, tok)
    led2 = psql_stdin(
        "SELECT count(*) FROM spark_ledger WHERE idempotency_key='withdraw:%d'"
        % did)
    ok("LS13 重放被挡且不多发（400「已处理」，流水仍 1 行）",
       s2 == 400 and led2 == "1",
       "http=%s msg=%s 流水=%s" % (s2, dig(r2, "message"), led2))
finally:
    bal1 = int(psql_stdin("SELECT spark_balance FROM users WHERE id=1"))
    kinds = psql_stdin(
        "SELECT string_agg(DISTINCT kind, ',') FROM spark_ledger "
        "WHERE user_id=1 AND created_at > now() - interval '2 minutes'")
    ok("LS14 净损只有罚金（存 %d 取回 %d−penalty，余额差应 =−penalty）"
       % (AMOUNT, AMOUNT),
       pen is not None and bal1 == bal0 - pen,
       "前=%d 后=%d 差=%d penalty=%r" % (
           bal0, bal1, bal1 - bal0, pen))
    ok("LS15 牵连面清点：这两分钟只动了 bank_deposit/bank_withdraw 两种流水",
       set(kinds.split(",")) <= {"bank_deposit", "bank_withdraw"},
       "kind=%s" % kinds)
    rows1 = psql_stdin("SELECT count(*) FROM bank_deposits")
    ok("LS16 结清探针单留在库里（历史账不删；删 spark_ledger/bank 行是红线）",
       int(rows1) == int(rows0) + 1, "前=%s 后=%s" % (rows0, rows1))

print()
print("红账：%s" % (FAILURES or "无"))
sys.exit(1 if FAILURES else 0)
