# -*- coding: utf-8 -*-
"""闸门 CI：迁移链必须能在干净空库上从头跑通（站方 ce26d719 修过一次）。

0342 曾报 duplicate key (id)=(106) —— 0235 用显式 id 插入导致 SERIAL 序列不
前进，0341 结束时 max(id)=106 而 last_value=105，nextval() 又吐 106。已部署库
当年侥幸撞过去（那时 106 恰好空着），所以线上一直没暴露，**直到有人跑
upgrade_drill 才炸**。

所以这条必须有常驻账面：新建一个空库，把 apps/api/migrations 按序跑一遍。
任何一条失败 ⇒ 整条链中止（sqlx 的 run 是单链），后面所有迁移都不会执行。
"""
import io
import os
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DB = "flux_migr_probe"
admin = ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "postgres", "-v", "ON_ERROR_STOP=1"]


def run(args, timeout=900):
    return subprocess.run(args, capture_output=True, text=True,
                          encoding="utf-8", errors="replace", timeout=timeout)


def run_in(args, stdin, timeout=180):
    """SQL 走 stdin：部分迁移文本超过 Windows 命令行长度上限（WinError 206）。"""
    return subprocess.run(args, input=stdin, capture_output=True, text=True,
                          encoding="utf-8", errors="replace", timeout=timeout)


subprocess.run(["docker", "start", "flux-postgres"],
               capture_output=True, text=True, encoding="utf-8",
               errors="replace")
run(admin + ["-c", 'DROP DATABASE IF EXISTS "%s"' % DB])
r = run(admin + ["-c", 'CREATE DATABASE "%s"' % DB])
if r.returncode != 0:
    print("[FAIL] 建探针库失败：", (r.stderr or "")[:200])
    raise SystemExit(1)

# 逐条按序执行（与 sqlx 的 run 同语义：单链，前一条失败即中止）
mig_dir = os.path.join(ROOT, "apps/api/migrations")
files = sorted(f for f in os.listdir(mig_dir) if f.endswith(".sql"))
failed, applied = None, 0
for fn in files:
    sql = io.open(os.path.join(mig_dir, fn), encoding="utf-8").read()
    # SQL 走 stdin：部分迁移文本超过 Windows 命令行长度上限（WinError 206）。
    r = run_in(["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
                "-q", "-d", DB, "-v", "ON_ERROR_STOP=1", "-f", "-"], sql)
    if r.returncode != 0:
        failed = (fn, (r.stderr or "").strip().splitlines()[:2])
        break
    applied += 1

if failed:
    print("[FAIL] 第 %d/%d 条迁移失败：%s" % (applied + 1, len(files), failed[0]))
    for ln in failed[1]:
        print("   " + ln[:160])
    print("   后果：sqlx 的 run 是单链，这一条失败 ⇒ 它之后的所有迁移都不会执行。")
    print("   已部署库可能侥幸没暴露（某个 id 恰好空着），但干净环境必炸。")
    raise SystemExit(1)

nr = run(["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
          "-d", DB, "-tAc",
          "SELECT count(*) FROM pg_tables WHERE schemaname='public' "
          "AND tablename <> '_sqlx_migrations'"])
n = (nr.stdout or "").strip()
print("== ALL PASS（干净空库按序跑通 %d 条迁移，建出 %s 张表）=="
      % (applied, n or "?"))
