# -*- coding: utf-8 -*-
"""闸门 DP：法币钱包链（充值回调 / 手工补单 / 余额购套餐）的动账四铁律。

批一百六十七。会员视角：捐赠页充值 → 回跳轮询订单状态 → 用余额买上传量/邀请
名额/VIP 套餐。这条钱路此前**没有任何常驻闸门**（写口广度报表里 /donate/topup、
/admin/payment-orders/complete、/donate/order 全在「从未被碰过」那一栏）。
本轮逐行读完 + 现网重测，**没有活体缺陷**，于是把读到的四条铁律钉成判据：

  ① 动账必留痕：每一处 `UPDATE users SET wallet_usd …` 的同一事务里必须写
     donation_ledger（settle.rs 两条入账、donate_admin.rs 手工补单、
     donate_notify.rs 消费口——四处配四对）。少一条就是「钱动了账上查不到」。
  ② 金额一把尺：`payment::check_paid_amount`（批四十四 P-173）是唯一形状判定，
     两条入账入口都必须调它；定义只许一处（第二处＝分家尺子）。
  ③ 结单幂等：结单 SQL 必须带 `status='pending' … RETURNING user_id` 的 CAS；
     出现不带 status 谓词的 `UPDATE payment_orders` 就红（重复回调双发）。
  ④ 账实自洽：donation_ledger 每行的 balance_after 必须等于该用户按 id 顺序的
     running sum；越界即说明某次动账没按同事务写流水（现网实测 0 行不符）。
  ⑤ 订单查询必须带 user 归属（防 IDOR：`WHERE order_no=$1 AND user_id=$2`）。
  ⑥ 回调回执必须是网关认的字面量 success/fail（不能走统一信封）。

现网台账（报数不判红）：`wallet_usd = sum(全部 kind)` 的恒等式在 1 个人身上
不成立（root：流水净额 70、钱包 0）——`kind='order'` 的消费流水是后加的，
历史消费没有对应行。DP4b 把这条不符人数钉成只降不升的上界。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_dp_wallet_chain.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOTDIR, "apps", "api", "src")
WALLET_UNBALANCED_MAX = 1  # ④b 只降不升（历史残留 1 人，新增不符＝洞）
ENTRY = [os.path.join(SRC, "payment", "settle.rs"),
         os.path.join(SRC, "staff_http", "donate_admin.rs"),
         os.path.join(SRC, "staff_http", "donate_notify.rs")]


def rd(p):
    return io.open(p, encoding="utf-8", errors="replace").read()


def cut(f):
    s = re.sub(r"/\*.*?\*/", "", rd(f), flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in s.split("\n"))


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("   psql 失败：%s" % (r.stderr or "").strip()[:220])
    return (r.stdout or "").strip()


def sval(q):
    return psql(q).split("\n")[0].strip()


# ---------- DP1 动账必留痕 ----------
pair, unpaired = [], []
for f in ENTRY:
    s = cut(f)
    moves = len(re.findall(r"UPDATE users SET wallet_usd", s))
    rows = len(re.findall(r"INSERT INTO donation_ledger", s))
    pair.append((os.path.basename(f), moves, rows))
    if moves == 0 or rows < moves:
        unpaired.append("%s(动账 %d 笔/流水 %d 条)" % (os.path.basename(f),
                                                     moves, rows))
ok("DP1 每笔钱包动账都配一条流水 %s（落单 %d）" % (pair, len(unpaired)),
   not unpaired, "、".join(unpaired) or "-")

# ---------- DP2 金额一把尺 ----------
defs = []
for root, _d, names in os.walk(SRC):
    if "target/" in root.replace("\\", "/"):
        continue
    defs += [os.path.join(root, n) for n in names
             if n.endswith(".rs")
             and "fn check_paid_amount" in rd(os.path.join(root, n))]
callers = [os.path.basename(f) for f in ENTRY
           if "check_paid_amount(" in cut(f)]
ok("DP2 金额形状判定只有一把尺（定义 %d 处），入账入口都在用（%s）"
   % (len(defs), "、".join(callers)),
   len(defs) == 1 and len(callers) >= 2,
   "定义处数=%d；调用方=%s" % (len(defs), callers))

# ---------- DP3 结单 CAS ----------
nocas = []
for f in ENTRY:
    s = cut(f)
    for m in re.finditer(r'"UPDATE payment_orders[\s\S]{0,320}?RETURNING', s):
        blk = m.group(0)
        if "status='pending'" not in blk.replace(" ", "") \
                and "status = 'pending'" not in blk:
            nocas.append(os.path.basename(f))
total_cas = sum(len(re.findall(r'"UPDATE payment_orders[\s\S]{0,320}?'
                               r'RETURNING', cut(f))) for f in ENTRY)
ok("DP3 结单语句都带 pending CAS（在册 %d 条、裸更新 %d）"
   % (total_cas, len(nocas)), total_cas >= 2 and not nocas,
   "、".join(sorted(set(nocas))) or "-")

# ---------- DP4 账实自洽（真库） ----------
bad_rows = sval("WITH r AS (SELECT id, abs(balance_after - "
                "sum(amount_usd) OVER (PARTITION BY user_id ORDER BY id)) "
                "AS d FROM donation_ledger) SELECT count(*) FROM r "
                "WHERE d > 0.005")
rows_all = sval("SELECT count(*) FROM donation_ledger")
ok("DP4 流水逐行自洽：%s 行里 balance_after 与累计不符 %s 行"
   % (rows_all, bad_rows), bad_rows == "0",
   "非 0＝某次动账没在同事务写流水（钱动了账上查不到）")
unb = sval("WITH s AS (SELECT user_id, sum(amount_usd) net FROM "
           "donation_ledger GROUP BY 1) SELECT count(*) FROM users u "
           "JOIN s ON s.user_id = u.id WHERE abs(u.wallet_usd - s.net) "
           "> 0.005")
ok("DP4b 钱包与流水净额不符的人数 %s ≤ 上界 %d（只降不升）"
   % (unb, WALLET_UNBALANCED_MAX), int(unb or 0) <= WALLET_UNBALANCED_MAX,
   "kind='order' 消费流水上线前的历史残留；新增不符＝新洞")

# ---------- DP4c 正控：造一条不自洽的流水，DP4 必须数到 ----------
ctl = psql("""
SET lock_timeout = '5s';
BEGIN;
SELECT 'user=' || (SELECT min(user_id) FROM donation_ledger);
INSERT INTO donation_ledger (user_id, kind, amount_usd, balance_after, note)
SELECT min(user_id), 'topup', 5, 999999, 'dp-probe' FROM donation_ledger;
WITH r AS (SELECT id, abs(balance_after - sum(amount_usd) OVER
  (PARTITION BY user_id ORDER BY id)) AS d FROM donation_ledger)
SELECT 'bad=' || count(*) FROM r WHERE d > 0.005 AND id =
  (SELECT max(id) FROM donation_ledger);
ROLLBACK;
""")
planted = re.search(r"bad=(\d+)", ctl)
ok("DP4c 正控：插一条 balance_after=999999 的假流水后判据数到 %s（须 1）"
   % (planted.group(1) if planted else "无读数"),
   bool(planted) and planted.group(1) == "1",
   "数不到＝DP4 是空判据；读数缺失＝psql 报错，别当绿")

# ---------- DP5/DP6 订单归属与回调回执 ----------
st = cut(os.path.join(SRC, "staff_http", "donate.rs"))
ok("DP5 订单状态查询带 user 归属（防越权读别人的支付状态）",
   "order_no = $1" in st and "user_id = $2" in st,
   "只按 order_no 查＝登录用户可探测他人订单状态")
nf = cut(os.path.join(SRC, "staff_http", "donate_notify.rs"))
ok("DP6 回调回执是网关认的字面量 success/fail（不走统一信封）",
   'body("success")' in nf and 'body("fail")' in nf,
   "回执改成 JSON 信封＝网关按非 success 重推或当失败告警")
sf = cut(os.path.join(SRC, "payment", "settle.rs"))
ok("DP6b 入账前必过验签，且验签失败立即拒（不入库）",
   "verify_notify(params)" in sf
   and re.search(r"verify_notify\(params\) else \{\s*return Err", sf),
   "验签被绕过或失败仍往下走＝任何人都能伪造回调铸余额")


summary()
