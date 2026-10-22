# -*- coding: utf-8 -*-
"""普查：把字典子树强转成 Record<string,string> 的地方，键在字典里存不存在。

为什么要管：`dict.admin as unknown as Record<string, string>` 这类强转**关掉了
tsc 的键检查**（字典本身是 `Dict` 类型，缺键编译不过；一转成 Record 就随便点）。
于是 `a.someKey` 拼错/漏登记时不报错，JSX 里渲染成空白——职员看到的不是英文键，
是**什么都没有**（比 P-556 那次更静）。

判据口径：
  * 只认「同一文件里 `const X = dict.<子树> as unknown as Record<…>`」这种直转，
    X 之后的 `X.<键>` 全算访问点；`X.length` 之类原型成员剔掉。
  * 键集从 zh-CN.ts 的对应子树里按**花括号配平**取深度 1 的字符串叶子
    （不是整本字典——整本会把别的段的同名键算成存在）。
  * 正控：拿一条**正在被点**的键假装它不在字典里，普查必须立刻报出它的访问点；
    报不出来就是尺子空转。

用法：python scripts/audit_dict_cast_keys.py
"""
import io
import os
import re
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ZH = os.path.join(ROOT, "apps", "web", "i18n", "zh-CN.ts")
WEB = os.path.join(ROOT, "apps", "web")
CAST = re.compile(r"const\s+(\w+)\s*=\s*dict\.(\w+)\s+as unknown as "
                  r"Record<string,\s*string>")
PROTO = {"length", "constructor", "toString", "hasOwnProperty", "valueOf",
         "includes", "map", "filter", "slice", "split", "trim", "replace",
         "charAt", "concat", "forEach", "join", "keys", "entries", "push",
         "some", "find", "sort", "padStart", "padEnd", "startsWith",
         "endsWith", "toLowerCase", "toUpperCase", "match", "repeat"}


def subtree_keys(src, name):
    """取顶层 `name: { … }` 块里深度 1 的**叶子**键。

    两版写法都踩过：
      * 按「行首 4 空格 + key: "」取键 ⇒ `key:` 值换行写的（zh-CN 的
        cheatersNote 就是）和嵌套对象（navSubs）都会算错，前者假红、
        后者假红（对象本来就不是字符串）。
      * 所以这里改成字符级扫描：深度 1 处遇到 `key:` 就向后跳过空白看
        第一个实字符——是 `{` 记成「嵌套（不算叶子）」，否则记成叶子键。
    """
    m = re.search(r"^  %s: \{$" % re.escape(name), src, re.M)
    if not m:
        return None, None
    i = m.end() - 1          # 指向子树的 `{`
    depth = 0
    start = i
    end = i
    while end < len(src):
        ch = src[end]
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                break
        elif ch in "\"'`":
            q = ch
            end += 1
            while end < len(src) and src[end] != q:
                end += 1
        end += 1
    body = src[start + 1:end - 1]
    leaves, nested = set(), set()
    for km in re.finditer(r'^(\s{4})(\w+|"[^"]+")\s*:\s*', body, re.M):
        # 只认深度 1 的键：前面的缩进恰好 4 空格（深度 2 的键缩进 6）
        after = body[km.end():]
        nxt = after.lstrip(" \t\r\n")
        key = km.group(2).strip('"')
        first = nxt[0] if nxt else ""
        if first == "{":
            nested.add(key)
        elif first in "\"'`[":
            leaves.add(key)
        else:
            leaves.add(key)      # 值换行/模板串/函数调用都算叶子
    return leaves, nested


def collect():
    """扫全仓直转口，返回 (访问点, 子树→(叶子, 嵌套))。"""
    src = io.open(ZH, encoding="utf-8").read()
    files = []
    for dirpath, _, fns in os.walk(WEB):
        if any(x in dirpath for x in (os.sep + "node_modules", ".next")):
            continue
        for fn in fns:
            if fn.endswith((".tsx", ".ts")):
                files.append(os.path.join(dirpath, fn))
    warns = []
    accesses = []
    scanned = 0
    keysets = {}
    for p in files:
        text = io.open(p, encoding="utf-8", errors="replace").read()
        casters = CAST.findall(text)
        if not casters:
            continue
        scanned += 1
        for var, sub in set(casters):
            if sub not in keysets:
                keysets[sub] = subtree_keys(src, sub)
            leaves, nested = keysets[sub]
            if leaves is None:
                warns.append((sub, os.path.relpath(p, ROOT)))
                continue
            for m in re.finditer(r"\b%s\.(\w+)" % re.escape(var), text):
                k = m.group(1)
                if k in PROTO or k in (nested or ()):
                    continue
                line = text[:m.start()].count("\n") + 1
                accesses.append((sub, k, os.path.relpath(p, ROOT), line))
    return accesses, keysets, scanned, warns


def main():
    accesses, keysets, scanned, warns = collect()

    def missing(extra=()):
        drop = set(extra)
        return [a for a in accesses
                if a[1] not in (keysets.get(a[0]) or ({}, set()))[0]
                or a[1] in drop]

    miss = missing()
    print("直转文件 %d 个；子树叶子/嵌套 %s；访问点 %d 处"
          % (scanned, {k: (len(v[0]), len(v[1])) if v[0] is not None else None
                       for k, v in keysets.items()}, len(accesses)))
    print("强转口点到、字典里没有的键 = %d 处" % len(miss))
    seen = set()
    for sub, k, rel, line in miss:
        tag = "" if (sub, k) not in seen else "     同上"
        seen.add((sub, k))
        print("  [%s] %-26s %s:%s" % (sub, k, rel, line) + tag)
    for sub, rel in warns:
        print("  仪器告警：子树 %s 的键集没取到（%s）" % (sub, rel))
    used = sorted({a[1] for a in accesses
                   if a[1] in (keysets.get(a[0]) or ({}, set()))[0]})
    if used:
        probe = used[0]
        n = len([a for a in missing(extra=[probe]) if a[1] == probe])
        print("正控：把在用的 %s 当成缺失 ⇒ 应报出它的 %d 处访问点（实际 %d）"
              % (probe, n or 0, n))
    else:
        print("正控：一个「正在被点且已登记」的键都没抓到 ⇒ 尺子在空转")


if __name__ == "__main__":
    main()
