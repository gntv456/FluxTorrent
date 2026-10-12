# -*- coding: utf-8 -*-
"""闸门 AW：银行/贷款/结息端到端（批三十二）。

钉住七件事（前三件事先说清「为什么这样断言」）：
  S2 P-84 部分还款的利息蒸发：游标推到今天、却把「今日计提 - 已付」漏在账外。
     只还 1 魔力就能把自上次结息以来的全部应计利息抹掉（每天可复现）。
     断言分两段：部分还款后 accrued_interest 必须等于 today_int-1；
     随后全额结清必须收 本金 + (today_int-1)，两次扣款之和 = 本金 + today_int。
  S3 P-85 负数成了「一次性结清」：body.amount.filter(|a| *a > 0) 把 -1 静默
     降级成缺省全结清。断言 -1 → 400 且贷款仍 active；反控 999999 → 结清但
     只收 payoff（不许按 999999 扣）。
  S4 P-83 last_interest_date 为 NULL 时 query_as 解 NaiveDate 直接 500。
     期望：按 worker bank_loan_interest 同口径（COALESCE created_at-1）计息后
     正常结清，不是 500。
  S5 结息 worker 全链路：手动触发 bank_daily（面板「执行」同一函数），钉
     ①正常计息 ②活期 NULL 游标修复（P-86）③逾期罚息 NULL 游标修复（P-89）
     ④定期到期自动结清本息守恒 ⑤同函数重跑零双付（幂等）。
  S6 P-87 bank_enabled 是零消费者假开关（settings_meta 可见可改，全仓无人
     读）。按 0193 先例：断言 settings_meta/site_settings 里已无此键。
     同节顺带实测真开关 module_bank 关→/bank/* 404+4101→恢复。
  S1/S6 其余为链路正控：定存/活期/贷款的扣款-入账守恒、幂等重放拒绝、
     超额/超限拒单、总览字段与设置一致。

共享设置：bank_fixed_settle_mode、module_bank 串行改、快照原值写回并回读。
探针产物（两个成员号 + 银行各行 + 流水 + 站内信 + 两次手动触发行）自己收尾。

跑法：
    PYTHONIOENCODING=utf-8 python scripts/pt_audit_aw_bank_loan.py
"""
import atexit
import os
import re
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    BASE, call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
PIN = {}
CLEAN = []
UIDS = []
TRIGGERS = []
SPARK0 = {}


def q(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % r.stderr.strip()[:200])
    return r.stdout.strip()


def ex(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-c", sql],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode:
        print("   ! SQL 失败: %s" % (r.stderr or "").strip()[:150])
    return r.returncode


def qi(sql, default=0):
    try:
        v = q(sql)
        return int(v) if v else default
    except Exception:  # noqa: BLE001
        return default


def setting(name):
    return q("SELECT COALESCE((SELECT value FROM site_settings WHERE name = "
             "'%s'), '')" % name)


def setv(name, val):
    assert ex("UPDATE site_settings SET value = '%s' WHERE name = '%s'"
              % (val, name)) == 0
    got = setting(name)
    print("   [set] %s = %s（回读 %s）" % (name, val, got))
    return got == val


def pin(name):
    PIN[name] = setting(name)


def cleanup():
    """可重复执行：设置原值回读、触发行、银行行、流水、成员号全收干净。"""
    for name in ("bank_fixed_settle_mode", "module_bank"):
        if name in PIN:
            ex("UPDATE site_settings SET value = '%s' WHERE name = '%s'"
               % (PIN[name], name))
    for tid in TRIGGERS:
        ex("DELETE FROM job_triggers WHERE id = %d" % tid)
    for uid in UIDS:
        for t in ("bank_interest_records", "bank_deposits",
                  "bank_demand_accounts", "bank_loans", "spark_ledger",
                  "messages"):
            col = "receiver_id" if t == "messages" else "user_id"
            ex("DELETE FROM %s WHERE %s = %d" % (t, col, uid))
        if uid in SPARK0:
            ex("UPDATE users SET spark_balance = %d WHERE id = %d"
               % (SPARK0[uid], uid))
        ex("UPDATE users SET status = 2 WHERE id = %d" % uid)
    for stmt in reversed(CLEAN):
        ex(stmt)
    del CLEAN[:]
    print("   [cleanup] module_bank=%s settle_mode=%s"
          % (setting("module_bank"), setting("bank_fixed_settle_mode")))


atexit.register(cleanup)


def brief(x):
    return str(x)[:200]


def today():
    return q("SELECT CURRENT_DATE::text")


def loan_int(p, rate_bp, days):
    """economy::loan_interest 同口径：(p*rate*days + 9999)/10000。"""
    return (p * rate_bp * days + 9999) // 10000


def rej(name, s, r, code, kw):
    msg = str(r.get("message", "")) if isinstance(r, dict) else str(r)
    return ok(name, s == code and kw in msg, "%s %s %s" % (s, msg, brief(r)))


def new_member(role):
    clear_register_rl()
    time.sleep(6)
    codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                   token=ROOT)[1].get("data") or {}).get("codes")) or []
    if not codes:
        return None
    code, cid = codes[0]["code"], codes[0]["id"]
    CLEAN.append("DELETE FROM invites WHERE id = %d" % cid)
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if len(nums) < 2:
        return None
    user = "aw%s%d" % (role, TAG)
    s, rr = call("POST", "/auth/register", {
        "username": user, "email": "%s@aw.local" % user,
        "password": "password123", "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}})
    uid = (rr.get("data") or {}).get("user_id")
    if not uid:
        return None
    UIDS.append(uid)
    tok = None
    for _ in range(8):
        tok = (call("POST", "/auth/login",
                    {"username": user, "password": "password123"})[1].get(
                        "data") or {}).get("token")
        if tok:
            break
        time.sleep(6)
    return (tok, uid) if tok else None


def run_bank_daily():
    """面板「执行」同一路径：入队 → worker 60s 内认领 → 轮询到收口。"""
    s, r = call("POST", "/admin/jobs/run", {"job": "bank_daily"}, token=ROOT)
    tid = (r.get("data") or {}).get("id")
    if not tid:
        return "enqueue-fail", s, brief(r)
    TRIGGERS.append(int(tid))
    deadline = time.time() + 150
    while time.time() < deadline:
        row = q("SELECT status || '|' || ok::text || '|' || COALESCE(result,"
                "'') FROM job_triggers WHERE id = %d" % tid)
        parts = row.split("|", 2)
        if len(parts) == 3 and parts[0] in ("done", "failed"):
            return parts[0], parts[1], parts[2]
        time.sleep(5)
    return "timeout", "?", ""


print("== S0 地基：刷新探针成员 + 共享设置快照 ==")
pin("bank_fixed_settle_mode")
pin("module_bank")
# 结息模式锁定 maturity：全额口径确定（快照原值，收尾写回并回读）
setv("bank_fixed_settle_mode", "maturity")
MIN_DEPOSIT = int(setting("bank_min_deposit") or 100)
MAX_DEPOSIT = int(setting("bank_max_deposit") or 1000000)
MIN_DEMAND = int(setting("bank_min_demand") or 100)
MIN_LOAN = int(setting("bank_min_loan") or 100)
RATIO = int(setting("bank_loan_ratio") or 100)
CONST = int(setting("bank_loan_ratio_constant") or 1000)
PB = int(setting("bank_overdue_penalty_bp") or 50)
A = new_member("a")
B = new_member("b")
ok("两个探针成员可准备（注册/登录限流下）", bool(A and B))
if not (A and B):
    sys.exit(1)
AT, AID = A
BT, BID = B
for uid in (AID, BID):
    SPARK0[uid] = qi("SELECT spark_balance FROM users WHERE id = %d" % uid)
ex("UPDATE users SET spark_balance = 500000 WHERE id IN (%d, %d)"
   % (AID, BID))
spark_a0 = qi("SELECT spark_balance FROM users WHERE id = %d" % AID)


def spark(uid):
    return qi("SELECT spark_balance FROM users WHERE id = %d" % uid)


s, r = call("GET", "/bank/overview", token=AT)
ov = r.get("data") or {}
ok("S0 总览可达且字段齐", s == 200 and r.get("code") == 0
   and isinstance(ov.get("limits"), dict)
   and len(ov.get("fixed_rates") or []) == 5
   and len(ov.get("loan_rates") or []) == 5
   and (ov.get("site") or {}).get("settle_mode") == "maturity",
   "%s %s" % (s, brief(ov)))
ok("S0 总览 limits 与设置一致",
   (ov.get("limits") or {}).get("min_deposit") == MIN_DEPOSIT
   and (ov.get("limits") or {}).get("min_demand") == MIN_DEMAND
   and (ov.get("limits") or {}).get("min_loan") == MIN_LOAN,
   brief(ov.get("limits")))

print("== S1 定存/活期/贷款健康路径（扣款-入账守恒 + 幂等 + 拒单）==")
s, r = call("POST", "/bank/deposit",
            {"amount": 10000, "term_days": 30, "idempotency_key": "awd1"},
            token=AT)
dep_id = (r.get("data") or {}).get("id")
ok("S1 定存 10000/30天 成立", s == 200 and dep_id and spark(AID) == spark_a0 - 10000,
   "%s %s" % (s, brief(r)))
dep_int = qi("SELECT interest FROM bank_deposits WHERE id = %s" % dep_id)
s2, r2 = call("POST", "/bank/deposit",
              {"amount": 10000, "term_days": 30, "idempotency_key": "awd1"},
              token=AT)
rej("S1 同键定存重放被拒（不双扣）", s2, r2, 400, "请勿重复提交")
ok("S1 重放后余额未动",
   spark(AID) == spark_a0 - 10000
   and qi("SELECT count(*) FROM bank_deposits WHERE user_id = %d" % AID) == 1)
s2, r2 = call("POST", "/bank/deposit",
              {"amount": MAX_DEPOSIT + 1, "term_days": 30}, token=AT)
rej("S1 超单笔上限拒单", s2, r2, 400, "不可超过")
s2, r2 = call("POST", "/bank/deposit", {"amount": 100, "term_days": 29},
              token=AT)
rej("S1 非法期限拒单", s2, r2, 400, "期限")
s, r = call("POST", "/bank/demand/deposit",
            {"amount": 20000, "idempotency_key": "awdd1"}, token=AT)
ok("S1 活期存入 20000", s == 200 and qi(
    "SELECT balance FROM bank_demand_accounts WHERE user_id = %d" % AID)
    == 20000 and spark(AID) == spark_a0 - 30000, "%s %s" % (s, brief(r)))
rej("S1 活期低于单笔下限拒单", *call("POST", "/bank/demand/deposit",
                                      {"amount": MIN_DEMAND - 1},
                                      token=AT), 400, "不少于")
s, r = call("POST", "/bank/demand/withdraw",
            {"amount": 5000, "idempotency_key": "awdw1"}, token=AT)
ok("S1 活期支取 5000 入余额", s == 200
   and qi("SELECT balance FROM bank_demand_accounts WHERE user_id = %d" % AID)
   == 15000 and spark(AID) == spark_a0 - 25000, "%s %s" % (s, brief(r)))
s2, r2 = call("POST", "/bank/demand/withdraw",
              {"amount": 5000, "idempotency_key": "awdw1"}, token=AT)
rej("S1 同键支取重放被拒", s2, r2, 400, "请勿重复提交")
rej("S1 活期超取拒单", *call("POST", "/bank/demand/withdraw",
                             {"amount": 99999999}, token=AT), 400, "余额不足")
ok("S1 超取后活期余额未动", qi(
    "SELECT balance FROM bank_demand_accounts WHERE user_id = %d" % AID)
    == 15000)

s, r = call("POST", "/bank/loan/apply", {"amount": 5000, "term_days": 7},
            token=AT)
LOAN1 = (r.get("data") or {}).get("id")
RATE = (r.get("data") or {}).get("daily_rate_bp")
ok("S1 贷款 5000/7天 放款", s == 200 and LOAN1 and RATE
   and qi("SELECT remaining FROM bank_loans WHERE id = %s" % LOAN1) == 5000
   and spark(AID) == spark_a0 - 20000, "%s %s" % (s, brief(r)))
rej("S1 在贷期间再申请被拒", *call("POST", "/bank/loan/apply",
                                   {"amount": 5000, "term_days": 7},
                                   token=AT), 400, "已有未结清")
rej("S1 超额度上限拒单", *call("POST", "/bank/loan/apply",
                               {"amount": CONST + 1, "term_days": 7},
                               token=AT), 400, "不可超过额度上限")
ok("S1 贷款历史含本笔", any(
    (x or {}).get("id") == LOAN1
    for x in (call("GET", "/bank/loans", token=AT)[1].get("data") or [])))

print("== S2 P-84 部分还款：利息守恒（红=accrued 被抹成 0）==")
ex("UPDATE bank_loans SET last_interest_date = CURRENT_DATE - 3, "
   "accrued_interest = 0 WHERE id = %s" % LOAN1)
TODAY_INT = loan_int(5000, RATE, 3)
sp = spark(AID)
s, r = call("POST", "/bank/loan/repay",
            {"amount": 1, "idempotency_key": "awrp1"}, token=AT)
acc = qi("SELECT accrued_interest FROM bank_loans WHERE id = %s" % LOAN1)
cur = q("SELECT last_interest_date::text FROM bank_loans WHERE id = %s" % LOAN1)
ok("S2 部分还款 1 魔力：今日欠息应落回 accrued（%d）" % (TODAY_INT - 1),
   s == 200 and acc == TODAY_INT - 1,
   "status=%s accrued=%s resp=%s" % (s, acc, brief(r)))
ok("S2 游标推到今天且本金未动",
   cur == today() and qi("SELECT remaining FROM bank_loans WHERE id = %s"
                         % LOAN1) == 5000 and spark(AID) == sp - 1,
   "cur=%s" % cur)
sp = spark(AID)
s, r = call("POST", "/bank/loan/repay", {"amount": 0}, token=AT)
paid = (r.get("data") or {}).get("paid")
ok("S2 反控：全额结清应收 本金+欠息 = %d" % (5000 + TODAY_INT - 1),
   s == 200 and paid == 5000 + TODAY_INT - 1 and spark(AID) == sp - paid
   and q("SELECT status FROM bank_loans WHERE id = %s" % LOAN1) == "paid",
   "%s %s" % (s, brief(r)))

print("== S3 P-85 负数还款（红=静默全结清）==")
s, r = call("POST", "/bank/loan/apply", {"amount": 5000, "term_days": 7},
            token=AT)
LOAN2 = (r.get("data") or {}).get("id")
s2, r2 = call("POST", "/bank/loan/repay",
              {"amount": -1, "idempotency_key": "awneg"}, token=AT)
rej("S3 amount=-1 必须 400 拒单", s2, r2, 400, "必须为正")
ok("S3 拒单后贷款仍 active、本金未动",
   q("SELECT status FROM bank_loans WHERE id = %s" % LOAN2) == "active"
   and qi("SELECT remaining FROM bank_loans WHERE id = %s" % LOAN2) == 5000)
sp = spark(AID)
s, r = call("POST", "/bank/loan/repay", {"amount": 999999}, token=AT)
paid = (r.get("data") or {}).get("paid")
ok("S3 反控：超大额只收 payoff=5000", s == 200 and paid == 5000
   and spark(AID) == sp - 5000
   and q("SELECT status FROM bank_loans WHERE id = %s" % LOAN2) == "paid",
   "%s %s" % (s, brief(r)))

print("== S4 P-83 NULL 游标还款（红=500）==")
s, r = call("POST", "/bank/loan/apply", {"amount": 5000, "term_days": 7},
            token=AT)
LOAN3 = (r.get("data") or {}).get("id")
ex("UPDATE bank_loans SET last_interest_date = NULL WHERE id = %s" % LOAN3)
EXP3 = loan_int(5000, RATE, 1)  # created 今 → COALESCE(created-1) = 昨天
sp = spark(AID)
s, r = call("POST", "/bank/loan/repay", {"amount": 0}, token=AT)
paid = (r.get("data") or {}).get("paid")
ok("S4 NULL 游标按 worker 同口径计息后结清（应收 %d）" % (5000 + EXP3),
   s == 200 and paid == 5000 + EXP3 and spark(AID) == sp - paid
   and q("SELECT status FROM bank_loans WHERE id = %s" % LOAN3) == "paid",
   "%s %s" % (s, brief(r)))

print("== S5 结息 worker：手动触发 bank_daily（与面板同一函数）==")
# 清掉红基线可能残留的 active 贷款（各节失败时留下），保证唯一 active 约束
ex("UPDATE bank_loans SET status = 'paid', remaining = 0, accrued_interest = 0 "
   "WHERE user_id IN (%d, %d) AND status = 'active'" % (AID, BID))
# ① A：正常计息行（游标=昨天）——同时是「任务确实跑了」的正控
ex("INSERT INTO bank_loans (user_id, amount, daily_rate_bp, penalty_rate_bp, "
   "term_days, remaining, due_at, last_interest_date, status) VALUES "
   "(%d, 10000, %d, %d, 7, 10000, now() + interval '7 days', "
   "CURRENT_DATE - 1, 'active')" % (AID, RATE, PB))
L_INT = qi("SELECT id FROM bank_loans WHERE user_id = %d AND status = 'active'"
           % AID)
EXP_INT = loan_int(10000, RATE, 1)
# ② A 活期账户留 NULL 游标（P-86：红=永久跳过、绿=修复为今天、0 利息）
ex("UPDATE bank_demand_accounts SET last_interest_date = NULL "
   "WHERE user_id = %d" % AID)
# ③ B：逾期贷款 NULL 游标 + rate=0（防 loan_interest 提前修游标），due 2 天前
ex("INSERT INTO bank_loans (user_id, amount, daily_rate_bp, penalty_rate_bp, "
   "term_days, remaining, due_at, last_interest_date, status) VALUES "
   "(%d, 100000, 0, %d, 24, 100000, now() - interval '2 days', NULL, "
   "'active')" % (BID, PB))
L_PEN = qi("SELECT id FROM bank_loans WHERE user_id = %d AND status = 'active'"
           % BID)
EXP_PEN = loan_int(100000, PB, 2)
# ④ B：到期定存（maturity 模式，本息一次结清）
s, r = call("POST", "/bank/deposit",
            {"amount": 3000, "term_days": 7, "idempotency_key": "awb1"},
            token=BT)
DEP2 = (r.get("data") or {}).get("id")
DEP2_INT = qi("SELECT interest FROM bank_deposits WHERE id = %s" % DEP2)
ex("UPDATE bank_deposits SET maturity_at = now() - interval '1 minute' "
   "WHERE id = %s" % DEP2)
ok("S5 四件套就位（计息行/活期NULL/逾期NULL/到期定存）",
   L_INT and L_PEN and DEP2 and DEP2_INT is not None,
   "L_INT=%s L_PEN=%s DEP2=%s" % (L_INT, L_PEN, DEP2))
SPA, SPB = spark(AID), spark(BID)
st, okk, res = run_bank_daily()
ok("S5 手动触发 bank_daily 收口成功", st == "done" and okk in ("t", "true"),
   "status=%s ok=%s res=%s" % (st, okk, res))
acc = qi("SELECT accrued_interest FROM bank_loans WHERE id = %s" % L_INT)
cur = q("SELECT last_interest_date::text FROM bank_loans WHERE id = %s" % L_INT)
ok("S5① 正常计息 1 天 = %d 且游标推今天" % EXP_INT,
   acc == EXP_INT and cur == today()
   and qi("SELECT count(*) FROM bank_interest_records WHERE kind = 'loan' "
          "AND reference_id = %s" % L_INT) == 1,
   "accrued=%s cur=%s" % (acc, cur))
dcur = q("SELECT last_interest_date::text FROM bank_demand_accounts "
         "WHERE user_id = %d" % AID)
ok("S5② 活期 NULL 游标被修复为今天且 0 利息（P-86）",
   dcur == today() and qi("SELECT balance FROM bank_demand_accounts "
                          "WHERE user_id = %d" % AID) == 15000,
   "cur=%s" % dcur)
pen_rem = qi("SELECT remaining FROM bank_loans WHERE id = %s" % L_PEN)
pen_cur = q("SELECT last_interest_date::text FROM bank_loans WHERE id = %s"
            % L_PEN)
ok("S5③ 逾期罚息并入本金 = %d（P-89）" % (100000 + EXP_PEN),
   pen_rem == 100000 + EXP_PEN and pen_cur == today()
   and qi("SELECT count(*) FROM bank_interest_records WHERE "
          "kind = 'loan_penalty' AND reference_id = %s" % L_PEN) == 1,
   "rem=%s cur=%s" % (pen_rem, pen_cur))
dst = q("SELECT status::text FROM bank_deposits WHERE id = %s" % DEP2)
ok("S5④ 到期定存本息入账 = %d" % (3000 + DEP2_INT),
   dst == "1" and spark(BID) == SPB + 3000 + DEP2_INT
   and qi("SELECT count(*) FROM spark_ledger WHERE idempotency_key = "
          "'withdraw:%s'" % DEP2) == 1,
   "status=%s spark=%s" % (dst, spark(BID)))
ok("S5 健康游标已写（站点日）", qi(
    "SELECT count(*) FROM bank_settle_runs WHERE run_date = "
    "(CURRENT_TIMESTAMP + interval '8 hours')::date AND finished_at IS NOT NULL"
) == 1)
SPA2, SPB2 = spark(AID), spark(BID)
st, okk, res = run_bank_daily()
ok("S5 同函数重跑收口成功（幂等）", st == "done" and okk in ("t", "true"),
   "status=%s ok=%s res=%s" % (st, okk, res))
ok("S5 重跑零双付：余额/计息/罚息/存款全不变",
   spark(AID) == SPA2 and spark(BID) == SPB2
   and qi("SELECT accrued_interest FROM bank_loans WHERE id = %s" % L_INT)
   == EXP_INT
   and qi("SELECT remaining FROM bank_loans WHERE id = %s" % L_PEN)
   == 100000 + EXP_PEN
   and q("SELECT status::text FROM bank_deposits WHERE id = %s" % DEP2) == "1")

print("== S6 P-87 假开关摘除 + module_bank 真开关实测 ==")
ok("S6 bank_enabled 已从 settings_meta 摘除（P-87）",
   qi("SELECT count(*) FROM settings_meta WHERE name = 'bank_enabled'") == 0
   and qi("SELECT count(*) FROM site_settings WHERE name = 'bank_enabled'")
   == 0)
setv("module_bank", "no")
print("   [wait] 模块缓存 TTL 30s，等失效后断言")
time.sleep(33)
s, r = call("POST", "/bank/deposit", {"amount": 1000, "term_days": 7},
            token=AT)
ok("S6 module_bank=no → 存款口 404/4101",
   s == 404 and r.get("code") == 4101, "%s %s" % (s, brief(r)))
s2, r2 = call("GET", "/bank/overview", token=AT)
ok("S6 module_bank=no → 总览口 404/4101",
   s2 == 404 and r2.get("code") == 4101, "%s %s" % (s2, brief(r2)))
setv("module_bank", PIN["module_bank"])
time.sleep(33)
s, r = call("GET", "/bank/overview", token=AT)
ok("S6 恢复 module_bank 后总览回 200",
   s == 200 and r.get("code") == 0, "%s %s" % (s, brief(r)))
ok("S6 存款口恢复后可用（无残留 4101）",
   call("POST", "/bank/deposit", {"amount": 1000, "term_days": 7},
        token=AT)[0] == 200
   and qi("SELECT count(*) FROM bank_deposits WHERE user_id = %d" % AID) == 2)

print()
summary()
