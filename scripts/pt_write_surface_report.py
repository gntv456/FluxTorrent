# -*- coding: utf-8 -*-
"""写口广度盘点（排期工具，不是判据）：所有写口路由 vs 常驻闸门文本。

用法：PYTHONIOENCODING=utf-8 python -u scripts/pt_write_surface_report.py
闸门只保证「被点名的路由」不退化；这份清单回答的是「还有哪些写口我压根没用过」。
退出码恒 0——它是报表，不做门禁，别挂进 CI 当绿灯。

资深会员把站点用一遍，最难的是知道自己「还没用到哪些」。GET 广度有 DA 钉，
写口（POST/PUT/DELETE）没有对账——本脚本只做一件事：把 utoipa/actix 宏里的
写路径全捞出来，再扫 scripts/pt_audit_*.py 的文本，报告没人提过的路由。
这是排期工具，不是判据（不做 exit code 承诺）。
"""
import io
import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "apps", "api", "src")
GATES = os.path.join(ROOT, "scripts")

routes = {}
for root, _d, names in os.walk(SRC):
    if "target/" in root.replace("\\", "/"):
        continue
    for n in names:
        if not n.endswith(".rs"):
            continue
        p = os.path.join(root, n)
        s = io.open(p, encoding="utf-8", errors="replace").read()
        for m in re.finditer(r'#\[(post|put|delete|patch)\(\s*"([^"]+)"', s):
            routes.setdefault(m.group(2), set()).add(
                os.path.relpath(p, ROOT).replace("\\", "/"))
        # #[utoipa::path(post, path = "/api/v1/…")] 也认
        for m in re.finditer(r'(post|put|delete|patch),\s*\n?\s*path = '
                             r'"(/api/v1[^"]+)"', s):
            routes.setdefault(m.group(2).replace("/api/v1", ""), set()).add(
                os.path.relpath(p, ROOT).replace("\\", "/"))

corpus = ""
for n in sorted(os.listdir(GATES)):
    if n.startswith("pt_audit_") and n.endswith(".py"):
        corpus += io.open(os.path.join(GATES, n), encoding="utf-8",
                          errors="replace").read()

touched, untouched = [], []
for path in sorted(routes):
    leaf = path.split("/")[-1] or path
    key = path.lstrip("/")
    if key in corpus or ("/" + key) in corpus:
        touched.append(path)
    else:
        untouched.append(path)

print("写口路由共 %d 条：被闸门提到 %d 条，从未提到 %d 条"
      % (len(routes), len(touched), len(untouched)))
print("\n=== 从未被任何闸门提到的写口（按文件聚合成 15 组）===")
by = {}
for p in untouched:
    for f in routes[p]:
        by.setdefault(f.split("/")[-1], []).append(p)
for f in sorted(by, key=lambda x: -len(by[x]))[:15]:
    print("%-34s %s" % (f, ", ".join(sorted(by[f])[:6])))
print("\n=== 其中带钱味的（spark/buy/pay/donate/loan/gift/tip/redeem/withdraw）===")
money = [p for p in untouched
         if re.search(r"spark|buy|pay|donat|loan|gift|tip|redeem|withdraw|"
                      r"deposit|bank|refund|topup|order", p, re.I)]
for p in money:
    print("  %-44s %s" % (p, sorted(routes[p])[0].split("/")[-1]))
