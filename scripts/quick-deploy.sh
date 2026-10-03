#!/usr/bin/env bash
#
# quick-deploy.sh —— 无脑部署（服务器侧）
#
# 一条命令完成：可选装 Docker → 拉代码 → 自动生成 .env 三项机密 → 起栈。
# 跑完只需要去浏览器开 http://服务器IP:3000/setup 走网页向导，再按域名部署文档配 HTTPS。
#
# 用法（任选其一）：
#   1) 直接跑（已装 Docker）：
#        bash quick-deploy.sh
#   2) 让脚本顺手装 Docker（Ubuntu/Debian）：
#        bash quick-deploy.sh --install-docker
#   3) 远程一键（从 GitHub 拉本脚本执行）：
#        curl -fsSL https://github.com/gntv456/FluxTorrent/raw/master/scripts/quick-deploy.sh | sudo bash -s -- --install-docker
#
# 可选参数：
#   --install-docker   自动安装 Docker（仅 Ubuntu/Debian 测过）
#   --domain 域名      顺便把 CORS_ORIGINS 设成 https://域名（域名 HTTPS 仍需另配）
#
set -euo pipefail

INSTALL_DOCKER=0
DOMAIN=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --install-docker) INSTALL_DOCKER=1 ;;
    --domain) DOMAIN="${2:-}"; shift ;;
    *) echo "未知参数: $1" >&2; exit 1 ;;
  esac
  shift
done

echo "=== FluxTorrent 无脑部署（服务器侧）==="

if [[ "$INSTALL_DOCKER" -eq 1 ]]; then
  echo ">> 安装 Docker ..."
  curl -fsSL https://get.docker.com | sudo sh
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "✗ docker 不存在，请先安装（加 --install-docker 让本脚本装）" >&2
  exit 1
fi

# 拉代码（已存在就更新）
if [[ -d fluxtorrent/.git ]]; then
  echo ">> 更新已有仓库 ..."
  git -C fluxtorrent pull --ff-only
else
  echo ">> 克隆仓库 ..."
  git clone https://github.com/gntv456/FluxTorrent.git fluxtorrent
fi
cd fluxtorrent

# 生成 .env（仅首次）
if [[ ! -f docker/.env ]]; then
  cp docker/.env.example docker/.env
fi

# 自动生成三项机密（只在还是占位符时替换，重跑不覆盖你已设的值）
DB_PASSWORD="$(openssl rand -base64 12 | tr -dc 'A-Za-z0-9' | head -c 16)"
REDIS_PASSWORD="$(openssl rand -base64 12 | tr -dc 'A-Za-z0-9' | head -c 16)"
JWT_SECRET="$(openssl rand -base64 48)"
sed -i "s|^DB_PASSWORD=change_me.*|DB_PASSWORD=$DB_PASSWORD|" docker/.env
sed -i "s|^REDIS_PASSWORD=change_me.*|REDIS_PASSWORD=$REDIS_PASSWORD|" docker/.env
sed -i "s|^JWT_SECRET=change_me.*|JWT_SECRET=$JWT_SECRET|" docker/.env

if [[ -n "$DOMAIN" ]]; then
  sed -i "s|^CORS_ORIGINS=.*|CORS_ORIGINS=https://$DOMAIN|" docker/.env
  echo ">> CORS_ORIGINS 已设为 https://$DOMAIN"
fi

# 起栈
echo ">> 启动容器 ..."
docker compose -f docker/docker-compose.yml up -d

# 等几秒再探活
sleep 8
if curl -fsS http://localhost:8080/api/v1/health >/dev/null 2>&1; then
  echo "✅ 健康检查通过"
else
  echo "⚠️ 健康检查未通过，运行 docker ps 和 docker logs 排查"
fi

echo
echo "===== 下一步（无脑跟着做）====="
echo "1. 浏览器打开  http://<这台服务器IP>:3000/setup  完成四步安装向导"
echo "2. 按 docs/webmaster/domain-deploy.md 配域名 + HTTPS（含把 announce 改成域名）"
echo "3. 不想配域名也能先用 IP 访问；但正式开站务必配，否则用户做不了种"
echo "详细图文：docs/webmaster/build-tutorial.md（SSH 方式）/ deploy-1panel.md / deploy-baota.md"
