"""迁移校验和口径审计：以 **git blob 字节** 为准比对 _sqlx_migrations。
sqlx (0.8) 存 sha384。只读，不写库。

为什么判据必须落在 blob 上（2026-10-07 改版；旧版是假通过）：
旧实现拿「工作区字节」算哈希，而这台 Windows 机器的工作区恰好是
autocrlf 的 CRLF 形态、与当年记账的字节一致 ⇒ 永远全绿。
可镜像是从**检出后的文件**构建的：`git archive` / 干净 clone / CI checkout
都是 LF。于是凡「库里按 CRLF 记账、仓库 blob 是 LF」的迁移，
在干净检出的构建里必然让 api 启动即
`Error: migration NNN was previously applied but has been modified`
——healthcheck 不过，worker 因依赖 api 也起不来。当天被咬两次
（一次报 156，一次被我整批转成 CRLF 后报 1）。

结论口径：能在 blob 的 LF 字节上对上才算健康；只对 CRLF 工作区成立的，
一律判问题（它意味着「只有这台机器的构建能上线」）。
"""
import hashlib
import os
import re
import subprocess
import sys

MIG_DIR = "apps/api/migrations"


def sh(*args, binary=False):
    r = subprocess.run(list(args), capture_output=True,
                       **({"text": True, "encoding": "utf-8",
                           "errors": "replace"} if not binary else {}))
    return r


def db_rows():
    res = sh("docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
             "fluxtorrent", "-t", "-A", "-c",
             "SELECT version, encode(checksum,'hex') FROM _sqlx_migrations "
             "ORDER BY version")
    if res.returncode:
        raise SystemExit("读取 _sqlx_migrations 失败：" + res.stderr[:200])
    out = {}
    for line in res.stdout.strip().splitlines():
        if "|" in line:
            v, c = line.split("|")
            out[int(v)] = c.strip().lower()
    return out


def blob(rel):
    r = sh("git", "cat-file", "blob", "HEAD:" + rel, binary=True)
    return None if r.returncode else r.stdout


def h(b):
    return hashlib.sha384(b).hexdigest()


def main():
    if not os.path.isdir(MIG_DIR):
        raise SystemExit("请在仓库根目录运行本脚本")
    db = db_rows()
    problems, pending, ok = [], [], 0
    for name in sorted(os.listdir(MIG_DIR)):
        if not re.match(r"^\d{4}_.*\.sql$", name):
            continue
        ver = int(name[:4])
        b = blob(MIG_DIR + "/" + name)
        if b is None:
            problems.append((ver, "NOT-IN-HEAD", name))
            continue
        stored = db.pop(ver, None)
        if stored is None:
            pending.append((ver, name))
            continue
        lf = h(b.replace(b"\r\n", b"\n"))
        crlf = h(b.replace(b"\n", b"\r\n"))
        if stored == lf or stored == h(b):
            ok += 1
        elif stored == crlf:
            problems.append((ver, "DB-CRLF-BLOB-LF（干净检出构建必拒启）", name))
        else:
            problems.append((ver, "CONTENT-MISMATCH", name))
    for ver in sorted(db):
        problems.append((ver, "APPLIED-IN-DB-BUT-FILE-MISSING",
                         "(db only, %s…)" % db[ver][:12]))

    print("库里已应用 %d 条 / blob 对得上 %d 条 / 待应用 %d 条"
          % (len(db) + ok + len(problems), ok, len(pending)))
    if pending:
        print("  待应用: " + ", ".join(str(v) for v, _ in pending[-8:]))
    if problems:
        print("problem rows:")
        for p in problems:
            print("  ", p)
        sys.exit(1)
    print("OK: 每条已应用迁移的记账都能对上仓库 blob 的 LF 字节"
          "（干净检出/CI 构建可启）")


if __name__ == "__main__":
    main()
