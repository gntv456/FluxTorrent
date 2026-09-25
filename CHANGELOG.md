# 更新日志（Changelog）

本文件记录面向部署者的显著变更。格式参照 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)；
版本号在首个语义化 tag（v0.x）发布后启用。

## [Unreleased]

### 通用建站定位四审收口 A 批（2026-09-25，详见 _doc/通用建站定位四审报告-2026-09-25.md）

- **生产空库首启不再自锁**：演示账号中性化防线改为「公开哈希 + 演示签名」双条件
  （`passkey LIKE 'demo%'` 或 `@demo.local`）。0017 引导 root 用的就是同一个
  password123 哈希，旧条件会把 root 一并随机化且不落口令日志，导致生产首启谁也
  登不进、向导也就进不去；root 的公开口令由已有的 must_reset_password 服务端闸门兜住。
- **站型包「另存快照」不再丢自定义**：custom 包快照补齐 `sections`/`tags`/`classes`/
  `economy`/`metadata` 五段（新增 `staff_http/pack_snapshot.rs`，与预置包同形；
  未采到的段落写 SQL NULL 而非 JSON null，避免 `apply_pack_extras` 误判为已声明）。
- **批量写口单源化**：`POST /admin/torrents/batch` 的 `change_category` 不再接受
  `medium_id/grade_id/edition_id`（改这三列不会反写 `torrent_sections`，会造成详情页
  与筛选显旧值），返回 400 并引导用 `change_sections`；前端本就只发 `category_id`。
- **假开关收口**：签到三键（`attendance_first`/`attendance_streak`/
  `attendance_daily_cap`）与 `farm_market_window_hours` 接上真实读取；迁移 0193 摘除
  零消费的 `carousel_images`/`stylesheet_default`/`nfo_view_style_default`/
  `bank_max_rate_pct`/`magic_pool_target_default`，主题令牌控件改取色器，`site_type`
  标为只读（此前渲染成可改下拉但必被拒）；`theme_token_glow` 前端注入名对齐
  `--brand-glow`（原来注入 `--glow`，无人读，改色不生效）；删除死配置字段 `SEED_DEMO_DATA`。
- 升级注意：0193 会删除上述五个设置键的值与元数据行，并从站型包 economy 预设里剔掉
  两个同名键；这些键此前无任何代码读取，删除不影响运行行为。
- **CI 与本地闸门**：`scripts/install_e2e.py` 把「空库首启→强制改密→完成向导→演示数据
  清 0」变成 11 条可复跑断言，并挂进 `e2e-smoke`（这是唯一能抓装机链断链的门）；
  三条 workflow 加 `concurrency` 去重、dependabot PR 不再跑重编译与 e2e（只留
  `security-audit`）、`e2e-smoke` 加 `paths` 过滤——private 仓库的 Actions 分钟数是有限
  资源（实测最后成功是 2026-09-18，之后 4~5 秒判死且无日志）。新增 `scripts/ci_local.sh`
  作 CI 不可用时的等价本地闸门；修 `audit_migration_checksums.py` 的 SQL 拼接缺空格
  （psql 报错被静默吞掉，导致所有迁移被误判「文件缺失」）与两个反了的分支标签。

### 通用建站系统收口（2026-09-25 六迭代，详见 _doc/通用建站定位符合度三审报告）

- **模块缺省翻转**：可选模块缺省从教育站全开改为 general 中立矩阵；
  11 预置站型包补全 29 键完整 modules 快照（0178/0179）
- **站型切换链路**：安装向导完整应用站型包（此前只落 extras 一段）；
  apply 不再清空自定义标语；词表重建带引用守卫；自定义站型快照带字幕口径
- **门禁收口**：网关补登 13 端点、worker 补挂 7 任务（含论坛抽奖锁）、
  首页/详情/弹窗/页脚/移动 TabBar 按模块过滤、forums 三子页与
  contactstaff/staffbox 补守卫
- **内容中性化**：初装论坛版块/勋章/任务/成就/投票去教育口吻；三语词典
  删除硬编码教育词表；登录页兜底标语中性化（0178/0180）
- **站型包拉平**：economy 预设 11/11、metadata 预设修正（去 musicbrainz
  死字、anime 补 bangumi）、分类图标模式函数化（apply 自动铺装）
- **等级体系后台化**：GET/PUT /admin/classes（1-12 档阈值/降级/晋升奖励，
  staff 档锁定）
- **适配器破空货架**：douban 元数据适配器 wasm 随核心上架（编译预检/
  checksum 幂等/默认停用）
- **用户自定义字段**：六类型字段定义/公开与私密可见性/注册页展示位，
  后台管理面板 + usercp 填写 + 公开档案下发 + 注册页动态渲染（0186）
- **自定义页面**：任意内容页（/p/slug，ammonia 消毒）+ 后台管理面板 +
  菜单挂接（0187/0192）
- **分类层级**：parent_id 树形（防环触发器）+ 列表筛选级联展开子孙 +
  后台父分类下拉（0188）
- **主题令牌**：品牌八色后台可配并随 theme 包分发，layout 注入 :root
  （0191）
- **旧三列退役**：教育词表按站型清理、存量数据迁 sections、前端展示
  全部单源化（0180/0185）
- **生态加固**：marketplace 大小上限/https 强制、rules 回滚只删包内键、
  core_compat 实际校验、site_type 设置页直改拒绝（引导走站型切换）
- **三审复核**：门禁终检漏网补齐（me/staffmessages、medal-rarities）、
  好学残留清理、en 词典笔误、appearance 分组命名

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

0. **升级到含「0134 函数注释搬移」修复的镜像之前，存量库必须先跑一次**
   `docker exec -i flux-postgres psql -U flux -d fluxtorrent < scripts/align_migration_checksums_0134.sql`，
   否则 sqlx 会因 134/136 校验和变化报 "migration was previously applied but has been
   modified" 拒绝启动。背景：`0134` 对三个只在 `0136` 创建的函数下 COMMENT，**空库按序
   执行到 134 必失败**（flux-api crash-loop），存量库因函数已存在而一直没暴露。
   同期修复：装机白名单补 `/api/v1/me/password`（否则 root 的强制改密被装机门拦住，
   向导永远完不成）；自助改密后失效 5s 用户状态缓存（否则改完密立刻完成向导会被
   「临时密码」旧值挡下）。另修 `purge_demo_data()` 的删除顺序（新迁移 **0194，已入库**：
   原实现先删 users 撞 `torrents_owner_id_fkey`、引用不存在的 `torrents.title`、演示种子
   口径不匹配，导致向导第一步在真空库上必 500；改后按 topics → torrents → 按
   `pg_constraint` 动态清引用表 → users 的安全顺序，空库首启端到端 10/10 通过）。

1. 迁移 0142 对大表建索引：存量站点请在低峰窗口升级，或带外 `CREATE INDEX
   CONCURRENTLY` 预建同名索引后再启动（迁移内 IF NOT EXISTS 会跳过）
2. 存量库如应用过「0129/0130 换号版」迁移，先执行
   `scripts/align_migration_renumber_0127_0128.sql` 再拉新代码
3. 反代部署在 .env 加 `TRUST_PROXY=1` 后 `docker compose up -d` 生效
4. 附件 named volume 首次创建后如属主不对（旧部署绑定目录迁移场景），宿主侧
   `chown -R 1000:1000 <目录>`
