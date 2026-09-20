#!/usr/bin/env bash
# FluxTorrent 每日备份（cron 示例：0 4 * * * /path/to/backup.sh）
# 用法：./backup.sh [保留天数，默认 14]
# 覆盖：PostgreSQL 全量 + 附件卷（.torrent 原始文件在 DB 内，随 pg_dump 覆盖）。
# Redis 不备份：限流/缓存均可再生。
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

# 轮转保留
find "$OUT_DIR" -name "fluxtorrent-*.dump" -mtime +$KEEP_DAYS -delete
find "$OUT_DIR" -name "attachments-*.tar.gz" -mtime +$KEEP_DAYS -delete
echo "[backup] rotated, keeping last $KEEP_DAYS days"
