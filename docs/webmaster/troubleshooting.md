# 故障排查（装机坑典）

按「症状 → 定位 → 处置」组织。容器名前缀 `flux-`（可用 `docker ps` 确认）。

## 装机段

| 症状 | 定位 | 处置 |
| :--- | :--- | :--- |
| api 容器反复重启 | `docker logs flux-api` | `.env` 三项必填缺失（DB/REDIS_PASSWORD、JWT_SECRET），或 JWT_SECRET 含 `change_me` 且非开发态——补齐后重启 |
| 向导打不开/一直 404 | 浏览器访问的是 `:8080` | 向导在 web 侧：`http://localhost:3000/setup`；`/setup/status` 可看装机状态 JSON |
| 登不进 root | 初始密码 `password123` 是否已改 | 首次登录强制改密；改过忘了走数据库重置（已实测可行）：① 生成 argon2id 哈希 `python -c "from argon2 import PasswordHasher; print(PasswordHasher(time_cost=2,memory_cost=19456,parallelism=1).hash('新口令'))"`（需 `pip install argon2-cffi`）；② `docker exec flux-postgres psql -U flux -d fluxtorrent -c "UPDATE users SET pass_hash='<上述哈希>', must_reset_password=false WHERE username='root'"`；③ 用新口令登录。仍登不进且邮箱可用 → 「忘记密码」走邮件找回 |
| 装完页面全空 | 向导是否完成 | 完成前业务 API 装机封锁是设计行为；完成向导后即放开 |

## 运行段

| 症状 | 定位 | 处置 |
| :--- | :--- | :--- |
| 用户反映「下载了没法做种」 | 种子里的 announce 地址 | 后台改 `announce_url` 为公网域名（最常见的新站事故，见 checklist） |
| 找回密码邮件收不到 | SMTP | 后台「站点设定 → 邮件」+ 测试发信；未配置时 token 只进 api 日志（可临时从日志取） |
| 数据「不更新」（做种数/上传量不动） | worker | `docker logs flux-worker`；全部定时任务由 worker 执行（当前 45 个，以任务面板为准），worker 挂了数据就停更——这是与某些 PHP 引擎需要外部 cron 不同的地方：**本系统无需配 crontab**，worker 容器本身就是调度器 |
| 登录第 6 次被拒 | 限流（设计行为） | 登录 5 次/分/用户名；等一分钟或换正确密码 |
| tracker 429/announce 被拒 | 限流阈值 | 每用户 1800 次/分、每 IP 3600 次/分（`ANN_RATE_*` 可调）；被限只延迟计费不丢失 |
| 5xx 且 Grafana 告警 Stream 积压/DLQ | 消费链 | 后台「运行日志」看 worker 死信；DLQ 有看门狗 job 自动重试 |
| 慢查询 | pgstats | 后台 `?tool=dbstats`（pg_stat_statements 已由 compose 预装启用，≥300ms 自动落日志） |

## 数据库直连（排查用）

```bash
docker exec -it flux-postgres psql -U flux fluxtorrent   # 密码在 docker/.env
```

## 还有问题

- 健康检查：`curl localhost:8080/api/v1/health` 应返回 `{"status":"up"}`；
- 全链路回归：`python scripts/regression.py`（对开发环境跑端到端断言）；
- 提问时附上：版本 tag、`docker ps` 输出、相关容器最近 50 行日志。
