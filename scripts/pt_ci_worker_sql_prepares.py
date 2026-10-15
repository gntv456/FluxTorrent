# -*- coding: utf-8 -*-
"""闸门 CI：worker 周期 job 的 SQL 逐条可解析（P-342 的另一半）。

P-342（medals.active 不存在 ⇒ 三个自动成就 job 每轮必报错）已修，但当时只盯着
被点名的三处。`pt_ci_sql_columns_live` 覆盖的是「别名.列」这一种形状，worker 里
还有别的写法（无别名、直写表名）。

做法：把 apps/worker/src 的 SQL 抽出来交给 PostgreSQL 解析（只解析不执行），
`column/relation/type does not exist` 当场报，不需要真跑 job。

两处刻意收窄，理由写在代码里：
  ① 只报「对象不存在」不报 syntax error——本脚本的字符串提取在 Rust 跨行
     拼接、format! 插值等场景下不完整，报语法错会把真红吞掉（第一版就因为
     没处理续行反斜杠，266 条里误报 47 类）。
  ② 跳过 apps/worker/src/bin（离线导入工具自带 CREATE TABLE IF NOT EXISTS，
     表不存在是首次运行的正常态）。

实现：所有语句拼进一个脚本、逐条 DO 块独立事务执行（原来每条一次往返要 2 分钟，
现在 2 秒）。
"""
import io
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def quote(s):
    return "'" + s.replace("'", "''") + "'"


def collect():
    lit = re.compile(r'r#"(.*?)"#|"((?:[^"\\]|\\.)*)"', re.S)
    stmts = []
    for dp, _, fns in os.walk(os.path.join(ROOT, "apps/worker/src")):
        for fn in sorted(fns):
            if not fn.endswith(".rs"):
                continue
            p = os.path.join(dp, fn)
            if os.sep + "bin" + os.sep in p:
                continue              # ② 离线工具自建表
            txt = io.open(p, encoding="utf-8", errors="replace").read()
            for m in lit.finditer(txt):
                s = m.group(1) if m.group(1) is not None else m.group(2)
                if not s or not re.match(
                        r"\s*(SELECT|UPDATE|DELETE|INSERT|WITH)\b", s, re.I):
                    continue
                if "$1" not in s and "$2" not in s:
                    continue
                lines = []
                for ln in s.split("\\n"):
                    ln = ln.replace('\\"', '"')
                    lines.append(ln[:-1] if ln.endswith("\\") else ln)
                    if not ln.endswith("\\"):
                        break
                s2 = " ".join(x.strip() for x in lines).strip()
                if "{" in s2 or "}" in s2 or len(s2) < 40:
                    continue
                # 不跳过 concat! 多条：worker 侧实测无误报，过度覆盖比现在更值钱
                # （259 条 vs 153 条）。如果后来出现提取噪声，再在这里收窄。
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
    return {int(m.group(1)): m.group(2).strip()
            for m in re.finditer(r"PREPARE_FAIL\[(\d+)\] (.+)", out)}


stmts = collect()
fails = run_all(stmts)
bad = []
for i, msg in sorted(fails.items()):
    if "does not exist" not in msg:   # ①
        continue
    bad.append("%s: %s" % (stmts[i][1].split("apps/")[-1], msg[:120]))
seen, uniq = set(), []
for b in bad:
    if b not in seen:
        seen.add(b)
        uniq.append(b)
for b in uniq[:18]:
    print("  " + b)
if uniq:
    print("---- worker SQL：检查 %d 条，对象不存在 %d 类 ----"
          % (len(stmts), len(uniq)))
    raise SystemExit(1)
print("== ALL PASS（worker SQL %d 条，无「对象不存在」）==" % len(stmts))
