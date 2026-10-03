#!/usr/bin/env bash
#
# offline-bundle.sh —— 离线部署打包器
#
# 适用：目标服务器无外网 / Docker Hub 抽风时，在有网的「打包机」上把
#       全部镜像构建并导出成一个 tar，拷到目标机 docker load 即用，
#       全程目标机零 registry 接触（对标 NP「整包拷到服务器」的体验）。
#
# 用法（在打包机上）：
#   ./scripts/offline-bundle.sh
#   ./scripts/offline-bundle.sh my-bundle.tar      # 指定输出文件名
#
# 产物：一个 tar，包含 api/worker/tracker/web 四个应用镜像 +
#       postgres:16-alpine + redis:7-alpine（compose 里显式引用的两个）。
#       监控镜像（prometheus/grafana）默认不打，需要的话手动 docker pull 追加。
#
set -euo pipefail

cd "$(dirname "$0")/.."            # 切到仓库根
COMPOSE="docker/docker-compose.yml"

# 读取 .env 里的镜像前缀/版本（与 compose 默认值保持一致）
if [ -f docker/.env ]; then
  PREFIX="$(grep -E '^FLUX_IMAGE_PREFIX=' docker/.env | head -1 | cut -d= -f2-)"
  VERSION="$(grep -E '^FLUX_VERSION=' docker/.env | head -1 | cut -d= -f2-)"
fi
PREFIX="${PREFIX:-ghcr.io/gntv456/fluxtorrent}"
VERSION="${VERSION:-latest}"

echo "==> 镜像前缀: $PREFIX   版本: $VERSION"

echo "==> [1/3] 构建应用镜像 (api/worker/tracker/web) …"
docker compose -f "$COMPOSE" build

echo "==> [2/3] 拉取数据镜像 (postgres/redis) …"
docker pull postgres:16-alpine
docker pull redis:7-alpine

OUT="${1:-fluxtorrent-offline-${VERSION}-$(date +%Y%m%d).tar}"
echo "==> [3/3] 导出镜像到 $OUT …"
docker save \
  "$PREFIX/api:$VERSION" \
  "$PREFIX/worker:$VERSION" \
  "$PREFIX/tracker:$VERSION" \
  "$PREFIX/web:$VERSION" \
  postgres:16-alpine \
  redis:7-alpine \
  -o "$OUT"

echo
echo "✅ 打包完成: $OUT"
echo "   体积: $(du -h "$OUT" | cut -f1)"
echo "   传输到目标机后执行："
echo "     docker load -i $OUT"
echo "     docker compose -f $COMPOSE up -d      # 注意：不要加 --build"
echo
echo "   ⚠️ 目标机的 docker/.env 里 FLUX_IMAGE_PREFIX / FLUX_VERSION 要与本机一致，"
echo "      否则 compose 会因 tag 对不上而去尝试拉取。"
