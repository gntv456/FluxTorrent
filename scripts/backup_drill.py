#!/usr/bin/env python3
"""备份恢复演练（_doc/反作弊与性能加固方案.md P1）：
把 pg_dump 备份恢复到临时数据库，验证关键表与引导账号存在，最后清理。
不做任何写操作到原库；仅要求本机 psql/pg_restore 可用（或经 docker exec）。

用法：
  python scripts/backup_drill.py \
      --dump backups/flux_20260913.dump \
      --db fluxtorrent
  # 需要的 env：DATABASE_URL（原库，仅用于取连接参数）；演练库固定命名 <db>_drill_tmp
"""
import argparse
import os
import subprocess
import sys
from urllib.parse import urlparse

KEY_TABLES = ["users", "torrents", "traffic_ledger", "spark_ledger", "snatches"]


def run(cmd, env=None, check=True, input=None):
    r = subprocess.run(cmd, env=env, input=input, capture_output=True,
        text=True)
    if check and r.returncode != 0:
        print(f"!! 命令失败：{' '.join(cmd)}\n{r.stderr}", file=sys.stderr)
        sys.exit(1)
    return r


def main():
    ap = argparse.ArgumentParser(description="备份恢复演练")
    ap.add_argument("--dump", required=True,
        help="pg_dump 备份文件（custom/plain 均可）")
    ap.add_argument("--db", default=None, help="原库名（默认从 DATABASE_URL 解析）")
    ap.add_argument("--host", default=None)
    ap.add_argument("--port", default=None)
    ap.add_argument("--user", default=None)
    ap.add_argument("--keep", action="store_true", help="保留演练库（调试用）")
    args = ap.parse_args()

    url = os.environ.get("DATABASE_URL",
        "postgres://flux:flux@127.0.0.1:5432/fluxtorrent")
    u = urlparse(url)
    host = args.host or u.hostname or "127.0.0.1"
    port = args.port or str(u.port or 5432)
    user = args.user or u.username or "flux"
    src_db = args.db or u.path.lstrip("/") or "fluxtorrent"
    drill_db = f"{src_db}_drill_tmp"

    base_env = dict(os.environ, PGPASSWORD=os.environ.get("PGPASSWORD") or (
        u.password or ""))
    psql = ["psql", "-h", host, "-p", port, "-U", user, "-v", "ON_ERROR_STOP=1"]

    if not os.path.exists(args.dump):
        print(f"!! 备份文件不存在：{args.dump}", file=sys.stderr)
        sys.exit(1)

    print(f"[1/5] 删除残留演练库 {drill_db}（如存在）")
    run(psql + ["-d", "postgres", "-c",
        f'DROP DATABASE IF EXISTS "{drill_db}"'], env=base_env, check=False)

    print(f"[2/5] 创建空库 {drill_db}")
    run(psql + ["-d", "postgres", "-c", f'CREATE DATABASE "{drill_db}"'],
        env=base_env)

    print(f"[3/5] 恢复备份（pg_restore/pg_dump 直灌）")
    restore = ["pg_restore", "-h", host, "-p", port, "-U", user, "-d",
        drill_db, "--no-owner", args.dump]
    r = run(restore, env=base_env, check=False)
    if r.returncode != 0:
        # custom 格式失败 → 尝试按 plain SQL 直灌
        print("  pg_restore 失败，改用 plain SQL 直灌…")
        with open(args.dump, "r", encoding="utf-8") as f:
            run(psql + ["-d", drill_db], env=base_env, input=f.read())

    print("[4/5] 验证关键表与引导数据")
    for t in KEY_TABLES:
        r = run(psql + ["-d", drill_db, "-tAc", f"SELECT count(*) FROM {t}"],
            env=base_env, check=False)
        cnt = r.stdout.strip() if r.returncode == 0 else "FAIL"
        print(f"    {t}: {cnt} 行")
        if cnt == "FAIL":
            print(f"!! 关键表 {t} 缺失，恢复失败", file=sys.stderr)
            sys.exit(1)
    r = run(psql + ["-d", drill_db, "-tAc",
        "SELECT count(*) FROM users WHERE class_id >= 90"], env=base_env,
            check=False)
    print(f"    管理员账号: {r.stdout.strip()} 个")
    # 分区完整性：三张流水表都应能查（分区父表可查即结构完好）
    run(psql + ["-d", drill_db, "-tAc", "SELECT count(*) FROM traffic_ledger"],
        env=base_env)

    print("[5/5] 清理")
    if not args.keep:
        run(psql + ["-d", "postgres", "-c", f'DROP DATABASE "{drill_db}"'],
            env=base_env)
        print(f"    演练库 {drill_db} 已删除")
    else:
        print(f"    演练库 {drill_db} 保留（--keep）")

    print("演练通过：备份可用、结构完整。RTO ≈ 步骤 [3] 耗时，请记录。")


if __name__ == "__main__":
    main()
