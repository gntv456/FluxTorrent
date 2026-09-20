# 更新日志（Changelog）

本文件记录面向部署者的显著变更。格式参照 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)；
版本号在首个语义化 tag（v0.x）发布后启用。

## [Unreleased]

### 安全（Security）

- **JWT 收敛 HttpOnly cookie**：登录经 Set-Cookie 下发 HttpOnly+SameSite=Lax 的
  `flux_token`（路径 /api/v1，24h；登出清除）；前端全面移除 localStorage 存储，
  XSS 不再可窃取会话令牌（Bearer 兼容保留，API Token 工具流不受影响）
- **发种上传大小上限**：`.torrent` 4MiB / NFO 1MiB 流式拦截（此前无上限，可 OOM 单体 api）
- **用户抓取明细隐私**：`/users/{id}/torrentlist` 的做种/下载/完成明细仅本人
  与 staff 可见（uploads 与保种认领保持公开）
- **演示账号防线**：生产态启动自动随机化 0018 演示数据中仍持有公开口令
  （password123）的账号
- **access log 剥离 query**：compat 下载 `?passkey=`、凭证 `?token=`、开放 API
  `?apikey=` 不再写入访问日志
- **反代真实 IP**：新增 `TRUST_PROXY=1` 配置——限流/IP 封禁/登录风控改信
  X-Forwarded-For 首值（反代部署必开）

### 修复（Fixed）

- **资金正确性（P0 印钞口族）**：银行定存/活期/站免池捐赠/众筹/置顶购买/勋章
  购买与赠送/贷款还款/悬赏冻结/论坛打赏的幂等重放闸门全覆盖——重放请求不再
  重复发放存单/池账/众筹进度/授予；幂等键统一加用户前缀防跨用户碰撞
- **单事务化**：银行存取/活期/签到/放款/还款/农场收获/论坛打赏的扣款、流水、
  快照、业务行同生共死，删除全部 spawn 退款/状态回滚补偿路径
- **付费下载余额快照**同事务更新（超花窗口消除）；**捐赠上传量套餐**补
  traffic_ledger 流水（不再被对账清掉）
- **迁移换号回退**：0127/0128 恢复原号（换号会让存量环境启动失败）；存量库
  对齐脚本 `scripts/align_migration_renumber_0127_0128.sql`
- **redis 健康检查**带密码（修 NOAUTH 假阳性）；**metrics token 变量名统一**
  ANN_METRICS_TOKEN（api 仪表盘不再恒空）
- **worker 自动扣款**锁内重读贷款状态（与手动还款并发不再双扣）

### 新增（Added）

- **request_id 贯穿**：信封/响应头/日志同源（沿用合法入站 X-Request-Id，
  支持跨系统串联排障）
- **对账告警 job**：流水 vs 快照三组差异检查（先于 reconcile 执行保留证据），
  负余额检测
- **监控栈**：`docker compose --profile monitoring up -d` 一键启用
  prometheus + grafana（预置仪表盘 + 五条告警：DLQ 积压/5xx 率/tracker Redis
  降级/Stream 积压/连接池打满）
- **热点索引迁移（0142）**：snatches.torrent_id、comments、messages、topics、
  torrents.owner_id 六个缺失索引
- **CI**：fmt/next build 门禁、announce→计费链路冒烟（tracker+worker 进 CI）、
  匿名鉴权矩阵遍历
- 治理文件：CONTRIBUTING / SECURITY / CODE_OF_CONDUCT

### 变更（Changed）

- **容器非 root**：四镜像 uid 1000 专用用户；六服务内存限制；日志轮转 10m×3
- **附件持久化**：api 挂附件 named volume（升级不再丢用户附件）；backup.sh
  覆盖附件
- **游戏运行时 EV 防线**：猜大小赔率钳 1999‰；刮刮乐档位 EV 复算 ≥1 回落缺省
- **仓库清理**：约 270 个非代码文件移出 git 跟踪（调试产物/竞品素材/个人工作区），
  .gitignore 补齐

### 升级注意事项（Upgrade Notes）

1. 迁移 0142 对大表建索引：存量站点请在低峰窗口升级，或带外 `CREATE INDEX
   CONCURRENTLY` 预建同名索引后再启动（迁移内 IF NOT EXISTS 会跳过）
2. 存量库如应用过「0129/0130 换号版」迁移，先执行
   `scripts/align_migration_renumber_0127_0128.sql` 再拉新代码
3. 反代部署在 .env 加 `TRUST_PROXY=1` 后 `docker compose up -d` 生效
4. 附件 named volume 首次创建后如属主不对（旧部署绑定目录迁移场景），宿主侧
   `chown -R 1000:1000 <目录>`
