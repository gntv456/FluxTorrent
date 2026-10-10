#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""兜底扫描：把 .rs 里字符串字面量内的 Rust 风格 `//` 注释揪出来。

Rust 的 `//` 只在**代码**里是注释；一旦落进 SQL 字符串，Postgres 把 `//`
读成整除运算符，整条语句 syntax error（2026-10-10 实测：种子编辑口
`PUT /torrents/{id}` 100% 500，注释被粘进了 r#"..."# 里）。
"""
import os
import re
import sys

SQL_WORDS = re.compile(r"\b(SELECT|UPDATE|INSERT|INTO|DELETE|WITH)\b", re.I)
URLISH = re.compile(r"(https?:|//\*)")


def bad(s):
    if not SQL_WORDS.search(s):
        return False
    for m in re.finditer(r"//", s):
        before = s[max(0, m.start() - 8):m.start()]
        after = s[m.end():m.end() + 2]
        if before.endswith(":") or before.endswith("/"):
            continue  # http:// // 协议前缀
        if after.startswith("*"):
            continue
        return True
    return False


def literals(text):
    """返回 (行号, 字符串内容)。

    口径按「零误报」设计，只认两类确实是 SQL 的字面量：
      · 原始串 r#"…"# —— sqlx 多行 SQL 的写法
      · 常规串且**首词是 SQL 关键字** —— "UPDATE …"/"SELECT …"
    其余一律不看。字符串配对正则会「从上一个串的收尾引号配到下一个串的
    开头条引号」，把中间正常的 Rust 注释误判成串内注释，首词口径正好躲开。
    """
    for m in re.finditer(r'r#"(.*?)"#', text, re.S):
        yield text[:m.start()].count("\n") + 1, m.group(1)
    for m in re.finditer(r'"((?:[^"\\]|\\.)*)"', text, re.S):
        body = m.group(1)
        if not re.match(r"\s*(SELECT|UPDATE|INSERT|DELETE|WITH)\b", body,
                        re.I):
            continue
        yield text[:m.start()].count("\n") + 1, body


def scan(root):
    hits = []
    for dirpath, dirs, files in os.walk(root):
        dirs[:] = [d for d in dirs if d not in ("target", "node_modules",
                                                ".git", ".next")]
        for fn in files:
            if not fn.endswith(".rs"):
                continue
            p = os.path.join(dirpath, fn)
            try:
                text = open(p, encoding="utf-8").read()
            except Exception:
                continue
            for line, body in literals(text):
                if not bad(body):
                    continue
                sn = [ln for ln in body.split("\n")
                      if "//" in ln and not URLISH.search(ln)]
                hits.append((p, line, (sn or [""])[0].strip()))
    return hits


if __name__ == "__main__":
    roots = sys.argv[1:] or ["apps"]
    seen = set()
    out = []
    for rt in roots:
        for p, line, sn in scan(rt):
            key = (p, line, sn)
            if key in seen:
                continue
            seen.add(key)
            out.append(key)
    for p, line, sn in sorted(out):
        print("FAIL %s:%d  %s" % (p, line, sn[:110]))
    print("== SQL 串内 // 注释：%d 处" % len(out))
    sys.exit(1 if out else 0)
