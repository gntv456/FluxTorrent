# 监控与告警

## 起监控栈

```bash
docker compose -f docker/docker-compose.yml --profile monitoring up -d
# Grafana: http://localhost:3001（密码 GRAFANA_PASSWORD，默认 admin）
```

Prometheus 抓取 api 与 tracker 指标；Grafana 预置 FluxTorrent 看板（`docker/grafana-flux-dashboard.json`）。

## 指标端点

| 端点 | 门禁 |
| :--- | :--- |
| `api /metrics` | `ANN_METRICS_TOKEN`（未配置返回 404，防匿名抓取） |
| `tracker /metrics` | 同上 |

## 预置告警（`docker/monitoring/alerts.yml`）

| 告警 | 含义 | 第一反应 |
| :--- | :--- | :--- |
| DLQ 积压 | announce 消费死信堆积 | 看 worker 日志与「运行日志」页死信原因 |
| 5xx 比率 | api 错误率 | `docker logs flux-api` 最近错误栈 |
| tracker Redis 降级 | tracker 连不上 Redis | 检查 redis 容器/密码 |
| Stream 积压 | announce 流消费滞后 | worker 是否存活；游标卡住看 `flux:announce:cursor` |
| 连接池耗尽 | PG 连接不够 | 查慢查询（dbstats）；必要时调池大小 |

## 日常巡检

- `curl localhost:8080/api/v1/health`（建议 uptime 监控每分钟打点）；
- 后台「运行日志」页（runtime_logs 落库，自动清理）看 job 失败；
- 慢查询：compose 已开 `pg_stat_statements` + `log_min_duration_statement=300ms`，后台 `?tool=dbstats` 直接看热点。
