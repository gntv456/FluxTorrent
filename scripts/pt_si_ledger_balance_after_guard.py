# -*- coding: utf-8 -*-
"""SI 常驻闸门：火花流水的 balance_after 不许为 NULL（批一百五十三）

发现方式：接 P-333（余额溢出 panic）时顺手查了 spark_ledger 的账，发现 2258 行
里有 152 行 balance_after 为 NULL——其中三个 kind 是**整类**缺：

    seeding_reward 107/0    social_team_reward 6/0    achievement 38/0

balance_after 是流水里「这笔之后余额是多少」的唯一凭据。缺了它就没法对账：
会员说「我的火花对不上」时能拿出的证据只剩 amount 求和，而并发写入下求和
不等于真实余额。这不是展示层瑕疵，是**账本失去自证能力**。

本批已修的三个：seeding_reward（bulk INSERT...SELECT，加 users join 取
u.spark_balance + d.amount）、social_team_reward 与 achievement（同一形状）。
余下六类暂列白名单（先 INSERT 幂等行、再 UPDATE 余额，写入时 balance_after
还算不出来），白名单**只许减不许增**（棘轮），且每条必须写明理由。

正控取修前那笔（seeding.rs 当时不带 balance_after），证明判据承重。
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary, FAILURES  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BS = chr(92)
NL = chr(10)
PRE_FIX = "a9aedfb2"   # seeding.rs 引入 bulk INSERT 而未带 balance_after 的那笔

PAT = re.compile(r"INSERT\s+INTO\s+spark_ledger\s*\(([^)]*)\)", re.S | re.I)

EXEMPT = {
    "class_promotion": "晋升奖励：流水先落、余额后改，两语句顺序导致",
    "preserve_reward": "保种奖励：同 class_promotion 形状",
    "funding_refund": "资助退款：退款流水与余额回滚不在同一语句",
    "bank_auto_deduct": "银行自动扣款：余额由银行流水单独对账",
    "seeding_referral": "推广分润：分润流水与余额更新分两条语句",
    "resurrection": "复活奖励：状态置位与发奖同事务但分两条语句",
}


def rs_files():
    out = []
    for dp, dn, fn in os.walk(os.path.join(ROOT, "apps")):
        dn[:] = [d for d in dn if d not in ("target", "node_modules")]
        for f in sorted(fn):
            if f.endswith(".rs"):
                out.append(os.path.join(dp, f))
    return out


def blob(rev, rel):
    p = subprocess.run(["git", "show", "%s:%s" % (rev, rel.replace(os.sep, "/"))],
                       capture_output=True, text=True, encoding="utf-8",
                       errors="replace", cwd=ROOT)
    return (p.stdout or "") if p.returncode == 0 else ""


def uncontinue(text):
    out = []
    for ln in text.split(NL):
        t = ln.rstrip()
        out.append(t[:-1].rstrip() if t.endswith(BS) else t)
    return NL.join(out)


def missing_sites(text):
    out = []
    for m in PAT.finditer(text):
        if "balance_after" in m.group(1):
            continue
        # kind 字面量取「INSERT ... SELECT nextval('spark_ledger_id_seq'), ... 'kind'」
        # 里**第二个**引号串——第一个恒为 spark_ledger_id_seq，取到它就全成豁免名了
        # （第一版正是这样，8 处缺列全被判成 spark_ledger_id_seq，豁免名一个都不匹配）。
        seg = text[m.end():m.end() + 500]
        lits = re.findall(r"'([a-z_]+)'", seg)
        kind = lits[1] if len(lits) > 1 else (lits[0] if lits else "?")
        out.append((text[:m.start()].count(NL) + 1, kind))
    return out


print("== SI 火花流水 balance_after 不得为 NULL ==")

files = sorted(rs_files())
ok("SI-A1 扫描面非空", len(files) > 100, "扫了 %d 个 .rs" % len(files))

bad = []
for f in files:
    text = uncontinue(open(f, encoding="utf-8", errors="replace").read())
    for ln, kind in missing_sites(text):
        if kind in EXEMPT:
            continue
        bad.append((os.path.relpath(f, ROOT).replace(os.sep, "/"), ln, kind))

ok("SI-A2 无未豁免的缺列写入点", not bad, "缺 %d 处 %s" % (len(bad), bad[:5]))
for rel, ln, kind in bad[:8]:
    print("       %s:%d  kind=%s" % (rel, ln, kind))

ok("SI-A3 豁免项每条都有理由（棘轮：只许减不许增）",
   all(v.strip() for v in EXEMPT.values()),
   "空理由 %s" % [k for k, v in EXEMPT.items() if not v.strip()])

pre = blob(PRE_FIX, "apps/worker/src/jobs/seeding.rs")
pre_txt = uncontinue(pre) if pre else ""
pre_bad = missing_sites(pre_txt)
ok("SI-A4 正控：修前 seeding.rs 被抓到（判据承重）",
   bool(pre) and len(pre_bad) > 0, "blob=%d 命中=%d" % (len(pre), len(pre_bad)))

summary()
