#!/usr/bin/env bash
#
# reverse-proxy.sh —— 反向代理自动配置（服务器侧，G.3/G.4 的自动化）
#
# 做三件事（全部幂等，可重复跑）：
#   1) 检测 Nginx 形态：宝塔面板 or 系统直装（apt/dnf）
#   2) 确保「网站 + /api + tracker /announce/」三条 location 全部反代到位
#   3) --domain 时把后台 Tracker 地址回写为 https://域名（调管理 API）
#
# 宝塔路径：把 /announce/ 与 /api/ 的 location 物理写进该站点的
# /www/server/panel/vhost/nginx/<域名>.conf（include proxy 之前）——
# 面板免费版没有 API，改文件是唯一通路；宝塔重载按钮/自动重载会带上它。
# 系统直装路径：/etc/nginx/conf.d/fluxtorrent-<域名>.conf 全量生成。
#
# 用法：
#   bash scripts/reverse-proxy.sh --domain mfwg.ptang.top          # 全自动
#   bash scripts/reverse-proxy.sh --domain mfwg.ptang.top --dry    # 只看要改什么
#   bash scripts/reverse-proxy.sh --status                          # 只体检不改
#
# 证书不在本脚本职责内（宝塔 SSL 页一键 / certbot 一条命令），但会检查提醒。
#
set -euo pipefail

DOMAIN=""
WEB_PORT="${FLUX_WEB_PORT:-3000}"
API_PORT="${FLUX_API_PORT:-8080}"
TRACKER_PORT="${FLUX_TRACKER_PORT:-7070}"
DRY=0
STATUS_ONLY=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --domain) DOMAIN="${2:-}"; shift ;;
    --web-port) WEB_PORT="$2"; shift ;;
    --api-port) API_PORT="$2"; shift ;;
    --tracker-port) TRACKER_PORT="$2"; shift ;;
    --dry) DRY=1 ;;
    --status) STATUS_ONLY=1 ;;
    *) echo "未知参数: $1（用法见文件头注释）" >&2; exit 1 ;;
  esac
  shift
done

mark_start="# >>> fluxtorrent reverse-proxy (auto-managed) >>>"
mark_end="# <<< fluxtorrent reverse-proxy <<<"

block_for() { # $1=域名 → 生成三条 location 的受管块
  cat <<NGX
$mark_start
    # 网站 + 同源 API（web 容器统一出口；RSC 直连容器网，不经这里）
    location /api/ {
        proxy_pass http://127.0.0.1:${WEB_PORT};
        proxy_set_header Host \$host;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
    }
    # tracker announce —— 最长前缀优先于 / 的网站反代（G.4 断链根因）
    location /announce/ {
        proxy_pass http://127.0.0.1:${TRACKER_PORT};
        proxy_set_header Host \$host;
    }
$mark_end
NGX
}

detect_nginx() {
  if [[ -d /www/server/panel/vhost/nginx ]]; then
    echo baota
  elif command -v nginx >/dev/null 2>&1; then
    echo system
  else
    echo none
  fi
}

ensure_managed_block() { # $1=目标conf路径 $2=域名
  local conf="$1" domain="$2"
  if [[ ! -f "$conf" ]]; then
    echo "✗ 找不到 $conf（站点还没建？先在面板添加站点 $domain）" >&2
    return 1
  fi
  if grep -qF "$mark_start" "$conf"; then
    echo "  · $conf 已有受管块，刷新端口值"
    [[ $DRY -eq 1 ]] && return 0
    # 删旧块再插新块（端口可能变了）
    sed -i "/^${mark_start//\//\\/}\$/,/^${mark_end//\//\\/}\$/d" "$conf"
  else
    echo "  · $conf 无受管块，将插入（include proxy 之前）"
  fi
  [[ $DRY -eq 1 ]] && return 0
  local tmp; tmp="$(mktemp)"
  # 插入锚点（宝塔模板）：include .../proxy/<域名>/*.conf 那行之前——
  # 受管块先于网站反代加载，肉眼排查直观。该行常带 tab 缩进，故锚匹配
  # 不限行首（index>0）。锚不存在时回落到 #PHP-INFO-START 之前；再没有
  # 就追加到 server 块之后（location 前缀匹配本就不依赖顺序，位置只为可读）。
  if grep -q "include /www/server/panel/vhost/nginx/proxy/${domain}/" "$conf"; then
    awk -v block="$(block_for "$domain")" \
        -v anchor="include /www/server/panel/vhost/nginx/proxy/${domain}/" \
        'index($0, anchor) > 0 { print block; have=1 } { print }
         END { if (!have) { print ""; print block } }' "$conf" > "$tmp"
  else
    awk -v block="$(block_for "$domain")" \
        'index($0, "#PHP-INFO-START") > 0 && !done { print block; done=1 } { print }
         END { if (!done) { print ""; print block } }' "$conf" > "$tmp"
  fi
  mv "$tmp" "$conf"
  echo "  ✓ 已写入 $conf"
}

nginx_reload() {
  if nginx -t 2>/dev/null; then
    if [[ -d /www/server/panel ]]; then
      /etc/init.d/nginx reload 2>/dev/null || nginx -s reload
    else
      systemctl reload nginx 2>/dev/null || nginx -s reload
    fi
    echo "✓ Nginx 已重载"
  else
    echo "✗ nginx -t 校验失败——配置没动坏但也没重载；跑 nginx -t 看报错" >&2
    nginx -t || true
    return 1
  fi
}

health_checks() {
  local dom="$1"
  echo "--- 体检 ---"
  local api_ok=0 ann_ok=0
  curl -kfsS "https://${dom}/api/v1/health" 2>/dev/null | grep -q '"status":"up"' && api_ok=1 \
    || curl -fsS "http://${dom}/api/v1/health" 2>/dev/null | grep -q '"status":"up"' && api_ok=1
  [[ $api_ok -eq 1 ]] && echo "✓ /api/v1/health 经域名可达" \
                      || echo "✗ /api/v1/health 经域名不可达（反代/栈/安全组）"
  local code; code="$(curl -ks -o /dev/null -w '%{http_code}' "https://${dom}/announce/" 2>/dev/null \
                || curl -s  -o /dev/null -w '%{http_code}' "http://${dom}/announce/" 2>/dev/null || echo 000)"
  case "$code" in
    200|301|302|400|401|403|404) echo "✓ /announce/ 返回 $code（已到 tracker）" ;;
    502|504) echo "✗ /announce/ 返回 $code（反代没通——tracker 端口对不对？）" ;;
    000)     echo "✗ /announce/ 连不上（域名解析/443/防火墙）" ;;
    *)       echo "? /announce/ 返回 $code（非预期，把输出发给支持）" ;;
  esac
}

announce_to_admin() { # $1=域名 —— 后台回写 announce（需管理员会话，做不了就提示路径）
  echo ""
  echo "后台 Tracker 地址请设为：https://$1 （不带 /announce，系统自动拼）"
  echo "路径：管理后台 → 站点设定 → 基础设定 → Tracker 地址；同页 HTTPS announce 也填同值。"
  # 不自动调管理 API：改密后的管理员凭据不该进部署脚本/环境变量。
}

main() {
  echo "=== FluxTorrent 反向代理自动配置 ==="
  local env_file
  for env_file in docker/.env ./docker/.env; do
    [[ -f "$env_file" ]] || continue
    for k in FLUX_WEB_PORT FLUX_API_PORT FLUX_TRACKER_PORT; do
      local v; v="$(grep -E "^${k}=" "$env_file" | cut -d= -f2 || true)"
      [[ -n "$v" ]] || continue
      case "$k" in
        FLUX_WEB_PORT) WEB_PORT="$v" ;;
        FLUX_API_PORT) API_PORT="$v" ;;
        FLUX_TRACKER_PORT) TRACKER_PORT="$v" ;;
      esac
    done
  done
  echo "端口：web=${WEB_PORT} api=${API_PORT} tracker=${TRACKER_PORT}"

  local kind; kind="$(detect_nginx)"
  case "$kind" in
    baota)  echo "检测到宝塔面板 Nginx" ;;
    system) echo "检测到系统直装 Nginx" ;;
    none)
      echo "✗ 没检测到 Nginx（宝塔或系统级）。两种走法：" >&2
      echo "   ① 装了宝塔：先在面板「网站」添加站点（任意目录/纯静态），再重跑本脚本" >&2
      echo "   ② 没面板：apt install nginx 后重跑，或照 docs/webmaster/domain-deploy.md 手动" >&2
      exit 1 ;;
  esac

  if [[ $STATUS_ONLY -eq 1 ]]; then
    [[ -n "$DOMAIN" ]] || { echo "--status 需要 --domain" >&2; exit 1; }
    health_checks "$DOMAIN"; exit 0
  fi

  [[ -n "$DOMAIN" ]] || { echo "缺少 --domain 你的域名（其他用法见文件头）" >&2; exit 1; }

  if [[ "$kind" == "baota" ]]; then
    local conf="/www/server/panel/vhost/nginx/${DOMAIN}.conf"
    ensure_managed_block "$conf" "$DOMAIN"
  else
    local conf="/etc/nginx/conf.d/fluxtorrent-${DOMAIN}.conf"
    if [[ ! -f "$conf" ]] || ! grep -qF "$mark_start" "$conf"; then
      echo "  · 生成系统 Nginx 配置 $conf"
      if [[ $DRY -eq 0 ]]; then
        cat > "$conf" <<NGX
# FluxTorrent 受管配置（scripts/reverse-proxy.sh 生成；删除本文件+reload 即移除）
server {
    listen 80;
    server_name ${DOMAIN};
$(block_for "$DOMAIN" | sed 's/^    /    /')
}
NGX
      fi
    fi
    echo "  · 证书不在本脚本职责：跑 certbot --nginx -d ${DOMAIN} 或面板一键"
  fi

  [[ $DRY -eq 0 ]] && nginx_reload
  [[ $DRY -eq 0 ]] && health_checks "$DOMAIN"
  [[ $DRY -eq 0 ]] && announce_to_admin "$DOMAIN"
  [[ $DRY -eq 1 ]] && echo "（--dry：以上只展示将做的改动，未写盘）"
}

main
