#!/usr/bin/env bash
# FluxTorrent 每日备份（cron 示例：0 4 * * * /path/backup.sh）
# 用法：./backup.sh [保留天数，默认 14]
# 覆盖：PostgreSQL 全量 + 附件卷（.torrent 原始文件在 DB 内，随 pg_dump 覆盖）。
# Redis（ZT81 2026-10-02 修正）：**不再是「均可再生」**——flux:announce 事件流与
# flux:announce:dlq 死信是**未计费的账**，丢了就对不上账且无证据链。故一并备份 RDB。
#
# Git Bash（Windows）注意：MSYS 会把传给 docker exec 的 /app 等绝对路径
# 改写成 C:/Program Files/Git/app 导致容器内找不到——必须在执行前关闭路径转换。
export MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL="*"
set -euo pipefail
KEEP_DAYS="${1:-14}"
STAMP=$(date +%F-%H%M%S)
OUT_DIR="${FLUX_BACKUP_DIR:-./backups}"
mkdir -p "$OUT_DIR"

# PG 全量（含 schema + 数据 + _sqlx_migrations 迁移记账；-Fc 自定义压缩格式，
# pg_restore 可选表恢复）
docker exec flux-postgres pg_dump -U flux -Fc fluxtorrent > "$OUT_DIR/fluxtorrent-$STAMP.dump"
echo "[backup] $OUT_DIR/fluxtorrent-$STAMP.dump ($(du -h "$OUT_DIR/fluxtorrent-$STAMP.dump" | cut -f1))"

# 附件卷（attachments named volume 挂在 api 容器 /app/attachments；api 未运行时跳过）
if docker ps --format '{{.Names}}' | grep -qx flux-api; then
  if docker exec flux-api sh -c 'test -d /app/attachments' 2>/dev/null; then
    docker exec flux-api tar czf - -C /app attachments \
      > "$OUT_DIR/attachments-$STAMP.tar.gz"
    echo "[backup] $OUT_DIR/attachments-$STAMP.tar.gz ($(du -h "$OUT_DIR/attachments-$STAMP.tar.gz" | cut -f1))"
  fi
else
  echo "[backup] WARN: flux-api 容器未运行，本轮未备份附件卷"
fi

# Redis 事件流/DLQ 快照（BGSAVE 后拷 RDB）。计费未落库的事件在此，必须进备份。
if docker ps --format '{{.Names}}' | grep -qx flux-redis; then
  docker exec flux-redis redis-cli -a "$REDIS_PASSWORD" --no-auth-warning \
    BGSAVE >/dev/null 2>&1 || true
  # 等 BGSAVE 落盘（最多 10s），再拷出 dump.rdb
  for _ in $(seq 1 10); do
    if docker exec flux-redis redis-cli -a "$REDIS_PASSWORD" --no-auth-warning \
      INFO persistence 2>/dev/null | grep -q 'rdb_bgsave_in_progress:0'; then
      break
    fi
    sleep 1
  done
  if docker exec flux-redis sh -c 'test -f /data/dump.rdb' 2>/dev/null; then
    docker exec flux-redis sh -c 'cat /data/dump.rdb' \
      > "$OUT_DIR/redis-$STAMP.rdb"
    echo "[backup] $OUT_DIR/redis-$STAMP.rdb ($(du -h "$OUT_DIR/redis-$STAMP.rdb" | cut -f1))"
  fi
else
  echo "[backup] WARN: flux-redis 容器未运行，本轮未备份事件流"
fi

# 轮转保留
find "$OUT_DIR" -name "fluxtorrent-*.dump" -mtime +$KEEP_DAYS -delete
find "$OUT_DIR" -name "attachments-*.tar.gz" -mtime +$KEEP_DAYS -delete
find "$OUT_DIR" -name "redis-*.rdb" -mtime +$KEEP_DAYS -delete
echo "[backup] rotated, keeping last $KEEP_DAYS days"
