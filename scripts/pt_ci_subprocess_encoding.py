# -*- coding: utf-8 -*-
"""闸门 CI：Windows 下 subprocess 必须显式 encoding，否则中文输出直接把闸门打断。

批一百二十七 P-327 的复发（批一百五十四）：`pt_audit_s_anime.py` 与
`pt_audit_h_packswitch.py` 的 `subprocess.run(PG + [sql], text=True)` 没给
encoding——Windows 控制台默认 GBK，PG 返回中文时（季节名、分类名）在接收
线程里抛 UnicodeDecodeError，主线程只拿到空 stdout，随后 IndexError 把闸门打断。

危害不在红，在于**假跑完**：崩之前只打出 2 条 PASS，看着像「跑过了」，而后面
10~12 条判据根本没执行。全仓这类写法共 57 处（另 55 处属侥幸：查询恰好没返回
中文），一旦命中就静默少跑一整段账面。

判据：scripts/*.py 里所有带 text=True 的 subprocess.run 都必须同时带 encoding。
"""
import io
import os
import re
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

CALL = re.compile(r"subprocess\.run\((?:[^()]|\((?:[^()])*\))*\)", re.S)
bad = []
for fn in sorted(os.listdir("scripts")):
    if not fn.endswith(".py"):
        continue
    if fn == os.path.basename(__file__):
        continue   # 本脚本 docstring 里就写着那个反例，别把自己判红
    p = os.path.join("scripts", fn)
    s = io.open(p, encoding="utf-8", errors="replace").read()
    for m in CALL.finditer(s):
        c = m.group(0)
        if "text=True" in c and "encoding=" not in c:
            bad.append("%s:%d" % (fn, s[:m.start()].count("\n") + 1))

if bad:
    print("[FAIL] %d 处 subprocess.run(text=True) 缺 encoding：" % len(bad))
    for b in bad[:20]:
        print("   " + b)
    print("   后果：中文输出时抛 UnicodeDecodeError ⇒ stdout 空 ⇒ 闸门崩在主判据前，")
    print("        后面整段账面静默不跑（批一百二十七 P-327 / 本批 s_anime 实测）。")
    raise SystemExit(1)
print("== ALL PASS（scripts 下 subprocess 全部显式 encoding）==")
