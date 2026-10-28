#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""NB：`ON CONFLICT … DO UPDATE SET c = …` 里对自身列的引用必须带表名（P-465）。

现网取证（2026-10-28，部署后 3 分钟内）：`torrent_http/suggest.rs` 的热搜计数
```
INSERT INTO search_term_stats (term, hits) VALUES ($1, 1)
ON CONFLICT (term) DO UPDATE SET hits = hits + 1, last_at = now()
```
被 PostgreSQL 直接拒：`column reference "hits" is ambiguous`。也就是说这条写
**从来没有成功过一次**；0425 建表那次失败只是把它藏得更深——两头静默
（写侧 `let _ =`、读侧 `unwrap_or_default()`），热搜永久零累积而无人知道。
P-461 的「出声」改动部署后，日志里 3 分钟内就把真因喊出来了。

修法：`hits = search_term_stats.hits + 1`（或 `EXCLUDED.hits`，语义不同要小心）。
本闸门量的是形状：SET 目标列在表达式里出现时，必须写成 `表名.列` 或
`EXCLUDED.列`。

为什么既有门没拦住：`pt_ci_api_sql_prepares.py` 报「1437 条 SQL 无对象不存在」，
但它对 `DO UPDATE` 这条同样没报错——它的判据只认 `does not exist` 字样，
ambiguous 这一类不在它的词表里（同族教训：判据覆盖面就是洞的形状）。
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTS = ["apps/api/src", "apps/worker/src"]
DO = re.compile(r"DO\s+UPDATE\s+SET\s+([\s\S]{0,240}?)(?:;|\"|$)", re.I)
ASSIGN = re.compile(r"([a-z_][a-z0-9_]*)\s*=\s*(.+?)(?=,\s*[a-z_][a-z0-9_]*\s*=|$)",
                    re.I | re.S)
# 台账：文件 -> 理由（多一个少一个都红）
LEDGER = {}


def strip_comments(text):
    out = []
    for line in text.splitlines():
        i, quote = 0, False
        while i < len(line) - 1:
            c = line[i]
            if c == '"':
                quote = not quote
            elif not quote and ((c == "/" and line[i + 1] == "/")
                                or (c == "-" and line[i + 1] == "-")):
                line = line[:i]
                break
            i += 1
        out.append(line)
    return "\n".join(out)


def self_refs(body):
    """返回 [(列, 表达式)]：表达式里对 SET 目标列的无限定自引用。"""
    bad = []
    for m in DO.finditer(body):
        for col, expr in ASSIGN.findall(m.group(1)):
            c = col.lower()
            if not re.search(r"\b%s\b" % re.escape(c), expr, re.I):
                continue
            if re.search(r"[\w.]\.%s\b" % re.escape(c), expr, re.I):
                continue  # 表名.列 / EXCLUDED.列 都算合法限定
            bad.append((col, " ".join(expr.split())[:60]))
    return bad


files = {}
for d in ROOTS:
    if not os.path.isdir(d):
        continue
    for dp, _dn, fns in os.walk(d):
        for fn in sorted(fns):
            if fn.endswith(".rs"):
                p = os.path.join(dp, fn).replace(chr(92), "/")
                files[p] = strip_comments(
                    open(p, encoding="utf-8", errors="replace").read())

hits = {p: self_refs(b) for p, b in files.items() if DO.search(b)}
offenders = sorted(p for p, v in hits.items() if v)

GHOST_BAD = ('"INSERT INTO t (a, b) VALUES ($1,$2) '
             'ON CONFLICT (a) DO UPDATE SET b = b + 1"')
GHOST_OK = ('"INSERT INTO t (a, b) VALUES ($1,$2) '
            'ON CONFLICT (a) DO UPDATE SET b = t.b + 1"')
GHOST_EX = ('"INSERT INTO t (a, b) VALUES ($1,$2) '
            'ON CONFLICT (a) DO UPDATE SET b = EXCLUDED.b"')
ok("NB1 正控：合成件 `SET b = b + 1` 必须被判违例",
   self_refs(GHOST_BAD) == [("b", "b + 1")], "%s" % self_refs(GHOST_BAD))
ok("NB1b 正控（反向）：`SET b = t.b + 1` 与 `SET b = EXCLUDED.b` 都必须放行",
   not self_refs(GHOST_OK) and not self_refs(GHOST_EX), "")
ok("NB2 「DO UPDATE SET 里无限定自引用」违例 = 台账 %s" % sorted(LEDGER),
   set(offenders) == set(LEDGER),
   "实测=%s" % {p: v for p, v in hits.items() if v})
ok("NB3 反空转：扫到 DO UPDATE 的文件数须 >= 8（尺子瞎了会读成空集）",
   len(hits) >= 8, "文件=%d" % len(hits))
print("   [实况] 含 DO UPDATE 的文件 %d 个：%s"
      % (len(hits), sorted(hits)[:6]))
summary()
