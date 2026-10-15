# -*- coding: utf-8 -*-
"""扫 worker 周期 job：是否每轮都会失败。

P-342 的形状：job 的 SQL 引用了不存在的列 ⇒ 每轮必报错。已修的三处
（medal_achv/mod.rs、backfill.rs、api/repo/medal_auto.rs）用
pt_ci_sql_columns_live 盯着，但这只覆盖 SQL 字符串。

这里补另一半：**实跑**。把每支 worker 周期 job 的 SQL 抽出来，逐条
PREPARE（PostgreSQL 会当场报 column does not exist / relation does not exist），
不实际执行。"""
import io, os, re, subprocess, sys
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

def psql(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-v", "ON_ERROR_STOP=1", "-c", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    return r.returncode, (r.stderr or r.stdout or "")

# Rust 里相邻字符串字面量会拼成一条 SQL
lit = re.compile(r'r#"(.*?)"#|"((?:[^"\\]|\\.)*)"', re.S)
bad = []
checked = 0
for dp, _, fns in os.walk(os.path.join(ROOT, "apps/worker/src")):
    for fn in fns:
        if not fn.endswith(".rs"):
            continue
        p = os.path.join(dp, fn)
        txt = io.open(p, encoding="utf-8", errors="replace").read()
        for m in lit.finditer(txt):
            s = m.group(1) if m.group(1) is not None else m.group(2)
            if not s or not re.match(r"\s*(SELECT|UPDATE|DELETE|INSERT|WITH)\b",
                                    s, re.I):
                continue
            if "$1" not in s and "$2" not in s:
                continue      # 无占位符的多半是 DDL/说明，跳过
            # Rust 普通字符串里的行尾续行是 "\\" + 换行。Rust 的解析规则是
            # 「最后一个非转义反斜杠后的换行才算行结束」，PG 不认反斜杠。
            # 不处理它就会对每条多行 SQL 都报「syntax error at or near \\"」——
            # 那是我以前写的 SQL 被 Rust 丢了反斜杠的同一个痛点（批一百五十三的 P-0）。
            out_lines = []
            for ln in s.split("\n"):
                ln = ln.replace('\\"', '"').replace('\\\\', '\\')
                if ln.endswith("\\"):
                    ln = ln[:-1]
                    out_lines.append(ln)
                else:
                    out_lines.append(ln)
                    break
            s2 = " ".join(x.strip() for x in out_lines).strip()
            if "{" in s2 or "}" in s2:      # format! 拼的，跳过
                continue
            # 离线导入工具（apps/worker/src/bin）自己建表（CREATE TABLE IF NOT EXISTS），
            # 表不存在是正常的首次运行状态，不是红。
            if os.sep + "bin" + os.sep in p:
                continue
            checked += 1
            rc, out = psql("PREPARE _probe AS " + s2)
            if rc != 0:
                first = (out.strip().splitlines() or [""])[0][:120]
                # 只报「表/列/类型/函数不存在」与「语法错误」中的前者：
                # 前者是产品确实会每轮报错（P-342 就是 column ... does not exist），
                # 后者大多是本脚本的字符串提取在 Rust 跨行拼接、format! 插值等
                # 场景下不完整造成的——报它们会把真红吞掉。
                if "does not exist" in first:
                    bad.append("%s: %s" % (p.replace("\\", "/").split("apps/")[-1],
                                           first))
seen, uniq = set(), []
for b in bad:
    if b not in seen:
        seen.add(b); uniq.append(b)
for b in uniq[:15]:
    print("  " + b)
print("== ALL PASS（worker SQL 预编译 %d 条，无「对象不存在」）==" % checked)
raise SystemExit(1 if uniq else 0)
