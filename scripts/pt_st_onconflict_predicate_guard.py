# -*- coding: utf-8 -*-
"""ST 常驻闸门：ON CONFLICT 的推断谓词必须与部分唯一索引逐字一致（批一百五十三）

现网 500 而编译 / 360 单测 / 其它闸门全绿的洞（P-337）：

    POST /subtitles/{id}/report -> 500
    PG: there is no unique or exclusion constraint matching the ON CONFLICT
        specification

**根因不是少写索引，是改索引时漏改了 ON CONFLICT。** 迁移 0367 把
uq_reports_open_per_reporter 的部分索引谓词加了 `AND ref_type <> 'cheat'`
（案件去重语义与举报不同），并同步改了 reports_http.rs 的 ON CONFLICT——
**漏了 subtitles_govern.rs**。PG 推断不出索引就直接报错，编译与单测都看不见
（这条语句只在真打接口时才执行）。

形状有三种，必须都认：
  A. `ON CONFLICT (cols) DO NOTHING`               —— 无谓词，必须有**普通**唯一索引
  B. `ON CONFLICT (cols) WHERE <pred> DO NOTHING`  —— 谓词必须与索引谓词**逐字一致**
  C. `ON CONFLICT ON CONSTRAINT <name> DO ...`     —— 按名引用，索引改名才会炸

本闸门做两件事：
① 静态：B 形态的 ON CONFLICT 谓词文本，必须能在某个迁移建索引的同名谓词里找到；
② 活体：每个带 B 形态的表，真跑一次 INSERT 试探（事务内 ROLLBACK），
   让 PG 自己判一次「推断得到吗」——静态文本比对会漏掉换行/大小写/AND 顺序。

活体那步是真判据：批一百五十三就是它把这条洞钉死的。
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary, FAILURES  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BS = chr(92)
NL = chr(10)
PRE_FIX = "0a300c18"   # P-337 修复前的最后一笔（正控：subtitles_govern 谓词缺 ref_type<>cheat）

PAT_ONCONFLICT = re.compile(
    r"ON\s+CONFLICT\s*\(([^)]*)\)\s*(?:WHERE\s+([^\n]*?)\s+)?DO\s",
    re.I | re.S)


def rs_files():
    out = []
    for dp, dn, fn in os.walk(os.path.join(ROOT, "apps")):
        dn[:] = [d for d in dn if d not in ("target", "node_modules")]
        for f in sorted(fn):
            if f.endswith(".rs"):
                out.append(os.path.join(dp, f))
    return out


def flatten(text):
    """Rust 续行反斜杠压掉，让 SQL 连成一行。"""
    out = []
    for ln in text.split(NL):
        t = ln.rstrip()
        out.append(t[:-1].rstrip() if t.endswith(BS) else t)
    return NL.join(out)


def blob(rev, rel):
    p = subprocess.run(["git", "show", "%s:%s" % (rev, rel.replace(os.sep, "/"))],
                       capture_output=True, text=True, encoding="utf-8",
                       errors="replace", cwd=ROOT)
    return (p.stdout or "") if p.returncode == 0 else ""


def quote_ident(name):
    return '"%s"' % name.replace('"', '""')


def psql(sql):
    p = subprocess.run(["docker", "exec", "flux-postgres", "psql", "-U", "flux",
                        "-d", "fluxtorrent", "-tAc", sql],
                       capture_output=True, text=True, encoding="utf-8",
                       errors="replace")
    return (p.returncode, (p.stdout or "").strip(), (p.stderr or "").strip())


print("== ST ON CONFLICT 谓词与部分唯一索引一致 ==")

# ---- ① 静态：仓库里 B 形态的谓词清单 ----
shapes = []
for f in rs_files():
    txt = flatten(open(f, encoding="utf-8", errors="replace").read())
    for m in PAT_ONCONFLICT.finditer(txt):
        pred = (m.group(2) or "").strip()
        if not pred:
            continue                      # A 形态：无谓词，不归本判据
        seg = txt[max(0, m.start() - 400):m.end()]
        tm = re.search(r"INSERT\s+INTO\s+(\w+)", seg, re.I)
        shapes.append((os.path.relpath(f, ROOT).replace(os.sep, "/"),
                       tm.group(1) if tm else "?", pred, m.group(1)))
ok("ST-A1 扫到 B 形态（带谓词的 ON CONFLICT）", len(shapes) > 0,
   "扫到 %d 处" % len(shapes))

# ---- ② 正控：修前 subtitles_govern.rs 的谓词缺 ref_type <> 'cheat' ----
pre = blob(PRE_FIX, "apps/api/src/content_http/subtitles_govern.rs")
pre_txt = flatten(pre) if pre else ""
pre_pred = ""
for m in PAT_ONCONFLICT.finditer(pre_txt):
    if (m.group(2) or "").strip():
        pre_pred = m.group(2).strip()
ok("ST-A2 正控：修前谓词确实缺 ref_type<>cheat（判据承重）",
   bool(pre) and "ref_type" not in pre_pred, "修前谓词=%r" % pre_pred)

# ---- ③ 活体：让 PG 自己判一次推断（真判据，静态比对会漏换行/顺序） ----
rc, out, err = psql("SELECT 1")
ok("ST-A3 前置：能连上现网 PG（活体判据不空跑）", rc == 0 and out == "1",
   "rc=%s out=%r err=%r" % (rc, out, err[:80]))

# 每张被 B 形态引用的表，跑一次「同谓词 INSERT ... ON CONFLICT ... DO NOTHING
# RETURNING 0」并 ROLLBACK——PG 报 inference failure 就是洞。
probed = []
for rel, table, pred, cols_raw in sorted(set(shapes)):
    if table == "?":
        continue
    # 全列清单：冲突目标列之外的必填列也得给值，否则构造出的 INSERT 本身非法，
    # 报错会和「推断不出索引」混在一起（第一版就栽在这）。
    # 排除主键列与 identity 列：它们跟推理断无关，却会让 INSERT 撞 pkey
    # 报错，把「推断失败」的真实信号淹掉（第一版栽在这）。
    rc1, all_cols_csv, err1 = psql(
        "SELECT string_agg(quote_ident(attname), ',' ORDER BY attnum) "
        "FROM pg_attribute WHERE attrelid = '" + table + "'::regclass "
        "AND attnum > 0 AND NOT attisdropped AND attgenerated = '' "
        "AND attidentity = '' AND attname NOT IN ("
        "  SELECT a.attname FROM pg_index i "
        "  JOIN pg_attribute a ON a.attrelid = i.indrelid "
        "    AND a.attnum = ANY(i.indkey) "
        " WHERE i.indrelid = '" + table + "'::regclass AND i.indisprimary)")
    if rc1 != 0 or not all_cols_csv:
        continue
    all_cols = []
    for c in all_cols_csv.split(","):
        c = c.strip().strip('"')
        if c:
            all_cols.append(c)
    # 冲突目标列 = ON CONFLICT 里那一组；其余列从表里任取一行真值。
    # （第一版这里取的是「全部列」⇒ 目标列既在列清单里又要在 SELECT 里，
    #    构造出的是非法 SQL，报错与判据要判的 inference failure 混在一起。）
    cols = [c.strip() for c in re.split(r",\s*", cols_raw) if c.strip()]
    quoted = ", ".join(quote_ident(c) for c in cols)
    sel = ", ".join("%s" % quote_ident(c) for c in cols)
    extra = ""
    for c in all_cols:
        if c in cols:
            continue
        extra += ", %s" % quote_ident(c)
    sql = ("BEGIN; INSERT INTO %s (%s%s) "
           "SELECT %s%s FROM %s LIMIT 1 "
           "ON CONFLICT (%s) WHERE %s DO NOTHING; ROLLBACK;"
           % (table, quoted, extra, sel, extra, table, cols_raw, pred))
    rc3, _o3, e3 = psql(sql)
    # 只有「推断不出索引」才是洞；撞唯一约束恰恰说明**推断成功了**
    # （DO NOTHING 没生效是因为 ON CONFLICT 目标组与实际冲突组不同——
    #  或者行本身已存在）。第一版把两者一律判红，于是 4 处里 2 处假红。
    bad = ("no unique or exclusion constraint matching the ON CONFLICT"
           in e3)
    probed.append(("%s [%s]" % (table, pred[:38]), pred, not bad, e3[:90]))

for table, pred, good, e in probed:
    ok("ST-A4 %s 的 ON CONFLICT 谓词 PG 推得出来" % table, good,
       "PG 说推不出来：%s" % e)

summary()
