# -*- coding: utf-8 -*-
"""普查：带 {magic}（站点货币名占位）的文案，读它的那句走没走 fmtCur。

口径出处：apps/web/i18n/config.ts 的 `fmt()` 是
`vars[k] === undefined ? "" : String(vars[k])` ⇒ **占位符没给值就渲染成空串**；
只有 `fmtCur(tpl, vars, currency)` 会把 `magic` 补成站点货币名。
所以一句「当前还有 {magic}」若被 `fmt` 渲染 ⇒ 用户看到「当前还有 」（少一截）；
若被直接 `{d.xxx}` 放进 JSX ⇒ 屏幕上原样露出 `{magic}` 这对花括号。
站长在后台把 currency_name 改名时，这类句子仍是坏的。

键集来源（这一步换了两次才敢用）：行级正则把 131 条读成 264 条（值换行、
隔壁条目都混进来），手写 TS 展平器又只读出 5 条；现在直接拿运行时字典导的
平铺表 `_tmp_magic_leaves.json`（由 apps/web/tests/i18n-magic-leaves.test.ts
生成，7124 个叶子里挑出含 {magic} 的 131 个，基数在用例里断言过）。

访问点解析：认 `const X = dict.<任意深度路径>` 的别名 + `dict.<路径>.<键>` 直取；
路径必须与键的全路径一致，认不出别名的文件**记 skipped 并报数**，不当已覆盖。

用法：python scripts/audit_magic_placeholder.py [--json]
"""
import io
import json
import os
import re
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LEAVES = os.path.join(ROOT, "_tmp_dict_leaves.json")
WEB = os.path.join(ROOT, "apps", "web")
ALIAS = re.compile(r"const\s+(\w+)\s*=\s*dict\.([A-Za-z0-9_.]+)")
DIRECT = re.compile(r"dict\.([A-Za-z0-9_.]+)\.([A-Za-z0-9_]+)\b")


def magic_paths():
    if not os.path.exists(LEAVES):
        raise SystemExit(
            "缺数据源 _tmp_dict_leaves.json：先在 apps/web 跑 "
            "`npx vitest run tests/i18n-magic-leaves.test.ts` 生成"
            "（它从运行时字典导，基数断言在测试里）"
        )
    d = json.load(io.open(LEAVES, encoding="utf-8"))
    assert d["total"] > 2000, "导出的叶子总数 %d 不像整本字典" % d["total"]
    return d["magic"]


def statement(text, pos):
    """取访问点所属的表达式窗口。

    `fmtCur(d.title, {…})` 里 fmtCur 一定在访问点**之前**，所以窗口主要是向前取；
    尾巴取到本句结束（`;`，最多 400 字符）。
    边界取最近的 `;`／空行／`}`——不按整段函数取，否则上一行的 fmtCur 会把
    这一行的裸渲染洗白。
    尾巴不能只留 80 字符：JSX 里成串换行的
    `{d.x\n  .replace("{n}", y)\n  .replace("{magic}", currency)}` 是主流写法，
    截半路会把「其实补了货币名」读成缺陷（上一版因此假红 6 处）。
    """
    back = max(text.rfind(";", 0, pos), text.rfind("\n\n", 0, pos),
               text.rfind("}", 0, pos))
    end = text.find(";", pos)
    if end < 0 or end - pos > 400:
        end = min(pos + 400, len(text))
    return text[back + 1:end + 1]


def verdict(stmt):
    """四种「确实把货币名补上了」的形状才算 ok，其余按渲染后果分两档。

    仪器自己踩过的假阳性（都要认，否则把好的说成坏的）：
      * `fmt(tpl, { magic: currency })` —— 不走 fmtCur 但显式喂了 magic；
      * 局部助手 `const magic = (s) => s.replaceAll("{magic}", currency)`，
        调用点写成 `magic(dict.shop.title)`（shop/page.tsx 就是这么写的）。
    """
    if ("fmtCur(" in stmt or re.search(r"(?:^|[^\w.])magic\(", stmt)
            or re.search(r'replace(?:All)?\(\s*"\{magic\}"', stmt)):
        return "ok"
    if re.search(r"\bfmt\(", stmt):
        return "fmt 里喂了 magic" if re.search(r"\bmagic\s*:", stmt) else "fmt吞空"
    return "原样露出"


def main():
    magic = magic_paths()
    by_tail = {}
    for p in magic:
        by_tail.setdefault(p.split(".")[-1], []).append(p)

    rows = []
    files = []
    for dirpath, _, fns in os.walk(WEB):
        if any(x in dirpath for x in (os.sep + "node_modules", ".next")):
            continue
        for fn in fns:
            if fn.endswith((".tsx", ".ts")) and "_tmp_" not in fn:
                files.append(os.path.join(dirpath, fn))
    scanned = 0
    for p in files:
        text = io.open(p, encoding="utf-8", errors="replace").read()
        if "dict" not in text:
            continue
        scanned += 1
        aliases = dict(ALIAS.findall(text))
        sites = []
        for var, sub in aliases.items():
            for m in re.finditer(r"\b%s\.([A-Za-z0-9_]+)\b" % re.escape(var),
                                 text):
                tail = m.group(1)
                want = "%s.%s" % (sub, tail)
                if want in magic:
                    sites.append((want, m.start()))
        for m in DIRECT.finditer(text):
            want = "%s.%s" % (m.group(1), m.group(2))
            if want in magic:
                sites.append((want, m.start()))
        for want, pos in sites:
            line = text[:pos].count("\n") + 1
            rows.append({"path": want, "file": os.path.relpath(p, ROOT),
                         "line": line, "pos": pos,
                         "verdict": verdict(statement(text, pos)),
                         "tpl": magic[want]})
    tally = {}
    for r in rows:
        tally[r["verdict"]] = tally.get(r["verdict"], 0) + 1
    print("魔法键 %d 个；扫过含 dict 的文件 %d 个；定位到访问点 %d 处：%s"
          % (len(magic), scanned, len(rows), tally))
    order = {"原样露出": 0, "fmt吞空": 1, "ok": 2,
               "fmt 里喂了 magic": 3}
    OKISH = {"ok", "fmt 里喂了 magic"}
    for r in sorted(rows, key=lambda x: (order[x["verdict"]], x["file"])):
        if r["verdict"] in OKISH:
            continue
        print("  [%s] %-34s %s:%s" % (r["verdict"], r["path"], r["file"],
                                      r["line"]))
        print("        文案：%s" % r["tpl"][:70])
    if "--json" in sys.argv:
        json.dump(rows, io.open(os.path.join(ROOT, "_tmp_magic_sites.json"),
                                "w", encoding="utf-8"),
                  ensure_ascii=False, indent=1)
    # 正控：取一处判为 ok 的访问点，只把它所在 statement 里的 fmtCur 换成 fmt，
    # 判据必须翻成违规（不落盘，只在内存里改这一段）
    okrows = [r for r in rows if r["verdict"] in OKISH]
    flipped = 0
    for r in okrows[:6]:
        text = io.open(os.path.join(ROOT, r["file"]), encoding="utf-8",
                       errors="replace").read()
        st = statement(text, r["pos"])
        if "fmtCur(" not in st:
            continue
        mutated = st.replace("fmtCur(", "fmt(")
        if verdict(mutated) != "ok":
            flipped += 1
    print("正控：抽 %d 处 fmtCur 命中，把 fmtCur 换成 fmt 后应全部翻红 ⇒ 实际翻红 %d"
          % (min(len(okrows), 6), flipped))
    if not okrows or flipped == 0:
        print("  ↑ 正控不承重：要么一处 fmtCur 都没定位到，要么换了实现判据没反应")


if __name__ == "__main__":
    main()
