# 快速开始（从零到开站）

> 适用：一台装了 Docker 与 git 的 Linux/Windows 服务器。全程约 10 分钟，其中大部分在等镜像拉取。

## 1. 起服务

```bash
git clone <本仓库> && cd FluxTorrent
cp docker/.env.example docker/.env
vim docker/.env    # 必填三项：DB_PASSWORD / REDIS_PASSWORD / JWT_SECRET
                    #（JWT_SECRET 须 ≥32 字节随机串；含 "change_me" 的值在生产模式会被启动检查拒绝）
docker compose -f docker/docker-compose.yml up -d
```

五个服务会依次就绪：postgres 16 → redis 7 → api（自动执行全部数据库迁移）→ worker（定时任务）→ tracker（HTTP 7070 + UDP 6969）。健康验证：`curl http://localhost:8080/api/v1/health`。

## 2. 完成安装向导

浏览器打开 `http://localhost:3000/setup`：

1. **选站型**：11 种预置（综合/教育/影视/音乐/动漫/电子书/体育/游戏/软件/纪录片/无损），决定分类、维度、模块缺省；拿不准就选「综合」，之后随时可换；
2. **站点名称 + 管理员**：系统已预置 `root`（初始密码 `password123`，首次登录强制修改——向导里用改密后的新密码登录）；
3. **合规勾选 → 完成安装**。向导幂等，可重复访问。

完成前所有业务 API 处于装机封锁状态（只放行登录/改密/健康检查），这是设计行为。

## 3. 开站前三项检查

不检查也能跑，但放用户进来之前建议先做（详见 [launch-checklist.md](launch-checklist.md)）：

1. **tracker announce 地址**：后台「站点设定」把 `announce_url` 从默认 `http://127.0.0.1:7070`（tracker 根地址，只对本机可用）改成你的公网域名——否则用户下载的 .torrent 里是内网地址，无法做种。注意填 tracker 根地址（不带 `/announce` 尾缀，系统会自动拼 `/announce/<passkey>`）；
2. **邮件 SMTP**：后台「站点设定 → 邮件」填 SMTP 服务器/端口/发件人即生效（找回密码/邀请信/群发都用它）；
3. **注册模式**：默认邀请制；要开放注册改 `registration_mode=open`。

## 4. 引导首个用户

后台「邀请管理」直发邀请码，或让 LV3+ 用户在 `/invites` 自助生成；也可切换开放注册。

## 常见问题

- **某个容器起不来**：`docker logs <容器名>`；api 反复重启多为 `.env` 三项必填缺失或 JWT_SECRET 不合规。
- **装完想重来**：`docker compose -f docker/docker-compose.yml down -v` 清卷后重来（会清掉全部数据，勿在生产执行）。
- **公网部署**：反代/TLS/边缘限流见 `_doc/生产部署指南.md`（对内文档），要点已收录进 [troubleshooting.md](troubleshooting.md)。
