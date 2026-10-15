# -*- coding: utf-8 -*-
"""闸门 CI（第二半）：代码 SQL 引用的列必须真实存在（P-342 的直接判据）。

第一半 `pt_ci_migration_columns_in_db.py` 查的是「迁移声明 vs 库」，能抓到
迁移写了库里没有的列；但 P-342 的形状正相反——**代码引用了库/迁移里根本没有
的列**（`medals.active`）。那一半在这里。

做法：对每条 Rust SQL 字符串，从 `FROM/JOIN/UPDATE/INTO <表>` 认出表，
再把该行里形如 `x.y =` / `x.y IS` 的 `y` 取出来，逐个对运行库的列表核对。
表名认不出的、或者该行是 JOIN ON 条件（跨表）的一律放过——宁可漏也不误报，
因为误报会让人养成忽略红灯的习惯。
"""
import io
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def psql(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    return (r.stdout or "").strip()


cols = {}
for ln in psql("SELECT DISTINCT table_name, column_name "
               "FROM information_schema.columns "
               "WHERE table_schema='public'").splitlines():
    if "|" in ln:
        t, c = ln.split("|", 1)
        cols.setdefault(t.strip().lower(), set()).add(c.strip().lower())

KW = {"and", "or", "not", "null", "true", "false", "case", "when", "then",
      "else", "end", "select", "from", "where", "set", "values", "on", "as",
      "in", "is", "like", "ilike", "between", "exists", "distinct", "limit",
      "offset", "order", "by", "group", "having", "asc", "desc", "count",
      "sum", "avg", "max", "min", "coalesce", "extract", "interval", "now",
      "at", "time", "zone", "descending", "ascending", "for", "of", "if"}

TBL = re.compile(r"\b(?:FROM|JOIN|UPDATE|INTO)\s+(\w+)", re.I)
USE = re.compile(r"\b(\w+)\.(\w+)\s*(?:=|<>|!=|<|>|\s+IS\b|\s+LIKE\b|\s+IN\b)", re.I)

hit = []
for src in ("apps/api/src", "apps/worker/src"):
    for dp, _, fns in os.walk(os.path.join(ROOT, src)):
        for fn in fns:
            if not fn.endswith(".rs"):
                continue
            p = os.path.join(dp, fn)
            for i, ln in enumerate(
                    io.open(p, encoding="utf-8", errors="replace"), 1):
                code = ln.split("//", 1)[0]
                if "SELECT" not in code.upper() and "UPDATE" not in code.upper():
                    continue
                # 本行里出现的表（只认运行库里真实存在的）。
                tables = {m.group(1).lower() for m in TBL.finditer(code)}
                tables = {t for t in tables if t in cols}
                if not tables:
                    continue
                # 别名映射：SQL 里写的是 `m.active` 而不是
                # `medals.active`。不认别名就整行放过，闸门会假绿
                # （P-342 正是别名写法，自检时才发现）。
                alias = {}
                for m in re.finditer(
                        r"\b(?:FROM|JOIN)\s+(\w+)(?:\s+(?:AS\s+)?(\w+))?",
                        code, re.I):
                    tbl = m.group(1).lower()
                    al = (m.group(2) or tbl).lower()
                    if tbl not in cols:
                        continue
                    # 后一个词可能是 WHERE/ON/LEFT/JOIN 等关键字，
                    # 而不是别名（正则要认 `medals m`，反例要认
                    # `medals WHERE` 里的 WHERE 不是别名）。
                    if al in KW or al in ("left", "right", "inner", "outer",
                                          "cross", "full", "natural"):
                        al = tbl
                    alias[al] = tbl
                alias.update({t: t for t in tables})
                for m in USE.finditer(code):
                    a, c = m.group(1).lower(), m.group(2).lower()
                    tbl = alias.get(a)
                    if not tbl or c in KW:
                        continue
                    if c in cols[tbl]:
                        continue
                    if c.endswith(("_id", "_at", "_seq", "_key", "_path")):
                        continue     # 多张表合并的字段，认不准，放过
                    hit.append("%s:%d  %s.%s" % (
                        p.replace("\\", "/").split("apps/")[-1], i, tbl, c))

if hit:
    seen, uniq = set(), []
    for h in hit:
        key = h.split("  ")[-1]
        if key not in seen:
            seen.add(key)
            uniq.append(h)
    print("[FAIL] %d 处代码引用了不存在的列（去重后 %d 类）："
          % (len(hit), len(uniq)))
    for h in uniq[:20]:
        print("   " + h)
    print("   后果：运行时 column does not exist（P-342：三个自动成就 job 全废）。")
    raise SystemExit(1)
print("== ALL PASS（代码 SQL 引用的列均存在于运行库）==")
