# 匿名使用统计收集端部署（0276）

「有多少站在用 FluxTorrent」的收数端。站点侧代码已内置（worker 任务
`usage_stats`，默认关），这里只是把接收端跑起来——一个 Cloudflare Worker
+ KV，免费额度（10 万请求/天）对本用途是天文数字。

## 站点侧怎么开（站长视角）

后台「站点设定 → 运维」：

- **匿名使用统计回报**：开启（默认关闭）
- **统计收集端地址**：默认官方收集端；自建就改成你自己的 `/ping` 地址；
  清空 = 即使开关开着也不发

上报内容只有五项：16 位匿名站点哈希（`sha256(JWT_SECRET+站名)` 截断，
不可逆）、版本号、用户数、种子数、活跃 peer 数。不含域名、IP、站名。

## 自建收集端（项目维护者）

1. 注册 Cloudflare 账号（免费），安装 wrangler CLI：
   `npm install -g wrangler && wrangler login`
2. 建 KV 命名空间：
   `wrangler kv namespace create STATS`（记下输出的 id）
3. 部署本目录的 worker.js：
   ```bash
   wrangler deploy scripts/stats-collector/worker.js --name flux-stats \
     --kv STATS=<上面拿到的 id> --var ADMIN_TOKEN:<自选一个查看口令>
   ```
4. 部署后拿到 `https://flux-stats.<你的子域>.workers.dev`，
   上报地址就是它加 `/ping`。
5. 看板：浏览器或 curl 访问 `/admin?token=<ADMIN_TOKEN>`，返回
   `{"sites": N, "active_30d": ..., "by_version": {...}, "sites": [...]}`。

## 口径说明

- **sites** = 收到过心跳的不同 site_id 数；站长换 JWT_SECRET 会被算成
  新站（略偏高，可接受）。
- **active_30d** = 最近 30 天内有心跳的站数——比总量更接近「在跑的站」。
- **by_version** = 各版本站点数，用来判断旧版本还剩多少（安全更新
  推送优先级的依据）。
- 收集端无法反查任何站点的身份；这是设计出来的性质，不是巧合。
