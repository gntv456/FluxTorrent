# -*- coding: utf-8 -*-
"""闸门 CI：迁移与 SQL 里不得出现 MySQL 专有类型/子句（本项目只用 PostgreSQL）。

实测（批一百五十五）：站方新迁移 0381_exam_stages.sql 的
`ADD COLUMN ... exam_last_stage TINYINT NOT NULL DEFAULT 0` —— TINYINT 是
MySQL 类型，PostgreSQL 没有。后果不是「某一列建不出来」，而是**整个迁移
事务失败 → 进程启动即崩**（`error returned from database: type "tinyint"
does not exist`），任何从零初始化或重放该版本的环境都起不来。同文件其它
四列都写的 INT，只有这一处是 TINYINT，属笔误。

判据只扫代码行（剥掉 `--` 注释），否则注释里的反引号/NexusPHP 会全篇误报。
"""
import io
import os
import re
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

# 只收 PostgreSQL 确实没有的：MySQL 专有类型与子句。
PAT = re.compile(
    r"\b(TINYINT|MEDIUMINT|LONGTEXT|MEDIUMTEXT|TINYTEXT"
    r"|AUTO_INCREMENT|ENGINE\s*=\s*\w+\s*(?:\(|/|$)|UNSIGNED\s+(?:INT|BIGINT|TINYINT))\b",
    re.I)

hits = []
roots = ["apps/api/migrations", "apps/worker/migrations"]
for root in roots:
    base = os.path.join(os.path.dirname(os.path.dirname(
        os.path.abspath(__file__))), root)
    if not os.path.isdir(base):
        continue
    for dp, _, fns in os.walk(base):
        for fn in fns:
            if not fn.endswith(".sql"):
                continue   # 只判 SQL；Rust 里的 `Engine::new(...)` 不是 MySQL 语法
            p = os.path.join(dp, fn)
            for i, ln in enumerate(
                    io.open(p, encoding="utf-8", errors="replace"), 1):
                code = ln.split("--", 1)[0]
                if PAT.search(code):
                    hits.append("%s:%d  %s" % (p.replace("\\", "/"), i,
                                                code.strip()[:90]))

if hits:
    print("[FAIL] %d 处 MySQL 专有语法（本项目只跑 PostgreSQL）：" % len(hits))
    for h in hits[:20]:
        print("   " + h)
    print("   后果：迁移事务整体失败 ⇒ 进程启动即崩（实测 0381 的 TINYINT）。")
    raise SystemExit(1)
print("== ALL PASS（无 MySQL 专有类型/子句）==")
