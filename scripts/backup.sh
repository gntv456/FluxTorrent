#!/usr/bin/env bash
# FluxTorrent 数据库每日备份（cron 示例：0 4 * * * /path/to/backup.sh）
# 用法：./backup.sh [保留天数，默认 14]
set -euo pipefail
KEEP_DAYS="${1:-14}"
STAMP=$(date +%F-%H%M%S)
OUT_DIR="${FLUX_BACKUP_DIR:-./backups}"
mkdir -p "$OUT_DIR"

# PG 全量（含 schema + 数据；-Fc 自定义压缩格式，pg_restore 可选表恢复）
docker exec flux-postgres pg_dump -U flux -Fc fluxtorrent > "$OUT_DIR/fluxtorrent-$STAMP.dump"
echo "[backup] $OUT_DIR/fluxtorrent-$STAMP.dump ($(du -h "$OUT_DIR/fluxtorrent-$STAMP.dump" | cut -f1))"

# Redis 只备份配置相关键（限流/缓存可再生，不备份）
# 轮转保留
find "$OUT_DIR" -name "fluxtorrent-*.dump" -mtime +$KEEP_DAYS -delete
echo "[backup] rotated, keeping last $KEEP_DAYS days"
