# -*- coding: utf-8 -*-
"""普查：字典文案里的 `{占位}` 有没有被调用方喂上值（P-558 同族推广）。

`fmt()` 的实现是 `vars[k] === undefined ? "" : String(vars[k])`
（apps/web/i18n/config.ts）⇒ **模板里每个 `{x}` 都要在实参里有 x**，否则那句
话里的数字/名称就地消失，不报错。两种坏形：
  A. `fmt(d.xxx, { n: … })` 少了某个占位键 ⇒ 渲染成空（句子少一截）；
  B. 裸渲染 + 只补了部分占位的 `.replace("{n}", …)` 链 ⇒ 剩下的原样露出。

数据源：`_tmp_dict_leaves.json`（apps/web/tests/i18n-magic-leaves.test.ts 从
运行时字典导的 7124 个叶子）。末级键重名的**不猜**，另列计数。

判据自己踩过的坑（写在代码里防回归）：
  * 找 enclosing call 必须**往回看**——`fmt(a.x, {…})` 里 `fmt(` 在访问点之前；
    第一版只往后找 120 字符，把 200+ 处正常调用读成「裸渲染」。
  * 往回找也要限定在同一 statement 内（上一个 `;`／`}` 之后），否则会认领
    上一行的 fmt。
  * 实参是变量（`fmt(tpl, vars)`）时静态看不出键集 ⇒ 记 unknown，不判缺。

正控：合成样本喂全 ⇒ 不报；删一枚键 ⇒ 只报那一枚。

用法：python scripts/audit_placeholder_fill.py
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
PLACE = re.compile(r"\{(\w+)\}")
# 文档性花括号：`种子页地址形如 /torrent/{id}`、`访问地址为 /wiki/{slug}` 里的
# {id}/{slug} 是**写给用户看的示例路径**，本来就该原样渲染。第一版把这类算成
# 缺值，24 处候选里一大半是它（common.notFoundDesc、adminWiki.hint 等）。
DOC_BRACE = re.compile(r"/\{(\w+)\}")


def load():
    if not os.path.exists(LEAVES):
        raise SystemExit("缺 _tmp_dict_leaves.json：先跑 apps/web 的 "
                         "`npx vitest run tests/i18n-magic-leaves.test.ts`")
    d = json.load(io.open(LEAVES, encoding="utf-8"))
    assert d["total"] > 2000, "叶子总数 %d 不像整本字典" % d["total"]
    return d["flat"]


def call_args(text, open_paren):
    """按顶层逗号切出实参列表（配平括号，忽略引号内的）。"""
    depth, i = 0, open_paren
    parts, start, q = [], open_paren + 1, None
    while i < len(text):
        ch = text[i]
        if q:
            if ch == "\\":
                i += 2
                continue
            if ch == q:
                q = None
        elif ch in "\"'`":
            q = ch
        elif ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
            if depth == 0:
                break
        elif ch == "," and depth == 1:
            parts.append(text[start:i])
            start = i + 1
        i += 1
    parts.append(text[start:i])
    return parts


def obj_keys(src):
    """对象字面量的顶层键。`...spread` / 计算键认不出 ⇒ 不进键集（宁缺勿滥）。

    KEYIN 曾经写成 `(?:^|[{,]\\s*)…`，`^` 后面不容空格 ⇒ `{ n: x }` 里首键
    永远匹配不上，正控当场报「不承重」（271 处假红全是这么来的）。
    现在按顶层逗号切段，逐段判 `^\\s*key\\s*:`。
    """
    s = src.strip()
    if not s.startswith("{"):
        return None
    body = s[1:s.rfind("}")]
    keys, depth, cur = [], 0, ""
    for ch in body:
        if ch in "{[(":
            depth += 1
        elif ch in "}])":
            depth -= 1
        if ch == "," and depth == 0:
            keys.append(cur)
            cur = ""
        else:
            cur += ch
    keys.append(cur)
    out = set()
    for piece in keys:
        p = piece.strip()
        if not p:
            continue
        if p.startswith("..."):
            return None                   # spread：键集静态看不出 ⇒ 交 unknown
        m = re.match(r'^"?([A-Za-z_$][\w$]*)"?\s*:', p)
        if m:
            out.add(m.group(1))
            continue
        # 简写属性 `fmt(tpl, { n })` 同样是喂了 n —— 第一版只认 `key:`，
        # 于是 18 处正常调用被报成缺值（正控只测了全形，没测简写）。
        if re.match(r"^[A-Za-z_$][\w$]*$", p):
            out.add(p)
    return out


def enclosing_fmt(text, pos):
    """访问点是否正处在某个 fmt/fmtCur 的**第一个实参**里。"""
    start = max(text.rfind(";", 0, pos), text.rfind("}", 0, pos),
                text.rfind("\n\n", 0, pos))
    prefix = text[start:pos]
    # `fmt(` 与 `fmtCur(` 是两个不同的 token（后者里 fmt 后面跟的是 C，
    # 不会被 rfind("fmt(") 命中），取离访问点最近的那个。
    spots = [(prefix.rfind(k + "("), k) for k in ("fmt", "fmtCur")]
    spots = [(i, k) for i, k in spots if i >= 0]
    if not spots:
        # 根本没有 fmt 调用：这是「裸渲染」档，不是「判不了」
        return None, set(), None
    rel, kind = max(spots)
    op = start + rel + len(kind)
    args = call_args(text, op)
    # 访问点必须落在第 0 个实参范围内
    a0_end = op + 1 + len(args[0]) if args else op
    if not (op < pos <= a0_end):
        # 找到 fmt( 但我们的访问点不在它的第一个实参里 ⇒ 与本句无关
        return None, set(), None
    return kind, obj_keys(args[1]) if len(args) > 1 else set(), args


def main():
    flat = load()
    tails = {}
    for p, v in flat.items():
        if "{" in v:
            tails.setdefault(p.split(".")[-1], []).append(p)
    uniq = {k: v[0] for k, v in tails.items() if len(v) == 1}
    dup = len(tails) - len(uniq)

    files = []
    for dirpath, _, fns in os.walk(WEB):
        if any(x in dirpath for x in (os.sep + "node_modules", ".next")):
            continue
        for fn in fns:
            if fn.endswith((".tsx", ".ts")) and "i18n/" not in fn.replace(
                    os.sep, "/"):
                files.append(os.path.join(dirpath, fn))

    rows = []
    unknown = 0
    handoff = 0
    for p in files:
        text = io.open(p, encoding="utf-8", errors="replace").read()
        if "dict" not in text:
            continue
        rel = os.path.relpath(p, ROOT)
        aliases = dict(ALIAS.findall(text))
        cands = []
        for var, sub in aliases.items():
            for m in re.finditer(r"\b%s\.([A-Za-z0-9_]+)\b" % re.escape(var),
                                 text):
                path = "%s.%s" % (sub, m.group(1))
                if path in flat and "{" in flat[path]:
                    cands.append((path, m.start()))
        for m in re.finditer(r"\bdict\.([A-Za-z0-9_.]+\.[A-Za-z0-9_]+)\b",
                             text):
            if m.group(1) in flat and "{" in flat[m.group(1)]:
                cands.append((m.group(1), m.start()))
        for path, pos in cands:
            tpl = flat[path]
            need = set(PLACE.findall(tpl)) - set(DOC_BRACE.findall(tpl))
            if not need:
                continue          # 纯文档性花括号，本来就该原样显示
            kind, given_raw, _ = enclosing_fmt(text, pos)
            # 被别的局部助手包了一层（shop/page.tsx 的 `magic(s)`、或任何
            # `wrap(d.x)`）：静态看不出它补了什么 ⇒ 记 unknown，不判缺。
            before = text[max(0, pos - 40):pos]
            if kind is None and re.search(r"[A-Za-z_$][\w$]*\(\s*$", before):
                unknown += 1
                continue
            # 交给别处插值：`{ b: t.bodyReviewApproved }`、`savedMsg: s.x`
            # 这种「登记进对象再传给别的模块 fmt」的形状，本行不渲染，
            # 缺值与否要到真正调用那里才判得出来（message-center-table、
            # settings-client→settings-save 两处假阳性就是这么来的）。
            if kind is None and re.search(r"[A-Za-z_$][\w$]*\s*:\s*$", before):
                handoff += 1
                continue
            rep = set(re.findall(r'replace(?:All)?\(\s*"\{(\w+)\}"',
                                 text[pos:pos + 600]))
            shape = kind or ("链式补值" if rep else "裸渲染")
            if given_raw is None:
                unknown += 1
                continue            # 实参是变量/spread：静态判不了，不硬猜
            given = set(given_raw) | rep
            if kind == "fmtCur" or "magic" in given:
                need.discard("magic")
            miss = sorted(need - given)
            if miss:
                rows.append({"path": path, "file": rel,
                             "line": text[:pos].count("\n") + 1,
                             "need": sorted(need), "shape": shape,
                             "given": sorted(given), "miss": miss})
    print("带占位的文案 %d 条（末级键重名不猜 %d）；判不了 %d 处、交给别处插值 %d 处；"
          "判出缺值 %d 处" % (len(tails), dup, unknown, handoff, len(rows)))
    tally = {}
    for r in rows:
        tally[r["shape"]] = tally.get(r["shape"], 0) + 1
    print("  按形状分：%s" % tally)
    for r in sorted(rows, key=lambda x: (x["shape"], x["file"]))[:20]:
        print("  [%s] %-30s %s:%s" % (r["shape"], r["path"],
                                      r["file"].split(os.sep)[-1], r["line"]))
        print("       模板要 %s｜给了 %s｜缺 %s"
              % (r["need"], r["given"], r["miss"]))

    synth = 'const a = dict.probe; fmt(a.tpl, { n: x, pct: y });'
    pos = synth.index("a.tpl")
    kind, given, _ = enclosing_fmt(synth, pos)
    ok_full = kind == "fmt" and given == {"n", "pct"}
    one = 'const a = dict.probe; fmt(a.tpl, { pct: y });'
    k2, g2, _ = enclosing_fmt(one, one.index("a.tpl"))
    only_n = g2 == {"pct"}
    # 简写属性也得认——第一版漏在这里，把 18 处正常调用报成缺值
    short = 'const a = dict.probe; fmt(a.tpl, { n, pct: y });'
    k3, g3, _ = enclosing_fmt(short, short.index("a.tpl"))
    ok_short = g3 == {"n", "pct"}
    spread = 'const a = dict.probe; fmt(a.tpl, { ...v });'
    k4, g4, _ = enclosing_fmt(spread, spread.index("a.tpl"))
    print("正控：全形=%s 删一枚只报 n=%s 简写=%s spread=%s（None＝键集不可判）"
          % (ok_full, only_n, ok_short, g4))
    if not (ok_full and only_n and ok_short):
        print("  ↑ 正控不承重：enclosing_fmt/obj_keys 的形状判据要重做")


if __name__ == "__main__":
    main()
