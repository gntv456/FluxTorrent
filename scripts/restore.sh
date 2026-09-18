#!/usr/bin/env bash
# FluxTorrent 数据库恢复 runbook（危险操作：写库前必须人工确认）
#
# 用法：
#   恢复到临时库验证：  ./restore.sh <dump 文件> --drill
#   恢复到生产库：      ./restore.sh <dump 文件> --force   （需输入 YES 确认）
#
# 与 backup.sh / admin/backups/run 同口径：docker exec flux-postgres、-Fc 自定义格式。
# 恢复 = 整库替换（--clean 先 DROP 再 CREATE），执行前自动停 api/worker/tracker 写入
# 由运维手动执行 docker compose stop api worker tracker —— 脚本会检测并提醒。
set -euo pipefail

DUMP="${1:?用法: ./restore.sh <dump文件> [--drill|--force]}"
MODE="${2:---drill}"

[ -f "$DUMP" ] || { echo "[restore] 文件不存在: $DUMP"; exit 1; }

PG_CONTAINER="${FLUX_PG_CONTAINER:-flux-postgres}"
PG_USER="${FLUX_PG_USER:-flux}"
PG_DB="${FLUX_PG_DB:-fluxtorrent}"

run_pg() { docker exec -i "$PG_CONTAINER" "$@"; }

case "$MODE" in
  --drill)
    DRILL_DB="fluxtorrent_drill_$$"
    echo "[restore][drill] 恢复到临时库 $DRILL_DB 验证完整性（不动生产库）"
    run_pg psql -U "$PG_USER" -d postgres -c "CREATE DATABASE \"$DRILL_DB\";"
    if ! run_pg pg_restore -U "$PG_USER" -d "$DRILL_DB" --no-owner "$DUMP"; then
      run_pg psql -U "$PG_USER" -d postgres -c "DROP DATABASE \"$DRILL_DB\";"
      echo "[restore][drill] ✗ pg_restore 失败：dump 损坏或版本不兼容，禁止用于生产恢复"
      exit 1
    fi
    TABLES=$(run_pg psql -U "$PG_USER" -d "$DRILL_DB" -tAc \
      "SELECT count(*) FROM information_schema.tables WHERE table_schema='public'")
    USERS=$(run_pg psql -U "$PG_USER" -d "$DRILL_DB" -tAc "SELECT count(*) FROM users")
    echo "[restore][drill] ✓ 校验通过：public 表 $TABLES 张 / users $USERS 行"
    run_pg psql -U "$PG_USER" -d postgres -c "DROP DATABASE \"$DRILL_DB\";"
    echo "[restore][drill] 完成。确认无误后执行: ./restore.sh $DUMP --force"
    ;;
  --force)
    echo "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!"
    echo " 生产恢复将【整库替换】$PG_DB，当前数据全部丢失！"
    echo " 前置检查："
    echo "   1) 已停止写入：docker compose stop api worker tracker"
    echo "   2) 已对当前库做安全备份：./backup.sh"
    echo "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!"
    read -r -p "输入 YES 继续生产恢复: " CONFIRM
    [ "$CONFIRM" = "YES" ] || { echo "已取消"; exit 1; }
    echo "[restore][force] ① 先对当前库做回滚备份 safety-$(date +%F-%H%M%S).dump"
    docker exec "$PG_CONTAINER" pg_dump -U "$PG_USER" -Fc "$PG_DB" \
      > "${FLUX_BACKUP_DIR:-./backups}/safety-$(date +%F-%H%M%S).dump"
    echo "[restore][force] ② 整库恢复（--clean 先删后建）"
    run_pg pg_restore -U "$PG_USER" -d "$PG_DB" --clean --if-exists --no-owner "$DUMP"
    echo "[restore][force] ③ 完成。重启服务：docker compose start api worker tracker"
    echo "[restore][force] ④ 验证：/api/v1/health + 抽查 users/torrents 计数"
    ;;
  *)
    echo "未知模式: $MODE（支持 --drill / --force）"; exit 1
    ;;
esac
