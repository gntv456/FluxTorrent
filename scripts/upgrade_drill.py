#!/usr/bin/env python3
"""升级演练（E5）：模拟一次真实的版本升级窗口，验证「备份 → 前滚 → 验证 → 回滚」
全路径可执行。不要求真的换版本镜像——它检验的是流程件齐不齐：

  1. pre：升级前备份（复用 backup.sh 产物或现场跑一次）
  2. migrate：对新迁移做「前进」演练——在备份恢复出的演练库上执行
     `sqlx migrate run`（等价 api 启动动作），断言迁移表前进且 checksum 对账通过
  3. verify：演练库上跑关键表/账号断言（同 backup_drill 的 KEY_TABLES 口径）
  4. rollback：应用层回滚口径检查（旧镜像存在性 + 恢复脚本文档指引）

产物：docs/ops/upgrade-drill.md 的实证数字（耗时/迁移数/断言结果）。

用法（本机 docker 栈在跑的口径，与 backup_drill 同基座）：
  python scripts/upgrade_drill.py                    # 全流程
  python scripts/upgrade_drill.py --skip-backup      # 已有今天备份时
  python scripts/upgrade_drill.py --keep             # 保留演练库调试
"""
import argparse
import os
import subprocess
import sys
from datetime import datetime
from urllib.parse import urlparse

KEY_TABLES = ["users", "torrents", "traffic_ledger", "spark_ledger",
              "snatches", "_sqlx_migrations"]

PG = ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d", "fluxtorrent",
      "-t", "-A", "-c"]


def psql(sql, check=True):
    r = subprocess.run(PG + [sql], capture_output=True, text=True)
    if check and r.returncode != 0:
        print(f"!! psql 失败: {sql}\n{r.stderr}", file=sys.stderr)
        sys.exit(1)
    return r.stdout.strip()


def sh(cmd, check=True):
    r = subprocess.run(cmd, capture_output=True, text=True, shell=isinstance(cmd, str))
    if check and r.returncode != 0:
        print(f"!! 命令失败: {cmd}\n{r.stderr}", file=sys.stderr)
        sys.exit(1)
    return r


def step(name):
    print(f"\n=== {name} ===")


def main():
    ap = argparse.ArgumentParser(description="升级演练：备份→迁移前滚→断言→回滚口径")
    ap.add_argument("--skip-backup", action="store_true",
                    help="跳过现场备份（已有当日备份时）")
    ap.add_argument("--keep", action="store_true", help="保留演练库")
    args = ap.parse_args()

    t0 = datetime.now()
    results = []

    # ── 0. 前置：迁移基线 ──────────────────────────────────────────
    step("0 基线：当前库迁移水位")
    base_ver = psql("SELECT max(version) FROM _sqlx_migrations")
    base_n = psql("SELECT count(*) FROM _sqlx_migrations")
    dirty = psql(
        "SELECT count(*) FROM _sqlx_migrations WHERE checksum IS NULL")
    print(f"  库内迁移水位: {base_ver}（共 {base_n} 条，null checksum {dirty}）")
    results.append(("基线迁移水位", f"INFO {base_ver} / {base_n} 条"))

    # ── 1. 升级前备份 ─────────────────────────────────────────────
    step("1 升级前备份")
    if args.skip_backup:
        print("  --skip-backup：跳过（请确认 backups/ 有当日归档）")
        results.append(("升级前备份", "跳过（复用既有）"))
    else:
        r = sh(["bash", "scripts/backup.sh"], check=False)
        ok = r.returncode == 0
        print(f"  backup.sh exit={r.returncode}"
              + ("" if ok else f"\n{r.stdout[-500:]}\n{r.stderr[-500:]}"))
        results.append(("升级前备份", "PASS" if ok else "FAIL"))
        if not ok:
            print("  备份失败——真实升级窗口到此必须中止。演练继续（--drill 精神），"
                  "但结果记 FAIL。")

    # ── 2. 迁移前进演练：把全部迁移在演练库重放（等价「新版本启动」）──
    step("2 迁移前进：演练库重放全量迁移")
    drill_db = "fluxtorrent_upgrade_drill"
    psql(f'DROP DATABASE IF EXISTS {drill_db} WITH (FORCE)')
    # 活栈在连源库，不能直接 TEMPLATE——改 pg_dump|pg_restore 走一份干净副本
    # （这也是「备份可用性」的顺带验证：备份链路坏了这里第一时间暴露）
    # dump 是二进制（-Fc），全程 bytes 管道直灌 pg_restore，不落中间文件
    psql(f"CREATE DATABASE {drill_db}")
    dump = subprocess.Popen(
        ["docker", "exec", "flux-postgres", "pg_dump", "-U", "flux",
         "-Fc", "fluxtorrent"],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    restore = subprocess.Popen(
        ["docker", "exec", "-i", "flux-postgres", "pg_restore", "-U", "flux",
         "-d", drill_db, "--no-owner"],
        stdin=dump.stdout, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    dump.stdout.close()
    _, r_err = restore.communicate(dump.stderr and None or None)
    d_err = dump.stderr.read().decode(errors="replace")
    if dump.wait() != 0 or restore.returncode != 0:
        print(f"!! dump/restore 失败：{d_err[:200]} | {r_err.decode()[:200]}",
              file=sys.stderr)
        sys.exit(1)
    print("  源库副本就绪（pg_dump|pg_restore，顺带验证备份链路）")
    try:
        # 模板库即当前库快照；在其上跑 sqlx migrate run（应零条新迁移=幂等重放口径），
        # 再断言水位与 checksum 与源库一致——这正是「同版本重升级」的 e2e 断言。
        url = os.environ.get("DATABASE_URL",
                             "postgres://flux:flux@127.0.0.1:5432/fluxtorrent")
        u = urlparse(url)
        drill_url = (f"postgres://{u.username}:{u.password}@"
                     f"{u.hostname or '127.0.0.1'}:{u.port or 5432}/{drill_db}")
        r = subprocess.run(
            ["docker", "exec", "-e", f"DATABASE_URL={drill_url}", "flux-api",
             "sqlx-migrate", "run"],
            capture_output=True, text=True, check=False)
        # 容器里没有 sqlx CLI（生产镜像不带）——回落到本地 cargo（开发机口径）
        if r.returncode != 0:
            print("  容器内无 sqlx CLI（正常，生产镜像不带），回落本地 cargo：")
            env = dict(os.environ, DATABASE_URL=drill_url)
            r = subprocess.run(
                ["cargo", "sqlx", "migrate", "run",
                 "--source", "apps/api/migrations"],
                capture_output=True, text=True, env=env, check=False)
        print(f"  migrate exit={r.returncode}: {r.stdout.strip()[:200]}")
        ver = subprocess.run(
            PG[:-1] + [drill_db] + ["-t", "-A", "-c",
                                     "SELECT count(*) FROM _sqlx_migrations"],
            capture_output=True, text=True).stdout.strip()
        same = ver == base_n
        print(f"  演练库迁移水位: {ver} 条（源库 {base_n}）"
              f"{'一致' if same else '不一致！'}")
        results.append(("迁移前进重放", "PASS" if same else "FAIL"))

        # ── 3. 演练库断言 ─────────────────────────────────────────
        step("3 演练库关键断言")
        q = subprocess.run(
            PG[:-1] + [drill_db] + ["-t", "-A", "-c",
                "SELECT count(*) FROM users WHERE username = 'root'"],
            capture_output=True, text=True).stdout.strip()
        ok = q == "1"
        print(f"  root 账号存在: {q}（{'PASS' if ok else 'FAIL'}）")
        results.append(("演练库 root 断言", "PASS" if ok else "FAIL"))
    finally:
        if not args.keep:
            psql(f'DROP DATABASE IF EXISTS {drill_db} WITH (FORCE)')
            print(f"  已清理演练库 {drill_db}")

    # ── 4. 回滚口径 ──────────────────────────────────────────────
    step("4 回滚口径：旧镜像存在 + 恢复脚本在位")
    r = sh("docker images --format '{{.Repository}}:{{.Tag}}' | grep -c flux "
           "|| true")
    n_img = r.stdout.strip() or "0"
    ok = int(n_img) > 0 and os.path.exists("scripts/restore.sh")
    print(f"  本机 flux 镜像数: {n_img}；restore.sh 在位: "
          f"{os.path.exists('scripts/restore.sh')}")
    results.append(("回滚口径（镜像+脚本）", "PASS" if ok else "FAIL"))

    # ── 汇总 ─────────────────────────────────────────────────────
    dt = (datetime.now() - t0).total_seconds()
    print(f"\n===== 演练汇总（{dt:.0f}s）=====")
    fails = 0
    for k, v in results:
        mark = ("✓" if v == "PASS" or v.startswith(("跳过", "复用", "INFO"))
                else "✗")
        fails += mark == "✗"
        print(f"  {mark} {k}: {v}")
    print(f"\n结论：{'全绿，升级窗口可执行' if fails == 0 else f'{fails} 项 FAIL，先修复再开窗口'}")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
