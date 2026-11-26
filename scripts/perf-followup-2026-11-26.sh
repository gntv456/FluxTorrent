#!/usr/bin/env bash
# 2026-11-26 加速批·上机收尾清单（主站 hxpt-prod 与镜像站两套都要做）
# 仓库侧代码已全部落地；本脚本只列需要在服务器上执行的动作，逐条手工跑。
# 用法：逐段复制执行，不要整文件 bash 跑（含只读诊断段）。

set -euo pipefail

echo "== ① nginx /api 直连改道（收益最大的一跳） =="
# 现状：浏览器 API 请求走 browser → Next route.ts 代理 → api 容器（双跳）。
# 目标：宿主 nginx 把 /api/v1/ 直接 proxy_pass 到 compose 网的 api:8080。
# 主站（hxpt-prod）：编辑 /etc/nginx/conf.d/flux.conf（或对应站点 conf），
# 把 /api/v1/ location 的 proxy_pass 指向 http://127.0.0.1:8080（若 nginx
# 与 compose 同机且 8080 已发布）或加 server 块进 compose 网络。
# 参考 docker/nginx/lb.conf.sample 的写法（含 gzip 兜底段）。
# 验证（改完 reload 后）：
#   curl -s -o /dev/null -w '%{http_code} %{time_total}s\n' \
#     -H 'Accept-Encoding: gzip' https://<域名>/api/v1/site-profile
#   curl -sI -H 'Accept-Encoding: gzip' https://<域名>/api/v1/site-profile | grep -i content-encoding
# 期望：200 且 content-encoding: gzip（双保险中至少一层生效）。

echo "== ② DB 慢查询实测（pg_stat_statements，只读诊断） =="
# docker exec flux-postgres psql -U fluxtorrent -d fluxtorrent -c "
#   SELECT calls, round(mean_exec_time::numeric,1) AS ms,
#          round(total_exec_time::numeric) AS total_ms, left(query,90) AS q
#   FROM pg_stat_statements
#   WHERE query NOT LIKE '%pg_stat%'
#   ORDER BY total_exec_time DESC LIMIT 20;"
# 若扩展未启用：
#   docker exec flux-postgres psql -U fluxtorrent -d fluxtorrent -c \
#     "CREATE EXTENSION IF NOT EXISTS pg_stat_statements;"
# （还要 postgresql.conf shared_preload_libraries 加 pg_stat_statements 并重启，影响连接，择机做）
# 拿到 Top20 后再议索引——不盲建。

echo "== ③ 发版后验证 gzip 生效（api 容器直测，不经 nginx） =="
# docker exec <api容器> curl -s -o /dev/null -w '%{size_download}\n' \
#   -H 'Accept-Encoding: gzip' http://127.0.0.1:8080/api/v1/site-profile
# docker exec <api容器> curl -s -o /dev/null -w '%{size_download}\n' \
#   http://127.0.0.1:8080/api/v1/site-profile
# 期望：带 gzip 头的字节数明显小于不带。

echo "== ④ .torrent 口回归（确认豁免闸没误伤压缩） =="
# 任选一颗种：
#   curl -sI -H 'Accept-Encoding: gzip' \
#     https://<域名>/api/v1/torrents/<id>/download-link | grep -i content-encoding
# 期望：无 content-encoding（identity 透传），Content-Type: application/x-bittorrent。
# SSE 同理：/api/v1/me/messages/stream 须无 content-encoding 且首帧立即到达。

echo "== ⑤ 回访缓存生效验证（web 层） =="
# curl -sI https://<域名>/fonts/$(ls 随便一个woff2名) | grep -i cache-control
# 期望：public, max-age=2592000；theme-css 为 max-age=86400。
