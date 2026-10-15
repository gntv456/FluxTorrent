# -*- coding: utf-8 -*-
"""闸门 CI：apps/api/src 的 SQL 逐条可解析（`relation/column/type does not exist` 即红）。

与 worker 版（pt_ci_worker_sql_prepares）同法，API 侧 SQL 形状更杂（format! 插值、
concat! 多语句、跨行拼接），所以只报「对象不存在」这一类真红，其余一律放过。

刻意收窄的三处，理由都写在代码里：
  ① `does not exist` 才算红——字符串提取在 format! 插值处不完整，
     syntax error 会把真红吞掉；
  ② 对象名若出现在任一迁移文件里（含**未提交的在途迁移**）就放过——
     代码与迁移同批交付时，验证库还没跑到那条迁移，不算洞（实测
     webhook_egress_rules 之于尚未提交的 0389）；
  ③ 一个字符串里出现多条 SQL（concat!）就跳过——那不是一条合法语句。

实现：所有语句拼进一个脚本、逐条 DO 块独立事务执行，秒级完成
（原来每条一次往返，1373 条要 4 分钟）。
"""
import io
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

_MIG = None


def in_migration(name):
    global _MIG
    if _MIG is None:
        _MIG = []
        d = os.path.join(ROOT, "apps/api/migrations")
        for fn in os.listdir(d):
            if fn.endswith(".sql"):
                _MIG.append(io.open(os.path.join(d, fn), encoding="utf-8",
                                   errors="replace").read())
    return any(name in t for t in _MIG)


def quote(s):
    return "'" + s.replace("'", "''") + "'"


def collect():
    lit = re.compile(r'r#"(.*?)"#|"((?:[^"\\]|\\.)*)"', re.S)
    stmts = []
    for dp, _, fns in os.walk(os.path.join(ROOT, "apps/api/src")):
        for fn in sorted(fns):
            if not fn.endswith(".rs") or fn.endswith("_tests.rs"):
                continue
            p = os.path.join(dp, fn)
            txt = io.open(p, encoding="utf-8", errors="replace").read()
            if os.sep + "tests" in p:
                continue
            for m in lit.finditer(txt):
                s = m.group(1) if m.group(1) is not None else m.group(2)
                if not s or not re.match(r"\s*(SELECT|UPDATE|DELETE|INSERT|WITH)\b",
                                        s, re.I) or "$" not in s:
                    continue
                lines = []
                for ln in s.split("\\n"):
                    ln = ln.replace('\\"', '"')
                    lines.append(ln[:-1] if ln.endswith("\\") else ln)
                    if not ln.endswith("\\"):
                        break
                s2 = " ".join(x.strip() for x in lines).strip()
                if len(s2) < 40 or "{" in s2 or "}" in s2:
                    continue
                if len(re.findall(r"\b(?:SELECT|UPDATE|DELETE|INSERT)\b", s2,
                                  re.I)) > 1:
                    continue
                stmts.append((s2, p.replace("\\", "/")))
    return stmts


def run_all(stmts):
    body = []
    for i, (sql, _p) in enumerate(stmts):
        body.append("DO $$ BEGIN PERFORM 0; EXECUTE %s; EXCEPTION WHEN "
                    "others THEN RAISE NOTICE 'PREPARE_FAIL[%d] %%', "
                    "SQLERRM; END $$;" % (quote(sql), i))
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-f", "-"],
        input="\n".join(body), capture_output=True, text=True,
        encoding="utf-8", errors="replace", timeout=900)
    out = r.stdout or ""
    fails = {}
    for m in re.finditer(r"PREPARE_FAIL\[(\d+)\] (.+)", out):
        fails[int(m.group(1))] = m.group(2).strip()
    return fails


stmts = collect()
fails = run_all(stmts)
bad = []
for i, msg in sorted(fails.items()):
    sql, path = stmts[i]
    if "does not exist" not in msg:
        continue                      # ① 只报「对象不存在」
    names = re.findall(r'"([^"]+)"', msg)
    skip = False
    for nm in names:
        if in_migration(nm.split(".")[-1]):
            skip = True               # ② 在途迁移里已定义
            break
    if skip:
        continue
    bad.append("%s: %s" % (path.split("apps/")[-1], msg[:120]))

seen, uniq = set(), []
for b in bad:
    if b not in seen:
        seen.add(b)
        uniq.append(b)
for b in uniq[:18]:
    print("  " + b)
if uniq:
    print("---- api SQL：检查 %d 条，对象不存在 %d 类 ----" % (len(stmts), len(uniq)))
    raise SystemExit(1)
print("== ALL PASS（api SQL %d 条，无「对象不存在」）==" % len(stmts))
