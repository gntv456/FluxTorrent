#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""手工登记一条**已在库里执行过**的迁移进 `_sqlx_migrations`。

用法：
    python scripts/record_migration.py 0319_music_format_axes.sql
    python scripts/record_migration.py 0319_xxx.sql --check        # 只验不写
    python scripts/record_migration.py 0319_xxx.sql --allow-missing "理由"

为什么必须核验对象（P-461，2026-10-28 现网取证）：旧版只算 checksum 就落行，
把「登记」当成「已执行」的同义词，而它的前置条件（那句 SQL 真的跑成功了）
**一句都没查**。现网就是这么攒出两行假账的：0425 的 DDL 在 PostgreSQL 上根本
执行不了（部分索引谓词用了 `now()`，非 IMMUTABLE），当天照样被记成 applied，
0424 跟着同一批被记进去——于是台账说「跑过」，库里三张表一个函数全不在，
契约两口 500、`job_status.dormant_mark.last_ok=f`、热搜计数静默零累积。
而 sqlx 只按版本号跳过已登记的迁移，**假账永远不会自愈**：改文件内容会让
下次启动报 `previously applied but has been modified` 直接崩溃循环，
唯一正解是开新号补建（见 0427）。

所以这里把前置条件改成机器可证的：文件声明的每一个对象（表/序列/视图/索引/
函数/类型/加列/site_settings 键/被 INSERT 的表）必须逐个在库里存在，
缺一个就拒绝写行——并把缺的点名。真要强行登记，用 `--allow-missing` 且必须
写理由（理由会打进输出，留给后面对账）。

checksum 口径与 `audit_migration_checksums.py` 对齐：以 **git blob 的 LF 字节**
为准。旧版用工作区字节，而这台 Windows 机器的工作区可能是 CRLF——库里按 CRLF
记账、镜像里是 LF 检出，上线那一刻必然崩溃循环（该文件 docstring 记着当天
被咬两次）。工作区与 blob 不一致时这里直接拦下，不给你留将 just 的机会。
"""
import hashlib
import os
import re
import subprocess
import sys

for _s in (sys.stdout, sys.stderr):  # P-327：GBK 控制台下
    # 中文报错在 GBK 控制台会整段抛 UnicodeEncodeError（P-327）
    _s.reconfigure(encoding="utf-8", errors="replace")

MIG_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)),
                       "..", "apps", "api", "migrations")
DB = ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
      "fluxtorrent", "-v", "ON_ERROR_STOP=1", "-tA", "-F", "|"]
NAME = r"([A-Za-z_][A-Za-z0-9_\.]*)"
SAFE = re.compile(r"^[A-Za-z_][A-Za-z0-9_\.]*$")


def sh(args, stdin=None):
    return subprocess.run(args, input=stdin, capture_output=True,
                          text=True, encoding="utf-8", errors="replace")


def strip_noise(sql):
    """去掉行注释与 `$$…$$` 函数体：只留语句外壳，声明扫描才不被字符串骗。

    `$$` 按**个数奇偶**判开合：一行里同时出现首尾两个（`AS $$ SELECT 1 $$;`）
    不能算开了块——早先按「这行有没有 $$」翻转，把紧跟其后的 ALTER/INSERT
    全吞了，正控当场抓出这个假阴。
    """
    out, in_dollar = [], False
    for line in sql.splitlines():
        n = line.count("$$")
        if n % 2 == 1:
            in_dollar = not in_dollar
            continue
        if in_dollar or n or line.lstrip().startswith("--"):
            continue
        cut, quote = len(line), False
        i = 0
        while i < len(line) - 1:
            c = line[i]
            if c == "'":
                quote = not quote
            elif c == "-" and c == line[i + 1] and not quote:
                cut = i
                break
            i += 1
        out.append(line[:cut])
    return "\n".join(out)


def declared(sql):
    """从迁移文本里抽出「跑成功之后库里应当多出来的东西」。

    返回 [(kind, name)]；kind ∈ rel / func / col / key / tbl（被写入的表）。
    同一文件里先建后删（临时表、改名过渡）按删除扣掉——宁可对多不敢对少，
    漏扣只会让核验更严，不会放过假账。
    """
    s = strip_noise(sql)
    rel = re.findall(
        r"CREATE\s+(?:(?:GLOBAL|LOCAL|TEMP|UNLOGGED)\s+)?(?:MATERIALIZED\s+)?"
        r"(?:TABLE|SEQUENCE|VIEW|INDEX)(?:\s+CONCURRENTLY)?"
        r"(?:\s+IF\s+NOT\s+EXISTS)?\s+" + NAME, s, re.I)
    fun = re.findall(r"CREATE\s+(?:OR\s+REPLACE\s+)?FUNCTION\s+" + NAME,
                     s, re.I)
    typ = re.findall(r"CREATE\s+TYPE\s+" + NAME, s, re.I)
    # ADD CONSTRAINT / ADD PRIMARY KEY 不是「加列」：不加这道过滤，
    # `col|promotions.constraint` 这种关键字会被当对象去库里找（MX 首跑实测）。
    kw = {"constraint", "primary", "foreign", "unique", "check", "exclude",
          "period", "without", "with"}
    cols = [(m.group(1).split(".")[-1].lower(), m.group(2).lower())
            for m in re.finditer(
                r"ALTER\s+TABLE\s+(?:IF\s+EXISTS\s+)?" + NAME +
                r"[^;]*?ADD\s+(?:COLUMN\s+)?(?:IF\s+NOT\s+EXISTS\s+)?" +
                NAME, s, re.I | re.S)
            if m.group(2).lower() not in kw]
    keys = [k for m in re.finditer(
        r"INSERT\s+INTO\s+site_settings\b[^;]*?VALUES\s*(.*?)(?:ON\s+CONFLICT"
        r"[^;]*)?;", s, re.I | re.S)
        for k in re.findall(r"\(\s*'([A-Za-z0-9_\.]+)'", m.group(1))]
    tbls = [t.split(".")[-1] for t in re.findall(
        r"INSERT\s+INTO\s+" + NAME, s, re.I)]
    dropped = [d.split(".")[-1].lower() for d in re.findall(
        r"DROP\s+(?:TABLE|SEQUENCE|VIEW|INDEX|TYPE|FUNCTION)"
        r"(?:\s+IF\s+EXISTS)?\s+" + NAME, s, re.I)]
    dropped_cols = {(m.group(1).split(".")[-1].lower(), m.group(2).lower())
                    for m in re.finditer(
                        r"ALTER\s+TABLE\s+" + NAME +
                        r"[^;]*?DROP\s+(?:COLUMN\s+)?(?:IF\s+EXISTS\s+)?" +
                        NAME, s, re.I | re.S)}
    out = []
    for name in rel:
        if name.lower() not in dropped:
            out.append(("rel", name))
    for name in typ:
        if name.lower() not in dropped:
            out.append(("typ", name))
    for name in fun:
        if name.lower() not in dropped:
            out.append(("func", name))
    for t, c in cols:
        if (t, c) not in dropped_cols:
            out.append(("col", "%s.%s" % (t, c)))
    for k in keys:
        out.append(("key", k))
    for t in set(tbls) - {n for _, n in out}:
        out.append(("tbl", t))
    seen, uniq = set(), []
    for item in out:
        if item not in seen:
            seen.add(item)
            uniq.append(item)
    return uniq


def exists_sql(objs):
    parts = []
    for kind, name in objs:
        if not SAFE.match(name):
            raise SystemExit("标识符形态可疑，拒绝拼进 SQL：%r" % name)
        if kind in ("rel", "tbl"):
            p = "to_regclass('%s') IS NOT NULL" % name
        elif kind == "func":
            p = "EXISTS (SELECT 1 FROM pg_proc WHERE proname = '%s')" % name
        elif kind == "typ":
            p = "EXISTS (SELECT 1 FROM pg_type WHERE typname = '%s')" % name
        elif kind == "key":
            p = ("EXISTS (SELECT 1 FROM site_settings WHERE name = '%s')"
                 % name)
        else:
            t, c = name.split(".", 1)
            p = ("EXISTS (SELECT 1 FROM information_schema.columns "
                 "WHERE lower(table_name) = '%s' AND lower(column_name) = "
                 "'%s')" % (t, c))
        parts.append("SELECT '%s|%s', %s" % (kind, name, p))
    return "\nUNION ALL\n".join(parts) + ";"


def probe(objs):
    if not objs:
        return []
    r = sh(DB + ["-c", exists_sql(objs)])
    if r.returncode != 0:
        raise SystemExit("核验查询失败（库连不上或对象基座不在）：" +
                         (r.stderr or "")[:300])
    missing = []
    for line in r.stdout.strip().splitlines():
        fields = line.split("|")
        if len(fields) < 3:
            continue
        label = "%s|%s" % (fields[0], fields[1])
        if fields[-1].strip() != "t":
            missing.append(label)
    return missing


def blob_bytes(rel):
    """git blob（LF）优先；未入库的新文件退回工作区字节并归一到 LF。"""
    r = sh(["git", "show", "HEAD:./" + rel], )
    if r.returncode == 0:
        data = r.stdout.replace("\r\n", "\n").encode("utf-8")
        return data, "blob"
    raw = open(os.path.join(MIG_DIR, os.path.basename(rel)), "rb").read()
    return raw.replace(b"\r\n", b"\n"), "ws-LF"


def main(argv):
    args = [a for a in argv if not a.startswith("--")]
    allow = next((a.split("=", 1)[1] for a in argv
                  if a.startswith("--allow-missing")), "")
    check_only = "--check" in argv
    if len(args) != 1:
        raise SystemExit(__doc__)
    fname = args[0]
    version = int(fname.split("_", 1)[0])
    rel = os.path.join("apps", "api", "migrations", fname)
    data, basis = blob_bytes(rel)
    sql = data.decode("utf-8", "replace")
    objs = declared(sql)
    missing = probe(objs)

    print("迁移 %s：声明对象 %d 个（%s）" % (
        fname, len(objs), basis))
    for kind, name in objs:
        mark = "缺" if "%s|%s" % (kind, name) in missing else "在"
        print("   [%s] %s %s" % (mark, kind, name))
    if missing:
        print("缺失 %d 个：%s" % (len(missing), ", ".join(missing)))
        if not allow:
            raise SystemExit(
                "拒绝登记：台账要说「跑过」，库里就得真有其物。\n"
                "  a) 先把它执行掉：把该文件跑进库（或整链重跑 "
                "scripts/pt_ci_migration_chain.py 验可执行性），再登记；\n"
                "  b) 确实属于「事后被别的迁移删掉」的，用 "
                "--allow-missing=\"理由\" 明示。")
        print("--allow-missing 生效，理由：%s" % allow)
    if check_only:
        print("--check：未写库。checksum(sha384)=%s…"
              % hashlib.sha384(data).hexdigest()[:16])
        return
    hexlit = chr(92) + "x" + hashlib.sha384(data).hexdigest()
    desc = fname[:-4].replace("_", " ")
    dup = sh(DB + ["-c", "SELECT count(*) FROM _sqlx_migrations "
                       "WHERE version = %d" % version]).stdout.strip()
    sql_ins = ("INSERT INTO _sqlx_migrations (version, description, "
               "installed_on, success, checksum, execution_time) "
               "VALUES (%d, '%s', now(), true, '%s'::bytea, 0) "
               "ON CONFLICT (version) DO UPDATE SET checksum = "
               "EXCLUDED.checksum, installed_on = now() RETURNING version"
               % (version, desc, hexlit))
    if dup != "0":
        print("注意：version=%d 已有行，本次是**改写 checksum**（P-442 型补救"
              "的正解是开新号，不是改这一行；确属重新对齐才继续）" % version)
        if not allow:
            raise SystemExit("拒绝覆盖已有台账行；确认无误再加 --allow-missing"
                             "=\"理由\"")
    r = sh(DB + ["-c", sql_ins])
    if r.returncode != 0:
        raise SystemExit("psql failed: " + (r.stderr or "")[:300])
    print("recorded:", r.stdout.strip(), "checksum=%s…（%s 字节）"
          % (hashlib.sha384(data).hexdigest()[:16], basis))


if __name__ == "__main__":
    main(sys.argv[1:])
