# -*- coding: utf-8 -*-
"""EM 的姊妹普查：箭头式 `.catch(() => setX(空值))` 形状的**底数与判据自证**。

为什么单独一个脚本（而不是直接并进 pt_em_fail_as_empty_guard.py）：
  EM 那 6 条断言钉的是 `catch { … }` **块体**形状，已逐只定性、有 PENDING 台账；
  箭头式一跑就是十几只命中，里面既有真要修的（读列表失败演成「没有记录」），
  也有合法降级（辅助数据置 null、写操作里已有别的提示）。**候选数≠缺陷数**，
  在没有逐只对着渲染侧核完之前，把它们塞进 EM 的 PENDING 就等于拿台账冒充结论。
  所以这里只做两件事：① 判据自证（两个正控必须成立，否则这脚本没资格报数）；
  ② 打印可复跑的底数，供逐只吃掉时销账。

判据三种写法都盖住：
  catch { … }                       —— 老形状
  .catch(() => { … })               —— 箭头带块
  .catch(() => setX([]))            —— 箭头单表达式
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WEB = os.path.join(ROOT, "apps", "web")

CATCH = re.compile(
    r"catch\s*(?:\([^)]*\))?\s*\{([^{}]*)\}"        # 老形状
    r"|\.catch\(\s*\(\s*\)\s*=>\s*\{([^{}]*)\}"     # 箭头带块
    r"|\.catch\(\s*\(\s*\)\s*=>\s*([^{};]+)",       # 箭头单表达式
    re.S,
)
EMPTYSET = re.compile(r"set\w+\(\s*(?:\[\]|null|\{\})\s*\)")
SIGNAL = re.compile(
    r"toast|flash|setErr|setMsg|set\w*Failed|set\w*Error|throw|console", re.I,
)


def strip(text):
    out = []
    for line in text.splitlines():
        s = line.strip()
        if s.startswith(("//", "*", "/*")):
            continue
        i = line.find("//")
        if i >= 0 and '"' not in line[:i] and "'" not in line[:i]:
            line = line[:i]
        out.append(line)
    return "\n".join(out)


def bare(text):
    hits = []
    for m in CATCH.finditer(text):
        body = m.group(1) or m.group(2) or m.group(3) or ""
        if EMPTYSET.search(body) and not SIGNAL.search(body):
            hits.append(" ".join(body.split())[:46])
    return hits


def read(path):
    if not os.path.exists(path):
        return ""
    return io.open(path, encoding="utf-8", errors="replace").read()


print("== S1 判据自证（两个正控，缺一个这脚本就没资格报数）==")
# 正控用**钉住的历史内容**（6cdc3f1a 时该页仍是 setRows([])）：
# 拿工作树当前文件当正控，会在这一只被修好后自己把正控跑没（本批就发生了）。
pin = subprocess.run(
    ["git", "show",
     "6cdc3f1a:apps/web/app/(main)/me/exams/page.tsx"],
    cwd=ROOT, capture_output=True, text=True, encoding="utf-8",
    errors="replace")
hits1 = bare(strip(pin.stdout)) if pin.returncode == 0 else []
ok("ARROW1 历史内容（6cdc3f1a 的 me/exams：箭头式 setRows([])）必须被认出",
   bool(hits1), str(hits1[:1]) if pin.returncode == 0
   else "取不到钉住的历史内容 rc=%s" % pin.returncode)
old = subprocess.run(
    ["git", "show", "872fafc1^:apps/web/components/staff-wiki.tsx"],
    cwd=ROOT, capture_output=True, text=True, encoding="utf-8",
    errors="replace")
if old.returncode != 0:
    print("   WARN 取不到 872fafc1^ 的 staff-wiki（不在本仓？），"
          "第二个正控未跑成")
    ok("ARROW2 取到了修复前的 staff-wiki 内容", False,
       "rc=%s %s" % (old.returncode, old.stderr[:80]))
else:
    hits = bare(strip(old.stdout))
    ok("ARROW2 修复前的 staff-wiki（箭头式 setRows([]) / setRevs([])）必须被认出",
       len(hits) >= 2, str(hits[:2]))

print("== S2 现网底数（**候选**，未逐只定性，不并入 EM 的 PENDING）==")
found = {}
for dp, dirs, fs in os.walk(WEB):
    dirs[:] = [d for d in dirs
               if d not in (".next", "node_modules", "e2e", "tests")]
    for f in fs:
        if not f.endswith(".tsx"):
            continue
        fp = os.path.join(dp, f)
        h = bare(strip(read(fp)))
        if h:
            found[os.path.relpath(fp, WEB).replace("\\", "/")] = h
print("   箭头式吞错命中的组件/页面数 = %d（EM 主闸门台账之外，逐个核渲染侧才升账）"
      % len(found))
for k in sorted(found):
    print("     %-58s %s" % (k, found[k][0]))
summary()
