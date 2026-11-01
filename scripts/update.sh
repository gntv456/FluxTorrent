#!/usr/bin/env bash
#
# update.sh —— 一条命令升级 FluxTorrent（服务器侧）
#
# 做四件事：拉预构建镜像 → 核对迁移方向 → 切服务 → 健康检查，失败自动回滚。
# 全程不重建镜像（那是 15~25 分钟），正常情况下 1~3 分钟即可完成。
#
# 用法：
#   bash scripts/update.sh              # 升级到 GitHub 最新 Release
#   bash scripts/update.sh --check      # 只看不动：当前版本 / 最新版本 / 迁移方向
#   bash scripts/update.sh --tag v0.5.3 # 指定版本（填当前线上版本即空跑）
#   bash scripts/update.sh --yes        # 免确认（定时任务 / 非交互）
#
# 环境变量（一般不用动）：
#   FLUX_REPO       默认 gntv456/FluxTorrent，从 GitHub 查最新 Release 用
#   FLUX_SERVICES   默认 "api worker tracker web"
#   FLUX_PUBLIC_URL 健康检查的站点地址，默认按 .env 的 FLUX_WEB_PORT 拼
#                   127.0.0.1（没改端口就是 :3000）
#   FLUX_API_HEALTH 健康检查的 api 地址，默认按 .env 的 FLUX_API_PORT 拼
#                   127.0.0.1（没改端口就是 :8080/api/v1/health）
#
# 为什么有「迁移守卫」：api 启动就跑迁移，库里已应用到第 N 号而镜像里只到
# M<N 时，sqlx 直接拒绝启动（"migration N was previously applied but is
# missing"），表现为崩溃循环 + 全站接口 500。守卫在切换**之前**把方向判出来，
# 并把「怎么修」写在报错里，避免把线上弄进那个状态。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE="$ROOT/docker/docker-compose.yml"
ENVFILE="$ROOT/docker/.env"
STATE="$ROOT/docker/.flux-version"
SERVICES="${FLUX_SERVICES:-api worker tracker web}"
REPO="${FLUX_REPO:-gntv456/FluxTorrent}"
PUBLIC_URL="${FLUX_PUBLIC_URL:-http://127.0.0.1:3000}"

# 健康检查的端口跟着 compose 走：api/web 的宿主端口是 ${FLUX_API_PORT:-8080}
# /${FLUX_WEB_PORT:-3000}，站长改过端口还硬编码 8080，健康检查必然失败，
# 脚本会把一次好端端的升级「回滚」掉——假失败比没检查更糟。
envget() { sed -n "s/^$1=//p" "$ENVFILE" 2>/dev/null | head -1 | tr -d '\r'; }
API_PORT="${FLUX_API_PORT:-$(envget FLUX_API_PORT)}"
API_HEALTH="${FLUX_API_HEALTH:-http://127.0.0.1:${API_PORT:-8080}/api/v1/health}"
WEB_PORT="${FLUX_WEB_PORT:-$(envget FLUX_WEB_PORT)}"
if [[ -z "${FLUX_PUBLIC_URL:-}" && -n "$WEB_PORT" && "$WEB_PORT" != "3000" ]]; then
  PUBLIC_URL="http://127.0.0.1:$WEB_PORT"
fi

TAG=""
CHECK=0
YES=0
WITH_ROLLBACK=1
while [[ $# -gt 0 ]]; do
  case "$1" in
    --check) CHECK=1 ;;
    --yes|-y) YES=1 ;;
    --tag) TAG="${2:-}"; shift ;;
    --no-rollback) WITH_ROLLBACK=0 ;;
    -h|--help) sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "未知参数: $1（--help 看用法）" >&2; exit 2 ;;
  esac
  shift
done

say() { printf '\n\033[1m== %s\033[0m\n' "$*"; }
ok()  { printf '  \033[32m✓\033[0m %s\n' "$*"; }
warn(){ printf '  \033[33m!\033[0m %s\n' "$*"; }
die() { printf '\n\033[31m✗ %s\033[0m\n' "$*" >&2; exit 1; }

[[ -f "$COMPOSE" ]] || die "找不到 $COMPOSE（脚本要在仓库的 scripts/ 下）"
[[ -f "$ENVFILE" ]] || die "找不到 $ENVFILE（先按 docker/.env.example 配一份）"
docker compose version >/dev/null 2>&1 || die "没有 docker compose v2"

dc() { docker compose -f "$COMPOSE" --env-file "$ENVFILE" "$@"; }
api_ok() { curl -fsS --max-time 5 "$API_HEALTH" >/dev/null 2>&1; }
web_ok() { curl -fsS --max-time 5 "$PUBLIC_URL/login" >/dev/null 2>&1; }

current_tag() {
  docker inspect "$1" -f '{{.Config.Image}}' 2>/dev/null \
    | awk -F: 'NF>1{print $NF}' || true
}

registry_path() { # $1=服务名 → 镜像全名
  local prefix
  prefix="$(sed -n 's/^FLUX_IMAGE_PREFIX=//p' "$ENVFILE" | head -1)"
  prefix="${prefix:-ghcr.io/gntv456/fluxtorrent}"
  printf '%s/%s:%s' "$prefix" "$1" "$TAG"
}

img_max_mig() {
  docker run --rm --entrypoint sh "$1" -c \
    'ls /app/migrations/ 2>/dev/null \
     | grep -oE "^[0-9]+" | sort -n | tail -1'
}

db_max_mig() {
  dc exec -T postgres psql -U flux -d fluxtorrent -tAc \
    'SELECT COALESCE(max(version),0) FROM _sqlx_migrations' \
    2>/dev/null | tr -d ' \r'
}

say "1/6 预检"
PREV_API="$(current_tag flux-api || true)"
[[ -n "$PREV_API" ]] \
  || die "flux-api 没在跑 —— 首次安装请用 scripts/quick-deploy.sh"
ok "当前线上 api 镜像 tag：$PREV_API"
if [[ "$PREV_API" == "latest" ]]; then
  warn "线上跑的是移动标签 latest —— 回滚目标不可靠，也无法复现。
     本次升级会把它固定成具体版本，之后回滚才有意义。"
fi
[[ -f "$STATE" ]] && ok "上次记录的目标版本：$(cat "$STATE")"

if [[ -z "$TAG" ]]; then
  say "2/6 查询最新 Release"
  JSON="$(curl -fsSL --max-time 20 \
    "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null || true)"
  if [[ -n "$JSON" ]]; then
    TAG="$(printf '%s' "$JSON" \
      | grep -o '"tag_name"[^,]*' | grep -o '"[^"]*"$' | tr -d '"')"
  fi
  if [[ -z "$TAG" ]]; then
    warn "查不到 Release（离线/私有库/GitHub 抖动），退到 :latest"
    TAG="latest"
  fi
else
  say "2/6 使用指定版本"
fi
# Release tag 带 v（v0.5.3），CI 发的镜像 tag 不带（publish-images 用
# ${TAG#v}，GHCR 上只有 0.5.3 / 0.5.3-amd64 / 0.5.3-arm64）。不剥这层 v，
# 拉镜像必然 404，而报错文案指向「私有包/版本不存在」——两头都指错。
TAG="${TAG#v}"
ok "目标版本：$TAG"
if [[ "$TAG" == "$PREV_API" ]]; then
  ok "线上已经是这个版本"
  [[ $CHECK -eq 1 ]] && exit 0
  if [[ $YES -eq 0 ]]; then
    read -r -p "仍要重跑一次（会重启四个服务）？[y/N] " a
    [[ "$a" == [yY] ]] || exit 0
  fi
fi

say "3/6 迁移守卫"
if [[ $CHECK -eq 0 ]]; then
  ok "拉取镜像（不重建，通常 10~60 秒）"
  for s in $SERVICES; do
    img="$(registry_path "$s")"
    if ! docker pull "$img" >/dev/null 2>&1; then
      die "拉取 $img 失败。常见原因只有两种：
       ① 包是私有的 —— 在 GitHub → Packages → 该包 → Package settings
          把可见性改成 Public；或在服务器上 docker login ghcr.io -u 用户名
          -p <带 read:packages 的 PAT>。未登录读私有包会表现成 not found。
       ② 版本不存在 —— CI 只在推 v* tag 时发镜像，核对 Releases 有没有该 tag。"
    fi
    ok "$s"
  done
  IMG_API="$(registry_path api)"
else
  # --check 不拉镜像；目标 tag 若本地已有，就按**目标镜像**判方向 ——
  # 这才是真正要问的问题：我要切过去的那个包，会不会把 api 弄崩。
  IMG_API="$(registry_path api)"
  if ! docker image inspect "$IMG_API" >/dev/null 2>&1; then
    warn "--check：目标镜像本地没有，只能按当前线上镜像判断"
    IMG_API="$(docker inspect flux-api -f '{{.Config.Image}}')"
  fi
fi

IMG_MIG="$(img_max_mig "$IMG_API" || true)"
DB_MIG="$(db_max_mig || true)"
[[ -n "$DB_MIG" ]] || die "读不到库里的迁移版本（postgres 容器没起？）"
# 迁移文件名是 0001 这类零填充串，直接进 [[ -gt ]] 会被当八进制（0448 的 8
# 非法 ⇒ "value too great for base"）。统一剥成十进制再比。
[[ -n "$IMG_MIG" ]] && IMG_MIG=$((10#$IMG_MIG))
DB_MIG=$((10#$DB_MIG))
ok "库里已应用到：$DB_MIG"
ok "目标镜像含到：${IMG_MIG:-?}（$IMG_API）"
if [[ -n "$IMG_MIG" && "$DB_MIG" -gt "$IMG_MIG" ]]; then
  die "目标镜像比库还旧（库 $DB_MIG > 镜像 $IMG_MIG）。
     启动后 sqlx 会拒绝跑 → api 崩溃循环、全站 500。
     正解：换一个 ≥$DB_MIG 的版本 tag；若已误降级，先把库恢复到对应版本。"
elif [[ -n "$IMG_MIG" && "$IMG_MIG" -gt "$DB_MIG" ]]; then
  warn "将应用 $((IMG_MIG - DB_MIG)) 号新迁移（服务启动时自动执行）"
elif [[ -z "$IMG_MIG" ]]; then
  # 「读不到上界」不是「方向安全」：守卫判不出就必须停手，否则这条
  # 门禁在它最该起作用的时候（镜像残缺/没有 sh/没拉下来）正好沉默。
  if [[ $CHECK -eq 1 ]]; then
    warn "读不到目标镜像 $IMG_API 的迁移上界——方向无法判定，别照这句话的绿去升级"
  else
    die "读不到目标镜像 $IMG_API 里 /app/migrations 的上界。
     判不出迁移方向就不切换：库里已应用到 $DB_MIG，而 sqlx 遇到
     「库比镜像新」会直接崩溃循环、全站 500。
     先确认镜像是完整的（docker pull $IMG_API，再看
     docker run --rm --entrypoint sh $IMG_API -c 'ls /app/migrations | tail -1'）。"
  fi
else
  ok "迁移方向安全（无新增、无回退）"
fi

if [[ $CHECK -eq 1 ]]; then
  say "--check 结束"
  printf '  当前 %s → 目标 %s\n' "$PREV_API" "$TAG"
  exit 0
fi

say "4/6 确认"
printf '  服务：%s\n  当前：%s\n  目标：%s\n' \
  "$SERVICES" "$PREV_API" "$TAG"
if [[ $YES -eq 0 ]]; then
  read -r -p "  继续切换？[y/N] " a
  [[ "$a" == [yY] ]] || { echo "  已取消"; exit 0; }
fi

say "5/6 切换"
FLUX_VERSION="$TAG" dc up -d --no-build $SERVICES

say "6/6 健康检查"
healthy=0
for _ in $(seq 1 30); do
  if api_ok && web_ok; then healthy=1; break; fi
  sleep 5
done

if [[ $healthy -eq 1 ]]; then
  printf '%s\n' "$TAG" > "$STATE"
  ok "api 健康 + 站点 200"
  say "升级完成：$PREV_API → $TAG"
  exit 0
fi

warn "健康检查失败（150 秒内 api / 站点未就绪）"
docker logs --tail 40 flux-api 2>&1 | sed 's/^/    /' || true
if [[ $WITH_ROLLBACK -eq 1 && -n "$PREV_API" && "$PREV_API" != "$TAG" ]]; then
  say "回滚到 $PREV_API"
  FLUX_VERSION="$PREV_API" dc up -d --no-build $SERVICES || true
  sleep 20
  if api_ok; then
    ok "已回滚，api 恢复健康"
    die "本次升级失败，线上已回到 $PREV_API（日志见上）"
  fi
  die "回滚后仍不健康 —— 需人工介入（docker logs flux-api）"
fi
die "升级失败且未回滚（--no-rollback 或无上一版本记录）"
