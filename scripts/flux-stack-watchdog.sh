#!/usr/bin/env bash
# flux-stack-watchdog —— flux-api 崩溃循环哨兵（2026-11-20 百灵宕机复盘产物）
#
# 背景：dev.38 webhook 自动发版时百灵库 flux-api 迁移校验失败进入崩溃循环，
# tracker/worker 被 compose 依赖链卡死，全站 API 500 六小时——部署失败只写
# webhook 本地日志，无任何告警。本哨兵补上「失败要喊人」这一环。
#
# 检测（任一命中即告警）：
#   1) flux-api 容器 Restarting 状态 / restart 计数 5 分钟内增长 ≥2
#   2) 任一 flux 容器非 healthy（api/web/tracker）
#   3) 容器内 /api/v1/health 非 {"code":0,...,"status":"up"}
#
# 告警：SMTP（Brevo relay，凭据从本机 flux-postgres site_settings 读，
#       与业务发信同一凭据源）+ 本地日志 /var/log/flux-watchdog.log。
# 防抖：同类告警（同一 fingerprint）1 小时内不重发；恢复发 recovery 邮件。
#
# 安装（两台服务器各一份）：
#   install -m 755 flux-stack-watchdog.sh /usr/local/bin/flux-stack-watchdog
#   echo '*/5 * * * * root /usr/local/bin/flux-stack-watchdog' \
#     > /etc/cron.d/flux-watchdog
# 测试：/usr/local/bin/flux-stack-watchdog --test   （强制发一封告警邮件）

set -u
LOG=/var/log/flux-watchdog.log
STATEDIR=/var/lib/flux-watchdog
mkdir -p "$STATEDIR" 2>/dev/null || true

FORCE_TEST="${1:-}"
HOST_TAG=$(hostname -s)
DB="docker exec flux-postgres psql -U flux -d fluxtorrent -tAc"
TS() { date '+%Y-%m-%d %H:%M:%S%z'; }

# ---- 邮件发送（凭据取自 DB，与业务发信同源） ---------------------------
send_mail() {
  local subj="$1" body="$2" logs="$3" kind="$4"
  local smtp_host smtp_port smtp_user smtp_pass smtp_from enc
  smtp_host=$($DB "SELECT value FROM site_settings WHERE name='smtp_host'" 2>/dev/null)
  smtp_port=$($DB "SELECT value FROM site_settings WHERE name='smtp_port'" 2>/dev/null)
  smtp_user=$($DB "SELECT value FROM site_settings WHERE name='accountname'" 2>/dev/null)
  smtp_pass=$($DB "SELECT value FROM site_settings WHERE name='accountpassword'" 2>/dev/null)
  smtp_from=$($DB "SELECT value FROM site_settings WHERE name='smtp_from'" 2>/dev/null)
  enc=$($DB "SELECT value FROM site_settings WHERE name='encryption'" 2>/dev/null)
  local to; to=$($DB "SELECT value FROM site_settings WHERE name='contact_email'" 2>/dev/null)
  if [ -z "$smtp_host" ] || [ -z "$smtp_user" ]; then
    echo "$(TS) MAIL_SKIP: 无SMTP凭据" >> "$LOG"; return 1
  fi

  local url="smtp://$smtp_host:$smtp_port"
  [ "$enc" = "ssl" ] && url="smtps://$smtp_host:$smtp_port"
  # smtps(465) 用隐式 TLS 无需参数；STARTTLS(如 Brevo 2525) 靠 --ssl-reqd
  local tlsargs=""
  [ "$enc" != "ssl" ] && tlsargs="--ssl-reqd"
  local full="Subject: $subj
From: flux-watchdog <$smtp_from>
To: <$to>
Content-Type: text/plain; charset=UTF-8
Date: $(date -R)

$body
${logs:+

--- docker logs flux-api (tail 15) ---
$logs}
"
  printf '%s' "$full" | curl -s --max-time 30 \
    --url "$url" $tlsargs --mail-from "$smtp_from" --mail-rcpt "$to" \
    --user "$smtp_user:$smtp_pass" --login-options 'AUTH=LOGIN' -T - 2>>"$LOG"
  local rc=$?
  echo "$(TS) MAIL($kind) rc=$rc → $to" >> "$LOG"
  return $rc
}

# ---- 检测 ------------------------------------------------------------
declare -a issues=()

# 1) api 容器状态
api_state=$(docker inspect flux-api --format '{{.State.Status}}' 2>/dev/null || echo missing)
if [ "$api_state" != "running" ]; then
  issues+=("flux-api 容器状态=${api_state}（应为 running）")
fi

# restart 计数窗口增长（存基线，5 分钟一跑，涨 ≥2 = 崩溃循环）
restarts=$(docker inspect flux-api --format '{{.RestartCount}}' 2>/dev/null || echo 0)
statefile="$STATEDIR/restarts"
prev_restarts=$(cat "$statefile" 2>/dev/null || echo "$restarts")
echo "$restarts" > "$statefile"
delta=$((restarts - prev_restarts))
if [ "$delta" -ge 2 ]; then
  issues+=("flux-api 5 分钟内重启 ${delta} 次（RestartCount ${prev_restarts}→${restarts}）= 崩溃循环")
fi

# 2) healthy 探针（api/web/tracker）
for c in flux-api flux-web flux-tracker; do
  h=$(docker inspect "$c" --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}no-healthcheck{{end}}' 2>/dev/null || echo missing)
  if [ "$h" != "healthy" ] && [ "$h" != "no-healthcheck" ]; then
    issues+=("$c 健康=${h}")
  fi
done

# 3) 业务健康口
health=$(docker exec flux-web node -e \
  'fetch("http://api:8080/api/v1/health").then(r=>r.text()).then(t=>console.log(t.slice(0,200))).catch(e=>console.log("FETCH_ERR:"+e.message))' \
  2>/dev/null || echo "EXEC_ERR")
case "$health" in
  *'"status":"up"'*) : ;;
  *) issues+=("/api/v1/health 异常：${health:0:120}") ;;
esac

# ---- 防抖与发送 ------------------------------------------------------
fp_file="$STATEDIR/last-alert-fp"
rec_file="$STATEDIR/last-recovery-ts"
if [ "${#issues[@]}" -gt 0 ] || [ "$FORCE_TEST" = "--test" ]; then
  if [ "$FORCE_TEST" = "--test" ] && [ "${#issues[@]}" -eq 0 ]; then
    issues=("--test 模式构造的测试告警（系统实际全部正常）")
  fi
  fp=$(printf '%s\n' "${issues[@]}" | sort | sha256sum | cut -c1-16)
  now=$(date +%s)
  last_fp=$(cat "$fp_file" 2>/dev/null || echo "")
  last_rec=$(cat "$rec_file" 2>/dev/null || echo 0)

  # 新 fingerprint 或 距上次恢复 >10 分钟 才发（同因不刷屏；复发要报）
  if [ "$FORCE_TEST" = "--test" ] || [ "$fp" != "$last_fp" ] || [ $((now - last_rec)) -gt 600 ]; then
    body=$(printf '%s\n' "${issues[@]}")
    logs=$(docker logs flux-api --tail 15 2>&1 | head -40 || true)
    echo "$(TS) ALERT fp=$fp: ${body//$'\n'/; }" >> "$LOG"
    send_mail "[$HOST_TAG] flux-api 异常" "$body" "$logs" alert
    echo "$fp" > "$fp_file"
  else
    echo "$(TS) ALERT(去重) fp=$fp" >> "$LOG"
  fi
else
  # 恢复：上次有告警且现在全绿 → recovery 邮件
  if [ -s "$fp_file" ]; then
    echo "$(TS) RECOVERY" >> "$LOG"
    send_mail "[$HOST_TAG] flux-api 已恢复" "所有检测项通过：api running、容器 healthy、/health up。" "" recovery
    rm -f "$fp_file"; date +%s > "$rec_file"
  fi
fi

exit 0
