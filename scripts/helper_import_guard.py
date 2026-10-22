# -*- coding: utf-8 -*-
"""常驻护栏：被闸门 import 的助手模块必须**能 import 成功**（P-557 一族）。

为什么要这条：`scripts/pt_audit_form.py` 我写了 `time.time()` 却没 `import time`
（00f2abd6），于是 `from pt_audit_form import …` 的三支闸门（af_search/ag_rss/
ah_hr）在 import 阶段就 NameError——全量回归汇总里它们显示成「一支红闸门」，
实际上一条断言都没跑过。`py_compile` 抓不到这类错（语法没错），只有真的
import 一次才暴露。

口径：
  * 助手 = 被别的脚本 `import`/`from … import` 且在 scripts/ 下有同名 .py 的模块；
    闸门本体（自己会打接口/连库的）不算，它们要靠各自的活体判据。
  * 每个助手单独起进程 import（一个崩了不影响其余，也不许顺带执行闸门 main）。
  * 正控：往键集里塞一个「明知 import 会崩」的合成模块，护栏必须报它。

接线状态：本脚本可直接 `python3 scripts/helper_import_guard.py`（exit 1 = 违规）。
`scripts/ci_local.sh` 现在在站方排版中（工作树已改），等那批落地再把它挂进链条。
用法：python scripts/helper_import_guard.py
"""
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
S = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                 "scripts")
IMPORT_RE = re.compile(r"^(?:from|import)\s+([a-z_][a-z0-9_]*)\b", re.M)


def helpers():
    names = set()
    for fn in sorted(os.listdir(S)):
        if not fn.endswith(".py"):
            continue
        with open(os.path.join(S, fn), encoding="utf-8", errors="replace") as f:
            text = f.read()
        for m in IMPORT_RE.findall(text):
            if os.path.exists(os.path.join(S, m + ".py")):
                names.add(m)
    return names


def try_import(mod):
    return subprocess.run([sys.executable, "-c",
                           "import sys; sys.path.insert(0, r'%s'); import %s"
                           % (S, mod)],
                          capture_output=True, text=True, encoding="utf-8",
                          errors="replace", timeout=120)


def main():
    mods = helpers()
    print("助手模块 %d 个：%s" % (len(mods), ", ".join(sorted(mods))))
    bad = []
    for m in sorted(mods):
        try:
            r = try_import(m)
        except subprocess.TimeoutExpired:
            bad.append((m, "import 卡住 >120s"))
            continue
        if r.returncode != 0:
            tail = (r.stderr or "").strip().split("\n")[-1]
            bad.append((m, tail[:160]))
    for m, why in bad:
        print("   [FAIL] %-24s %s" % (m, why))
    # 正控：合成一个必崩的助手，护栏必须抓得到
    probe = "_tmp_guard_probe_mod.py"
    ppath = os.path.join(S, probe)
    with open(ppath, "w", encoding="utf-8") as f:
        f.write("X = NO_SUCH_NAME_AT_ALL\n")   # 语法过、import 必炸（P-557 同形）
    try:
        r = try_import("_tmp_guard_probe_mod")
        caught = r.returncode != 0
    finally:
        os.remove(ppath)
    print("正控：合成必崩模块被 import 抓到 = %s" % caught)
    if bad or not caught:
        print("护栏失效要 raise：%d 个助手 import 崩；正控 %s"
              % (len(bad), "未抓到" if not caught else "已抓到"))
        sys.exit(1)
    print("== ALL PASS（助手全部可 import，正控承重）")


if __name__ == "__main__":
    main()
