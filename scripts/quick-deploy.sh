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

# 冷启动绑定管理（WEB_BIND）：向导未完成 → web 临时绑 0.0.0.0（外网浏览器
# 要能开 :3000/setup）；已完成 → 删除该行收回 127.0.0.1。幂等可重跑。
# 探测用 API 端口以 .env 实际值为准（端口自检可能已改）。
API_PORT="$(grep -E '^FLUX_API_PORT=' docker/.env | cut -d= -f2 || true)"
API_PORT="${API_PORT:-8080}"
if curl -fsS "http://localhost:${API_PORT}/api/v1/setup/status" 2>/dev/null \
   | grep -q '"done": *true'; then
  if grep -qE "^WEB_BIND=0.0.0.0" docker/.env; then
    sed -i '/^WEB_BIND=0.0.0.0/d' docker/.env
    echo ">> 向导已完成：WEB_BIND 收回为默认 127.0.0.0（外网不再直连 3000）"
  fi
else
  if ! grep -qE "^WEB_BIND=" docker/.env; then
    echo "WEB_BIND=0.0.0.0" >> docker/.env
    echo ">> 首次安装：WEB_BIND 临时 0.0.0.0（向导完成后重跑本脚本自动收回）"
  fi
fi

# 端口自检：同机已有服务抢端口时自动改用备选端口（写进 .env，幂等——
# 已写过的 FLUX_*_PORT 不覆盖；未冲突的端口不动）。容器互联走内部网络，
# 换宿主端口不影响功能，只影响「从宿主机直连调试」用的地址。
PORTS_TAKEN=""
pick_port() { # $1=检测函数名惯用标签 $2=env键 $3=默认端口 $4=备选端口
  local env_key="$2" default_port="$3" alt_port="$4"
  if grep -qE "^${env_key}=" docker/.env; then
    return 0 # 站长已手动指定，尊重
  fi
  if ss -lntH "sport = :${default_port}" 2>/dev/null | grep -q .; then
    echo "${env_key}=${alt_port}" >> docker/.env
    PORTS_TAKEN="${PORTS_TAKEN} ${default_port}→${alt_port}"
  fi
}
echo ">> 端口自检 ..."
command -v ss >/dev/null 2>&1 || { apt-get install -y iproute2 >/dev/null 2>&1 || true; }
if command -v ss >/dev/null 2>&1; then
  pick_port db FLUX_DB_PORT 5432 15432
  pick_port redis FLUX_REDIS_PORT 6379 16379
  pick_port api FLUX_API_PORT 8080 18080
  pick_port web FLUX_WEB_PORT 3000 13000
  pick_port tracker FLUX_TRACKER_PORT 7070 17070
  if [[ -n "$PORTS_TAKEN" ]]; then
    echo ">> 检测到端口被占，已自动改用：${PORTS_TAKEN}（已写入 docker/.env，可手动调整）"
  fi
else
  echo "!! ss 不可用，跳过端口自检（起栈失败时多为端口冲突，见文档 FAQ）"
fi

# 起栈
echo ">> 启动容器 ..."
docker compose -f docker/docker-compose.yml up -d

# 等几秒再探活（端口可能被自检改过，以 .env 实际值为准）
API_PORT="$(grep -E '^FLUX_API_PORT=' docker/.env | cut -d= -f2 || true)"
API_PORT="${API_PORT:-8080}"
WEB_PORT="$(grep -E '^FLUX_WEB_PORT=' docker/.env | cut -d= -f2 || true)"
WEB_PORT="${WEB_PORT:-3000}"
sleep 8
if curl -fsS "http://localhost:${API_PORT}/api/v1/health" >/dev/null 2>&1; then
  echo "✅ 健康检查通过"
else
  echo "⚠️ 健康检查未通过，运行 docker ps 和 docker logs 排查"
fi

echo
echo "===== 下一步（无脑跟着做）====="
echo "1. 浏览器打开  http://<这台服务器IP>:${WEB_PORT}/setup  完成四步安装向导"
echo "   （首次安装若打不开：docker/.env 里 WEB_BIND 改 0.0.0.0 并重跑 up -d，"
echo "     云安全组放行 ${WEB_PORT}/TCP；配好域名后改回 127.0.0.1）"
echo "2. 域名 + HTTPS：宝塔/1Panel 建站反代 + 证书后，跑一键补全（G.4 自动化）："
echo "     bash scripts/reverse-proxy.sh --domain 你的域名"
echo "   （自动补 /announce/ 与 /api/ 反代 + 体检；没有面板走 domain-deploy.md）"
echo "3. 不想配域名也能先用 IP 访问；但正式开站务必配，否则用户做不了种"
echo "详细图文：docs/webmaster/build-tutorial.md（SSH 方式）/ deploy-1panel.md / deploy-baota.md"
