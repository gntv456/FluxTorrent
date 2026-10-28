#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""NC：全仓 SQL 字面量必须能被 PostgreSQL **真正计划**（PREPARE 级）。

为什么要有这条（2026-10-28 现网取证）：
1) `pt_ci_api_sql_prepares.py` 报「api SQL 1437 条，无对象不存在」，可
   `/contracts/*` 查的表当时根本不存在（P-461）、`suggest.rs` 的
   `ON CONFLICT … SET hits = hits + 1` 是 `column reference "hits" is ambiguous`
   （P-465，这条写从来没成功过一次）。两支洞都在它眼皮底下。
2) 根因在机制：它把每条 SQL 塞进 `DO $$ … EXECUTE '<sql>' … $$` 里跑，
   而 **plpgsql 的 EXECUTE 对任何带 `$n` 的语句恒报
   `there is no parameter $1`**（实测），它的失败过滤器只留 `does not exist`
   ⇒ 凡含 `$`（它自己的入选门槛就是「必须含 `$`」）全部被当成「不是真错」丢掉。
   即：**这支门的绿是空的**。
3) 正解是 `PREPARE`：参数类型由上下文推断，规划期就做表/列/类型/歧义全套检查，
   且 PREPARE 只计划不执行 —— 跑在现网库上也没有副作用。

判据分三档：真错（ambiguous / column / operator / cast / group）必须为 0；
`does not exist` 仍归 MZ/既有门（这里只打印）；语法类多半是 format! 片段，
只打印不判（但会进「账面外」清单，便于人工看）。
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTS = ["apps/api/src", "apps/worker/src"]
LIT = re.compile(r'r#"(.*?)"#|"((?:[^"\\]|\\.)*)"', re.S)
HEAD = re.compile(r"^\s*(SELECT|WITH|INSERT|UPDATE|DELETE)\b", re.I)
# 真错台账：语句指纹 -> 定性（多一个少一个都红）
HARD_LEDGER = {}
# 合成正控：必须被同一支管线抓到
GHOST = ("INSERT INTO search_term_stats (term, hits) VALUES ('nc_ghost', 1) "
         "ON CONFLICT (term) DO UPDATE SET hits = hits + 1")


def norm(s):
    """Rust 续行（行尾 \\）归一 + 压空白 + 去掉引号转义。"""
    lines = s.replace('\\"', '"').split("\n")
    out, acc = [], ""
    for ln in lines:
        if ln.rstrip().endswith("\\"):
            acc += ln.rstrip()[:-1] + " "
            continue
        acc += ln
        out.append(acc)
        acc = ""
    if acc:
        out.append(acc)
    return " ".join(" ".join(out).split())


def collect():
    stmts = []
    for d in ROOTS:
        if not os.path.isdir(d):
            continue
        for dp, _dn, fns in os.walk(d):
            for fn in sorted(fns):
                if not fn.endswith(".rs") or fn.endswith("_tests.rs"):
                    continue
                p = os.path.join(dp, fn).replace(chr(92), "/")
                if "/tests" in p:
                    continue
                txt = open(p, encoding="utf-8", errors="replace").read()
                for m in LIT.finditer(txt):
                    raw = m.group(1) if m.group(1) is not None \
                        else m.group(2)
                    if not raw or not HEAD.match(raw):
                        continue
                    s = norm(raw)
                    if len(s) < 30 or "{" in s or "}" in s:
                        continue
                    stmts.append((s, p))
    return stmts


PSQL = ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
        "-q", "-d", "fluxtorrent"]


def _run(body):
    return subprocess.run(PSQL + ["-f", "-"], input=body,
                          capture_output=True, text=True, encoding="utf-8",
                          errors="replace", timeout=900).stderr or ""


def _err_of(stmts, i):
    """单条复核：整条语句自己跑，stderr 里第一条 ERROR 就是它的。"""
    out = _run("PREPARE nc_%d AS %s;" % (i, stmts[i][0]) + chr(10))
    for ln in out.splitlines():
        if "ERROR:" in ln:   # psql 会加 "psql:<stdin>:N:" 前缀
            return ln.strip()
    return ""


def probe(stmts):
    """分批 PREPARE；只有出错的批次才逐条复核——psql 会把 LINE 前缀截断
    （实测 `...ES ('a', 1) ON CONFLICT`），靠上下文里的名字归因会静默错位，
    所以先粗后细，绝不从截断的文本里猜索引。"""
    errs = {}
    raw = ""
    CHUNK = 80
    for start in range(0, len(stmts), CHUNK):
        idxs = list(range(start, min(start + CHUNK, len(stmts))))
        body = chr(10).join("PREPARE nc_%d AS %s;" % (i, stmts[i][0])
                            for i in idxs)
        e = _run(body)
        if not e.strip():
            continue
        raw += e
        for i in idxs:
            m = _err_of(stmts, i)
            if m:
                errs[i] = m
    return errs, raw


stmts = collect() + [(GHOST, "<synthetic>")]
errs, stderr_all = probe(stmts)
ghost_i = len(stmts) - 1

ghost_i = len(stmts) - 1

HARD = re.compile(r"ambiguous|column .* does not exist|operator does not "
                  r"exist|cannot cast|must appear in the GROUP BY", re.I)
SOFT = re.compile(r"syntax error|does not exist", re.I)
hard, soft, missing = {}, {}, {}
for i, msg in sorted(errs.items()):
    if i == ghost_i:      # 合成件只归 NC1 管
        continue
    path = stmts[i][1]
    if HARD.search(msg):
        hard.setdefault(path, []).append(msg[:70])
    elif SOFT.search(msg):
        (missing if "does not exist" in msg.lower() else soft).setdefault(
            path, []).append(msg[:70])

ok("NC1 正控：合成的 ambiguous 语句必须被同一支管线抓到（管线不是恒绿）",
   ghost_i in errs and "ambiguous" in errs.get(ghost_i, "").lower(),
   "ghost_err=%s" % errs.get(ghost_i))
for i, msg in sorted(errs.items()):
    if i == ghost_i or not HARD.search(msg):
        continue
    sql, path = stmts[i]
    print("   [硬错] %s" % path)
    print("          %s" % msg.split("ERROR:")[-1].strip())
    print("          SQL: %s" % " ".join(sql.split())[:110])
ok("NC2 「规划期就报真错」的文件 = 台账 %s（ambiguous / 列不存在 / 算子 / 转型）"
   % sorted(HARD_LEDGER), set(hard) == set(HARD_LEDGER),
   "实测=%s" % {k: v[:2] for k, v in hard.items()})
ok("NC3 反空转：探到语句 > 500 且**至少抓到一个真错**（合成 ghost 保证"
   "有 1 个：抓不到说明归因/采集坏了，空集一律不可信）",
   len(stmts) > 500 and bool(errs),
   "语句=%d 有错=%d stderr=%d 字节"
   % (len(stmts), len(errs), len(stderr_all.strip())))
print("   [账面外] 语法/片段类 %d 个文件（只打印不判）：%s"
      % (len(soft), sorted(soft)[:6]))
print("   [交给 MZ/既有门] does not exist 类 %d 个文件：%s"
      % (len(missing), sorted(missing)[:6]))
summary()
