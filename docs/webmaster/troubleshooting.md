# 故障排查：装机坑典与常见故障

> 站起不来、用户做不了种、邮件收不到……遇到怪事先来这翻。按「现象 → 怎么定位 → 怎么处理」组织。不懂的词查 [术语小词典](glossary.md)。
> 容器名都带 `flux-` 前缀（用 `docker ps` 能确认）。

## TL;DR

- 大部分「起不来」都是 `.env` 三项没填或 `JWT_SECRET` 不合规——先看日志 `docker logs <容器名>`。
- 用户「下载了做不了种」十有八九是 announce 地址还是内网 `127.0.0.1`——去后台改公网域名。
- 数据「不更新」（做种数/上传量不动）是 worker 挂了——它才是定时任务的调度器，不用你配 crontab。
- 打开是「Welcome to nginx」= 流量落进了**宿主机 Nginx 的默认页**（本栈六个容器里没有 Nginx）：反代没配好，或没用域名访问——见装机阶段表。

---

## 装机阶段（刚起服务时）

| 现象 | 怎么定位 | 怎么处理 |
| :--- | :--- | :--- |
| api 容器反复重启 | `docker logs flux-api` | `.env` 三项必填缺失（DB/REDIS_PASSWORD、JWT_SECRET），或 JWT_SECRET 含 `change_me` 且非开发态——补齐后重启 |
| 向导打不开 / 一直 404 | 你访问的是哪个端口 | 向导在网站侧 `http://localhost:3000/setup`；`/setup/status` 能看装机状态（JSON） |
| 登不进 root | 初始密码 `password123` 是否已改 | 首次登录强制改密；改过忘了可走数据库重置（已实测可行）：① 生成 argon2id 哈希 `python -c "from argon2 import PasswordHasher; print(PasswordHasher(time_cost=2,memory_cost=19456,parallelism=1).hash('新口令'))"`（需 `pip install argon2-cffi`）；② `docker exec flux-postgres psql -U flux -d fluxtorrent -c "UPDATE users SET pass_hash='<上述哈希>', must_reset_password=false WHERE username='root'"`；③ 用新口令登录。仍登不进且邮箱可用 → 走「忘记密码」邮件找回 |
| 装完页面全空 | 向导是否完成 | 完成前业务接口被锁是**正常设计**；跑完向导即放开 |
| 打开是「Welcome to nginx!」欢迎页 | `curl -H "Host: 你的域名" http://127.0.0.1/` ① | 这是**宿主机 Nginx 的默认页**（本栈不含 Nginx，正常页面应由 web 容器出）：① 若 curl 也返回欢迎页 → 反代 server 块没生效：宝塔走 [宝塔部署](deploy-baota.md) G.3 建站+反向代理（目标 `http://127.0.0.1:3000`）；系统直装按 [域名部署](domain-deploy.md) §3.2 整段复制配置，确认软链进 `sites-enabled`、`nginx -t` 通过、已 reload；② 若 curl 返回的是你的站 → 配置没问题，是**用裸 IP 访问了**（server 只认域名）——改用域名访问，先确认 DNS 已解析到本机 |

---

## 运行阶段（已开站后）

| 现象 | 怎么定位 | 怎么处理 |
| :--- | :--- | :--- |
| 用户反映「下载了没法做种」 | 种子里的 announce 地址 | 后台改 `announce_url` 为公网域名（最常见的新站事故，见 [开站 checklist](launch-checklist.md)） |
| 找回密码邮件收不到 | SMTP 配置 | 后台「站点设定 → 邮件」+ 测试发信；没配时 token 只进 api 日志（可临时从日志取） |
| 数据「不更新」（做种数/上传量不动） | worker | `docker logs flux-worker`；全部定时任务都由 worker 跑（当前约 45 个，以任务面板为准），它挂了数据就停更。好消息：**本系统不用配 crontab**，worker 容器自己就是调度器 |
| 登录第 6 次被拒 | 限流（正常设计） | 登录限 5 次/分/用户名；等一分钟或换正确密码 |
| tracker 报 429 / announce 被拒 | 限流阈值 | 每用户 1800 次/分、每 IP 3600 次/分（参数可调）；被限只延迟计费不丢数据 |
| 5xx 且 Grafana 告警 Stream 积压 / DLQ | 消费链 | 后台「运行日志」看 worker 死信；DLQ 有看门狗任务自动重试 |
| 慢查询 | pg 统计 | 后台 `?tool=dbstats`（pg_stat_statements 已由 compose 预装启用，≥300ms 自动落日志） |

---

## 数据库直连（排查用）

```bash
docker exec -it flux-postgres psql -U flux fluxtorrent   # 密码在 docker/.env
```

---

## 还有问题

- 健康检查：`curl localhost:8080/api/v1/health` 应返回 `{"status":"up"}`；
- 全链路回归：`python scripts/regression.py`（对开发环境跑端到端断言）；
- 提问时附上：版本 tag、`docker ps` 输出、相关容器最近 50 行日志。

---

词看不懂？→ [术语小词典](glossary.md)
