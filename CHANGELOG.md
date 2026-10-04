# 更新日志（Changelog）

本文件记录面向部署者的显著变更。格式参照 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)；
版本号在首个语义化 tag（v0.x）发布后启用。

## [未发布] - 2026-10-04

v0.3.0（2026-10-03）以来 6 个提交：部署体验 + 站长手册通俗化 + wiki。**无新迁移，升级零动作。**

### 部署体验

- **一键部署脚本增强**（quick-deploy）：起栈前自动检测 5 个默认端口（5432/6379/8080/3000/7070），被占自动换备选端口——旧服务器上跑着别的站不再起不来；向导未完成时自动临时开放 3000，装完重跑脚本自动收回。
- **反向代理一键补全**（新脚本 reverse-proxy.sh）：一条命令自动补 `/announce/` 与 `/api/` 反代（宝塔/系统 Nginx 均可），自动校验配置并体检，替代手动改配置文件。
- **安装向导更聪明**：用域名访问时一键采用建议的 Tracker 地址；IP 访问时明确提示「先留空」；完成页对 IP 直连部署给出收尾三步指引（界面三语）。

### 文档

- **宝塔部署手册重排**：全程地图 + 每阶段「做什么/怎么做/完成标志」+ 向导四屏对照表；SSH/1Panel/快速开始各篇同步补面板操作路径。
- 站内帮助中心（`/help`）上线；对外文档站落地 [wiki.ptang.top/ft](https://wiki.ptang.top/ft/)。

## [0.3.0] - 2026-10-03

v0.2.0（2026-09-27）以来 151 个提交的聚合：娱乐屋（游戏厅）全生态、抽卡收藏、
多机部署 G30、数据层规模 G31-D、UDP tracker 首次真可用、四语化、生态兼容复测、
UI 设计系统治理。**本版含新迁移 0227–0272（46 个），升级即自动执行。**

### 破坏性变更 / 升级注意

- **`JWT_ALG=rs256` 无密钥拒绝启动**（G30-A）：原先静默自动生成的密钥在容器重建/多副本
  下互不相认。rs256 部署须挂载密钥；hs256（默认）用户零感知。
- **对外列表口径变更**（0267）：兼容层/Torznab/RSS 默认**含零做种新种**（`alive=1`）——
  冷启动期第三方工具不再「零结果」；代价是持 Token 成员可批量导出目录（与站内浏览
  可见度一致，受 60 req/min 限流封顶）。
- **`docker/.env.example` 的 `CORS_ORIGINS` 缺省值**从空改为 `http://localhost:3000`
  （原留空非开发态必启动失败，新手第一次 `up -d` 就撞墙）；**生产必须改自己的域名**。
- 0230 起流水归档可配（`ledger_retain_months`，0=永久缺省）；论坛正文 posts 已移出
  归档清单（retention 永不删帖）。

### 娱乐屋（游戏厅）全生态（0228–0265，30+ 提交）

- **九款玩法**：钓鱼（星夜海舞台/鱼竿图鉴/渔汛）· 猜大小（三骰机制/按区奖池）·
  九宫格 · 刮刮乐 · 大转盘 · 扭蛋 · 农场（土地阶梯/施肥/等距苗床）· 牧场（加工坊）·
  宠物（进化/舞台化档案/日志）；「甜梦奇境·蓝调」整套换肤（纯 CSS 覆盖层）。
- **经济安全三闸**：玩法 EV 上限闸（票档就是 EV 闸，超限拒保存）· 奖池行表化
  （后台可配，删掉赔率静默回落）· 「扣款→结算」单事务（六条玩法路径）。
  全站游戏审计修复批另堵 3×P1 经济漏洞 + 造假三连 + 16×P2。
- **奖品有站内用途**：物品目录/背包/使用端点/装扮生效链/折算价；口粮券做种满
  6h 自动发（0239）。运营后台面板：三口径 EV 对照/参数现值/概率公示页/全服公示
  feed/周榜（收集度与局数，非经济）。

### 抽卡 + 收藏面（0228/0233–0235）

- 十连=真实单抽、数学双镜像（10 万抽 3.3560% 实测对表）、碎片分解/定向兑换/升级、
  staff 发放面板、个人收藏册 `/gacha`。

### 生态兼容复测批（2026-10-03，0267+0268）

- **修正此前「全绿」结论**：只验「端点可达」没验「工具消费路径」。按第三方工具
  真实用法逐项复测（48 断言）后修复：RSS `<enclosure>` 直链（cross-seed/qB/Flexget
  此前拿到 HTML）、Torznab caps 根节点（Prowlarr/Jackett 此前无法添加索引器）、
  tv/movie-search 参数、ptppUserInfo 补 passkey、老脚本别名薄壳。
- **开放 API 补深**：`/open/torrents/{id}` 深详情、`/open/me/{seeding,hr,messages,
history}`（移动壳不再爬 HTML）、`/openapi.json` 4→27 路径含 schema、高级筛选接
  站内同一套归一器。新增**工具维护者对接页** + `scripts/demo_seed.py`（三行命令
  起测实例）。矩阵见 `docs/ops/compat-matrix.md`。

### UI 设计系统治理（2026-10-03）

- **四 P0**：深色模式完全失效（`:root,` 前缀覆盖 109 个夜间令牌）· 移动端列表砖红
  色块透出 · `/login` 安全告知永久丢失（写死 max-height 无滚动条）· CSS 变量名带
  空格整条声明失效 + 9 个悬挂 `var()`。
- **新门禁** `scripts/ui_design_guard.mjs` 入 CI（六规则：变量须有定义/硬编码色只减
  不增/断点四档/z-index 语义档/reduced-motion/focus-visible）；品牌资产全套重制
  （logo/og/favicon/全档图标）。

### 多机部署 G30（迁移 0224/0225，三机配方已实证）

- 计费流消费者组（XREADGROUP + XAUTOCLAIM 回收）· 任务认领 `FOR UPDATE SKIP
LOCKED` + 角色分片 `FLUX_WORKER_JOBS` · tracker 挂 LB（XFF 双档）+ peer 外置
  Redis（多副本互见）· web 运行期可换上游 · `/health` 判活 · worker 优雅停机 ·
  跨进程配置 3s 跟随 · `docker/compose.multi-node.yml` + nginx LB 样例。
- 详见 `_doc/G30-多机部署三机配方.md`。

### 数据层规模 G31-D（迁移 0230/0237）

- **读写分离**：`DATABASE_REPLICA_URL` 只读副本，热点读分流（36 处），副本故障自动
  回落主库；**流水归档**：按月分区 DETACH→DROP 两步；**余额基线表**（0237）：归档
  前冻结期初，三处重算公式改 baseline+SUM（直接 SUM 会清零老用户余额的 P0 修正）。
- 部署规模矩阵 S/M/L/XL 四档（`docs/ops/performance.md`）。

### UDP tracker 首次真可用（G31）

- **⚠ BEP-15 魔数误写**（0x41727109807a→0x41727101980）：修复前 UDP tier 所有标准
  客户端被静默丢弃——此前「支持 UDP」实为从未可用。另修 interval 8 字节格式 bug；
  `TRACKER_UDP_WORKERS=N` 多核收包（SO_REUSEPORT）。

### PT 硬度深测修复批（2026-10-02/03，资深用户视角四轮）

- tracker 种子白名单（防幽灵 swarm 计费）· `sum()` numeric 静默归零 · H&R 三口径
  统一读 `hr_hours` · RSS 付费种默认过滤 · 注册初始等级断链（class0）·
  `orphan_offset` 余额污染 · 晋升奖励从不入账 · announce→计费全链路四发现。
- 另有 zt81 并行审计批：调度饿死根治/announce 参数加固/metrics 鉴权/peer GC 等。

### 开源对标 E 批（E1–E16 择要）

- **四语化**（zh-CN/en/ja 段级回落）；CSP 安全头最严形态；等级页晋级进度可视化；
  视图布局自配（列/段落显隐 9+12 键）；同义词检索；PWA 安装引导；捐赠自动回馈
  档位（四通道发放）；面板部署指南（1Panel/宝塔）；admin 数据大盘（趋势/健康度/
  周活）；升级演练 `scripts/upgrade_drill.py`；覆盖率基线 + Playwright E2E 骨架。
- **可插拔验证码**（none/turnstile/recaptcha/hcaptcha）· **passkey/WebAuthn 第二
  通道** · 申请制入站 · 不活跃策略三档 · 运维 webhook 双出口（Discord/TG）·
  邮件激活真实发信版。

### 仓库与 CI

- UI design guard / view_layout guard / terms guard / type drift guard 四门禁入 CI；
  `pnpm lint`（ESLint 9）+ Next build 门禁补齐；行数门禁 45 文件零新增。
- Rust 单测 144→202；模块 30→31 键（invites 入注册表）。

## [0.2.0] - 2026-09-27

竞品对比行动批（C 批，全案见 `_doc/开源生态全方位对比分析与完善策划案-2026-09-27.md`）。**本版含新迁移 0226**，升级即自动执行。

### C3 仓库卫生

- **根目录 520+ 个开发残留清出**：调试脚本/输出/截图（`_*` 前缀）移入本地 `_attic/`（已 gitignore，不入库不删除）；`.gitignore` 补 `/_attic/`、`/_*.cjs`、参考目录规则；解除误跟踪的 `_shot/` 截图。仓库根恢复「一个 clone 就能读懂」的形态。

### C1 文档中心 + README

- **新增 `docs/` 四册 22 篇**：webmaster（快速开始/开站 checklist/升级回滚/故障排查）、user（注册→顶级等级 7 篇用户指南）、customize（字段/维度/页面菜单/站型术语/内容包主题/模块/适配器 7 篇 How-to）、ops（监控/任务/性能/备份/工具生态收录）。内部策划案留在 `_doc/` 不对外。
- **README 全量重写**：修掉滞后两个数量级的旧数字（14 页/79 单测/migrations 0001+0002 → 75 路由文件/144 单测/225 迁移），补与 NexusPHP/UNIT3D 一页对比表与 docs 入口。

### C2 发布纪律

- 版本号全仓 0.1.0 → **0.2.0**（api/worker/tracker Cargo.toml + web package.json）；本文件首个正式版本段；发布 tag `v0.2.0`。
- **publish-images 增 arm64**：新增 `arm64-native` job（QEMU）与 `manifest-merge` job，semver/latest 变多架构清单（arm64 失败时自动回落仅 amd64，不阻断发布）。

### C4 新手运营双模板（迁移 0226）

- **安装向导第 ④ 步（可选）**：站型之后可选新手运营模板——**考核淘汰制**（NexusPHP 式：入职考核自动派发 + H&R 从严 + 低保户降级）或**缓冲宽进制**（UNIT3D 式：初始上传缓冲 + H&R 宽限预警 + 无考核）。跳过 = 维持现状。
- 实现为 `onboarding_preset` 设定键 + 两套参数簇写入（等级 demotable/HR 阈值/考核开关与宽限/初始缓冲），**不动已有用户数据**，后台 `?tool=onboarding` 一页可随时查看与切换。
- 目的：「不偏向任何 PT 类型」延伸到运营风格层——两种已被竞品源码级验证的新手哲学成为站长的一键选项。

### C5 经济反通胀面板

- **后台「经济仪表」**：`GET /admin/economy-dashboard`——近 7/30 天火花产出 vs 消耗曲线、池子存量、Top 消耗 SKU、人均持币、净通胀率；数据全部来自 spark_ledger 既有流水，零新表。
- **购买上限阀门**：新设定 `economy_max_buffer_gb`——用户缓冲量（上传-下载）超过该值后禁止再购买上传量类商品（UNIT3D `max-buffer-to-buy-upload` 同款反通胀阀门；0 = 不限制，缺省）。
- **用户侧「我的魔力明细」页**：`/me/sparks` 按天分页展示收支流水（触点 #11「积分感知透明化」）。

### P2 首批（触点补齐）

- **等级要求公开页 `/classes`**：只读展示全站等级门槛与特权（class_rules 单源），可在「菜单管理」挂入导航（触点 #2）。
- **一次性邮箱域名黑名单**：新表 `banned_email_domains` + 注册/邀请绑定校验 + 后台维护（触点 #4）。
- **登录 cookie Secure flag 显式化** + **账户级登录失败锁定**（连续失败 N 次锁 M 分钟，Redis 计数；与既有 IP 限流叠加）（2.7 节安全小补）。
- **彩虹 ID / 用户名染色 SKU**：装扮体系新增可购 SKU（按天/永久两档），用户名渲染走既有 dressup 通道（触点 #17）。
- **自助解封（首次宽恕）**：被禁用户在冷却期内可自助解封一次，`self_unban_used` 标记防重复；后台可查记录（触点 #24，对齐 NexusPHP 1.10.2 卖点）。

### C6 工具生态收录（准备件）

- `docs/ops/ecosystem.md`：PT-depiler 站点定义样例（走 `ptppUserInfo` + 开放 API 的自研站通道）、Jackett 通用定义映射（基于 `/compat/nexusphp` 形状）、cross-seed/Torznab 口径与负载建议。对外提交动作待仓库公开后执行。

### 升级注意

- 0226 只新增设定键/表/页面，不改既有数据；`economy_max_buffer_gb` 缺省 0（不限制），彩虹 ID SKU 与自助解封需站长在后台主动启用/上架才对外可见。
- 升级后建议过一遍 `docs/webmaster/launch-checklist.md`（新站）或直接继续运营（存量站无破坏性变更）。

## 0.2.0 前的迭代批次（0.2.0 发布时未入版本段的记录，考古用）

### 全量代码审查修复批（2026-09-28，迁移 0237）

对 G30-A/B/G31 全部 13 个 commit 做独立缺陷审查（P0×5/P1×8/P2×8），落地修复：

- **P0 归档年月解析死代码**：`rsplitn(2)` 倒序切片使年份 parse 恒失败——
  DETACH/DROP 一次都不会执行；改右切两段取年月。
- **P0 balance_baseline 余额基线**：users 快照是流水 SUM 派生（非独立权威），
  直接归档会把老用户 uploaded/downloaded/spark 清零——新增基线表（迁移 0237），
  DETACH 前冻结期初余额，三处重算公式（announce 增量/reconcile 全量/seeding
  小时重算）改为 baseline+SUM(剩余窗口)。实证：冻结后重算差额精确补回。
- **P0 posts（论坛正文）移出归档清单**：业务数据误圈入流水归档，retention
  会删帖；分区预建保留。
- **P0 ensure_group 游标读失败 fail-hard**（静默回落 $ 会丢升级窗口事件）+
  reclaim idle 120s→360s（恢复旧 6 轮 DLQ 节奏）。
- **P1 三件**：tracker peer 外置 upsert Lua 原子化（读改写竞态吞回连测量值）、
  种子详情主查钉主库（purchased 写后立读）、`Repo::read_fallback` 副本运行期
  故障回落主库（热点读不再因副本宕机 500）。
- **审查遗留 #1 闭环**：UDP announce 接入 peer 外置存储（外置模式下与 HTTP
  同读写 Redis swarm，双协议互见）；顺带修复 UDP 响应 interval 按 i64 写
  8 字节的 BEP-15 格式 bug（V17 实测）。
- 其余遗留 6 项见配方 §8.5（均非活跃缺陷）。

### UDP tracker 修复 + 多核收包（2026-09-27）

- **⚠ 修复 UDP tracker 从未可用**：connect 协议魔数误写为 0x41727109807a
  （BEP-15 规定 0x41727101980），所有标准客户端的 UDP 请求被静默丢弃。
  实测修复后 connect→announce 全往返通。
- **UDP 多核收包**：`TRACKER_UDP_WORKERS=N`（缺省 1，行为不变）以
  SO_REUSEPORT 起 N 个收包 socket（内核负载均衡；仅 Unix，Windows 自动
  钳回 1）。XL 档触发点就此关闭。
- 配方 §8.4 含 Linux 宿主验证命令（Docker Desktop UDP 回程不可用于验证）。

### 数据层规模支撑 G31-D 批（2026-09-27，迁移 0230；配方 §8 规模矩阵）

- **部署规模矩阵**（配方 §8）：S/M/L/XL 四档——同一份代码伺候大小站，
  升级=改配置换配方；明确不做分片（计费流水追加型，冷热分层+副本已覆盖）。
- **读写分离钩子**：`DATABASE_REPLICA_URL` 配只读副本（流复制/托管副本），
  `Repo.read_db` 分流热点读；未配置 = 零行为变化（同句柄），副本连不上启动
  自动退化主池。已分流：种子列表/详情系/首页/用户公开主页（四文件 36 处，
  均验证纯读）。**钉主库清单**（写后立读/鉴权/支付）写入配方 §8.1。
- **流水归档可配**：`ledger_retain_months` 站点设定（0=永久缺省）——worker
  每日对超期分区**先 DETACH（秒级可回滚）下周期 DROP**；归档范围仅
  traffic/spark 两张流水表（posts 论坛正文是业务数据不归档——审查修正）；
  迁移 0230
  （⚠️ settings_meta 的 FK 指向 site_settings，必须先插设置行——首版顺序
  颠倒曾致启动失败）。
- compose 预设：`max_connections=200`（两份 compose）、`DATABASE_REPLICA_URL`
  注入位。
- 实证：`_v_g31_datalayer.py` 5/5（无副本退化/归档配置落库/DETACH→DROP 两步
  语义）；`_v_g30_multi.py` 21/21 回归无破坏。

### 多机部署 G30-B 批（2026-09-27，迁移 0225；配方 §6.5）

- **计费流消费者组**：`flux:announce`/`flux:agent_block` 从全局游标 XRANGE 改
  XREADGROUP——多 worker 并发消费真分摊（此前靠 advisory 锁互斥 = 热备）；
  崩溃实例的 pending 由 XAUTOCLAIM（>120s）回收；失败留 PEL 重试、6 次进
  DLQ 语义不变；旧游标首启自动迁移退役。
- **worker 角色分片**：`FLUX_WORKER_JOBS=job 名清单` 让实例只跑清单内定时任务
  （手动触发不受限）；`job_status.executed_by`（迁移 0225）面板可见分实例。
- **tracker 挂 LB**：HTTP announce 支持 `TRUST_PROXY=1` 取 XFF 首值，peer IP
  不再被记成 LB 地址。
- **web 上游运行期可换**：新增 `app/api/[...path]/route.ts` 运行期代理
  （API_SERVER_URL 环境变量即换，无需重打镜像），构建期 rewrites 降为兜底。
- **任务面板分实例可见**：`/admin/jobs` 返回并展示 `executed_by`（前端实例列，
  三语）——多 worker 下「最近运行」看得出谁在跑。
- **tracker peer 外置**（收尾批）：`FLUX_TRACKER_PEER_STORE=redis` 启用 swarm
  级 Redis Hash——多 tracker 副本互见不互抹（双 tracker 实测 complete=2）；外置
  模式停用旧 60s 覆盖快照（互抹根源）；缺省空 = 内存单机行为不变。
- **worker 60s 分支并发化**（收尾批）：独立任务 JoinSet 并发（900s 慢任务不再
  堵同轮；顺序链保留）；**连接池 5→16（DB_POOL_SIZE 可配）**——并发调度后 5
  连接必打满，PoolTimedOut 曾被误报成「另一实例锁冲突」（run_guarded 文案同步
  修正为两种可能）。
- 实证：`_v_g30_multi.py` 21/21（V7 组/消费者≥2/游标退役、V9 代理通）+ 收尾
  实测：计费入账单行不双计（announce→stream→组消费→ledger→快照全链）、
  XFF 双档（TRUST_PROXY 未开伪造 XFF 被忽略=socket IP；开启后事件 ip=XFF 首值）、
  FLUX_WORKER_JOBS 白名单实例不越界、双 tracker 互见（V13）。

### 多机部署 G30-A 批（2026-09-27，迁移 0224；方案 _doc/G30-多机部署开发方案-2026-09-27.md，配方 _doc/G30-多机部署三机配方.md）

- **多副本部署支撑**：新增 `docker/compose.multi-node.yml`（profiles 分机取子集、无
  container_name 可 `--scale`）与 nginx LB 样例；web 补 `/health` 端点与 healthcheck
  （此前唯一无判活依据的服务）；全服务加 `stop_grace_period`（api 45s / worker 90s）。
- **worker 优雅停机**：SIGTERM/SIGINT → 停止认领新任务 + 在跑手动任务 60s 排空
  （`apps/worker/src/shutdown.rs`）；此前 docker stop 10s 即 SIGKILL 硬掐。
- **任务认领防互踩**：`job_triggers` 认领语句加 `FOR UPDATE SKIP LOCKED` 并记录
  `claimed_by` 认领实例——多 worker 并发不再产出「另一实例正在跑」的假失败。
- **跨进程配置失效通道**：`flux:cfg:ver` 版本键 + 3s 轮询（术语/模块开关两域），
  多副本下 A 改文案 B ≤5s 跟随（此前永不跟随直到重启）；移除全仓零订阅者的
  `settings:changed` 死发布点；Redis 故障时降级回 30s TTL 行为。
- **运行日志实例维度**：`runtime_logs` 加 `instance` 列（`FLUX_INSTANCE_ID` 优先、
  缺省回落容器 hostname），后台「运行日志」页多实例筛选器（单实例自动隐藏）。
- **迁移单飞开关**：`FLUX_BOOT_MIGRATIONS=0` 跳过启动迁移与种子（第二副本起用）。
- **⚠ 破坏性变更**：`JWT_ALG=rs256` 未提供密钥（`JWT_RS_PRIVATE_PEM/JWT_RS_PUBLIC_PEM`
  或挂卷的 `JWT_RS_KEY_DIR` 密钥文件）时**拒绝启动**——原先静默自动生成的密钥在容器
  重建/多副本下互不相认（A 签 token B 验不过）。依赖自动生成的部署请先按配方文档生成
  一次密钥对并挂卷注入；默认 hs256 用户零感知。
- 实证：`_v_g30_multi.py` 17/17 全绿（双副本 JWT 互认 / 通道 bump / syslog 实例筛选 /
  双 worker 并发认领实战——12 任务 6/6 分流两实例、零「另一实例」假失败）。

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

### 通用建站定位四审 C 批（模块开关覆盖，2026-09-25）

- **后台面板随模块开关消失**（0196）：`staff_panel_entries` 新增 `module_key`，9 条明显
  从属某模块的条目挂键（考核配置/绩效考核/勋章管理/签到记录/任务配置/金字字幕评选/
  道具管理/投票/保种统计），面板查询按 `modules.is_on` 过滤；此前 29 个模块只在前台四处
  一致，后台条目只看 `min_class`。迁移内含防呆：`module_key` 指向不存在的键直接 RAISE。
  实测 `exams/jixiao` 关闭时条目 62 → 60。
- **invites 进模块注册表**（0197，第 30 键）：网关映射、worker `job:expire_invites` 归属、
  页面 `requireModule`、导航与用户菜单入口 `mod()` 过滤、11 个站型包 `modules` 快照补键。
- **0198 补齐后加模块键的开关面**：`site_settings` 的 `module_<key>` 值行与 `settings_meta`
  登记行（`settings_meta.name` 有 FK 指向 `site_settings`，必须先插值行）。缺它会出现
  「注册表里有该模块、后台没有开关、写设置被『未知设定项』拒」——0197 首次落地即踩中。
- **首页板块清单单源化**（四审 L6）：键、推荐占宽、是否进默认排版收进后端唯一一份
  `HOME_SECTIONS` 并随 `/home.home_sections` 下发；前端删掉自带的三份副本
  （`HOME_SECTION_KEYS` / `DEFAULT_HOME_LAYOUT` / `RECOMMENDED_SPAN`）。两个行为变化：
  ① `latest`（海报墙）自 0089 就在白名单里、但前端 switch 一直没有分支 ⇒ 排序与占宽
  对它无效、只能硬钉在页面末尾，现在它和其它板块一样吃排版；② 排版里出现未知键/重复键
  从「整份作废回退默认」改为「跳过该条」（手改库不再一坏全坏）。`/home` 共享缓存键
  v1→v2（旧 payload 缺 `home_sections` 会让新前端把首页渲染成空白）。新增
  `scripts/home_sections_guard.mjs` 比对后端清单与前端渲染 `case` 集，漂移即失败，
  已挂进 `scripts/ci_local.sh`。
- **自建产物也能挂模块开关**（0199）：`custom_pages` 与 `user_field_defs` 新增可空
  `module_key`。挂上后该模块一关，页面 `/p/{slug}` 与注册页/usercp/公开档案里的字段
  一并下线；指向某自定义页的**菜单项**也跟着消失（不留点开 404 的死入口）。
  写入侧校验「未知模块键宁拒不留」。两个后台面板共用新增的 `ModuleKeySelect`，
  其中「启用/停用」与「编辑」两处是手拼 PUT body——都补带 `module_key`，
  否则全量覆盖 PUT 会把挂载静默清空（分类那边报过的同一形态）。
- **修正 0196 的源读错**：后台面板过滤原先读 `modules.is_on`（注册表种子值），
  而模块开关的权威是 `site_settings.module_*` —— 站长运行时关一个模块并不会让面板
  条目消失。判据收进 `modules::module_on_sql()` 一处生成，面板/页面/字段/菜单共用。
  复验 14/14：关 `subtitles` → 页面 404 且菜单链接消失、重开恢复；挂 `exams` 的字段
  在注册页随开关出现/消失；关 `medals` → 「勋章管理」从后台面板消失（旧实现不会）；
  挂未知键被拒。
- **消灭模块开关的双源**（0200）：删除 `modules.is_on`。开关真值只有
  `site_settings.module_<key>` 一处，读不到时回落 Rust `default_on()`（general 中立矩阵）。
  起因是 0196 把面板过滤写成读 `modules.is_on`（注册表种子值，后台改开关不会动它），
  站长运行时关模块面板条目不消失——同类误读几乎必然复发，所以把那列删掉。
  删列前核实过：代码唯一访问 modules 表的语句是 `EXISTS(key)`，`pg_proc`/`pg_views`
  无引用；历史迁移（0107/0178/0179/0197/0198）都排在它之前，不改写任何历史文件。
- **修 0197 自己埋下的新漂移 + 补三道门禁**：`packages/domain-types` 里
  `ModuleKey` 联合与 `MODULE_KEYS` 数组是**两份手抄清单**，加 invites 时两份都漏了，
  而原有 `check_type_drift.mjs` 只管错误码/促销枚举/响应形状，管不到模块键。
  现改为数组是唯一清单、union 由 `typeof` 派生；新增 `scripts/module_keys_guard.mjs`
  比对 Rust `key::ALL` ↔ TS `MODULE_KEYS` ↔ DB `modules` 表（已验证：故意删掉
  `invites` 会红并点名），与 `home_sections_guard.mjs`、`check_type_drift.mjs` 一起
  挂进 `build-test.yml` 与 `scripts/ci_local.sh`。
- 复验（新卷 + 新镜像）：空库装机闸门 11/11、自建产物挂开关 14/14 全通过，
  `modules` 列集为 `key,name_zh,name_en,descr,grp`。
- 复验：`cargo test --workspace` 114 passed（含 `key_count` 29→30、3 条首页清单契约）、
  web `tsc` 零 error；新镜像上 invites/面板 11 条断言全通过（关闭后 `/invites/status` 与
  `POST /invites` 均 4101「本站未开放此功能」；面板 62→60 去掉考核配置/绩效考核；
  首页渲染 A/B 开 150756 / 关 150715 字节，差的正是那条导航入口），
  首页单源 API 面 7/7 + 浏览器 DOM 实测（默认 9 格占宽合清单；把 `latest` 排到第 1 格
  `span-1` 后第 1 格确为海报墙；库里手塞坏键时只剩 1 格，旧行为会回退成 9 格）。

### 通用建站定位四审 L6 收口：SEO/OG/sitemap 面板（2026-09-25）

- **SEO 设置面终于接上前台**（0201）：根布局 `generateMetadata` 改为消费
  `/site-profile` 新增的 `seo` 段——站长填的 META 描述/关键词真的落到
  `<meta name="description">`/`keywords`/`og:*`/`twitter:*`。此前 `metadescription`
  只有 RSS 消费、`metakeywords` 零读取，设置页那排框是四审 L6 点名的假开关族；
  同时摘掉两个纯装饰键 `titlekeywords`/`cssdate`，新增 `seo_indexable`
  （**缺省 `no`**：私有站被搜索引擎抓走是事故不是特性，要放开得站长显式打开）。
- **描述三级回落**：META 描述 → 站点简介 `site_desc` → 自带词表。自带文案写的是
  「通用 PT 建站系统」，与「不偏向任何 PT 类型」的定位相反，所以站长自己的话
  必须排在它前面。
- **新增 `/robots.txt` 与 `/sitemap.xml`**（全仓此前没有）：未开收录时 robots 是
  `Disallow: /`、页面带 `noindex`、sitemap 不给条目；开收录后 robots 放行全站但
  仍排除 `/admin/ /me/ /api/ /checkout/ /messages/`。sitemap 只列**匿名可达**地址
  （首页 + 公开自定义页）——把需要登录的种子详情写进去等于给爬虫一堆死链，
  为此新增匿名接口 `GET /api/v1/public-pages`。
- **复验抓到两个真缺陷**（都不是「没做」，而是「做了但被别的环节吃掉」）：
  ① 站点准入闸门把 `/robots.txt`、`/sitemap.xml` 一起 307 到 `/login`，爬虫永远
  拿不到 ⇒ 收录开关与两个机器文件全是摆设；已把两者与 `/p/{slug}` 放进匿名白名单。
  ② `PUBLIC_SITE_URL` 未配置时不编假域名（`siteBase()` 返回 null，
  `metadataBase`/`og:url` 省略），不再编一个假域名。
  ③ 顺带补上 `/p/{slug}` 自己的 `generateMetadata`：sitemap 里那些页面落到前台时
  `<title>` 全是全站同一个「站名 · 后缀」，等于给爬虫一串同名页。
- **门禁收口**：本批与上一批把自己的文件撑过了 300 行/80 列门禁（10 项违规），
  现拆回全绿——`sitetype.rs` 拆出 `profile_bits.rs`（`setting_text`/`seo_block`/
  `theme_tokens`），`user_fields.rs` 拆出 `user_fields_def.rs` 并删掉从未被调用的
  `mount_user_fields`（路由实际在 `http/mod.rs` 注册），`staff-user-fields.tsx`
  三处逐字段手抄的 PUT body 改展开（PUT 是全量覆盖，漏抄一个就静默清空那一个）。
- 升级注意：0203 把 `seo_indexable` 从 `main` 分区挪回 `tweak`——浏览器复验时发现「SEO 与统计」这张卡按 `grp` 出现，四个 SEO 键的 `grp` 不同就会在两个分区各冒出来一次，收录开关孤零零一个字段。
- 部署接线：`docker-compose.yml` 的 **web 服务此前不收 `PUBLIC_SITE_URL`**（只有 api 收），而
  `metadataBase`/`og:url`/sitemap 绝对地址读的正是 web 容器内这个变量 ⇒ 官方镜像经 compose 部署时
  `og:url` 静默省略、sitemap 落相对路径。现与 api 同口径转发，`.env` 里设一次两边都生效；
  不设 = api 回落 `localhost:3000`、web 省略绝对地址（不编假域名）。
  - 升级注意：0201 删除 `titlekeywords`/`cssdate` 两键的值与元数据行、新增
    `seo_indexable`（默认 `no`）。既有站点前台描述原本取字典默认，若设过 `site_desc`
    现在会改取它；打开收录前对爬虫无变化。
- 复验（新镜像 + 独立 web 容器 24 条断言）：含「翻 `seo_indexable` 一个开关，
  robots/noindex/sitemap 三处一起翻」、匿名取 `/robots.txt` 不再被 307、
  `og:url` 落在部署域名、清空 META 描述后回落站点简介；对照组保留「未登录首页
  仍被准入闸门挡住」，避免拿 `/login` 的根布局 metadata 冒充首页。
  `cargo test --workspace` 138 passed、`tsc --noEmit` 零 error、`cargo fmt --check` 干净、
  四道跨语言门禁（行数/行宽、模块键、首页板块、type-drift）全绿。

### 通用建站定位四审 L7 收口：术语表机制（2026-09-25，0205/0206）

- **术语是行表，不是列**：新增 `site_terms(canonical → replacement, enabled, sort)`，
  站长在「管理组面板 → 术语表」把「种子 / 魔力 / 保种 / 邀请」这类写死的固有词
  改成本站叫法。为什么不塞进 `site_settings` 一个键：术语是「一组可逐条增删启停的
  行」，KV 里只能编分隔符（`forum_banned_words` 那种换行分隔的 text 就是前车之鉴）。
- **两个出口、一份语义**：
  - 前端只改一处——`getDict()` 出口把整本字典的字符串叶子过一遍规则，于是 236 个
    `useI18n` 消费点与全部服务端组件自动跟随，不必把 5000 行三语字典重写成占位符；
  - 后端在响应信封出口（`errors.rs`）同口径改写，390 条中文校验串一并跟上。
    两侧共享同一套规则语义：**长词优先、单次正向扫描、替换结果不再参与匹配**
    （「种子↔资源」这种互指规则不会级联或死循环）。
- **零规则 = 零行为**：表是空的，新装与升级后的站点文案一字不变。预置规则由站型包
  携带（0206 新增 `site_type_packs.terms`，沿用「NULL = 本包不声明」口径 ⇒ 切换
  站型不会擅自改写谁的词汇表；数组才是显式覆盖）。另存快照会把术语段（含 `enabled`）
  一起采进包，不再犯 0193/0197 那族「另存丢载荷」。
- **`{magic}` / `{n}` 占位符保护**（自查出的缺陷，未上线即修）：改写发生在**模板**上，
  字典里的 `{magic}` 是站点货币名出口、`{n}` 是插值变量；若某条规则的原词正好是
  `magic` 或 `n`，不保护就会把占位符撕开，`fmt()` 取不到变量、文案当场少一块。
  现在两个出口都把 `{...}` 当原子片段跳过，写入侧另拒含花括号的词条；Rust 与 vitest
  各留一组**逐条镜像**的用例锁住同语义（一端漂了另一端就红）。
- **假开关守卫**：`scripts/terms_guard.mjs` 拿每条**启用中**的规则去两份语料
  （源字典 + 后端文案）里找命中，零命中即 FAIL 并点名——防的正是本档反复出现的
  「登记了但没东西读」。已实测：插一条 `绝无此词zz` 会红并点名，删掉后转绿。
- 升级注意：0205 建新表 + 一条后台导航条目，0206 给 `site_type_packs` 加可空列；
  两者都不写任何规则行，因此升级后前台文案与升级前逐字相同。
- 顺带记录一处既有假开关（**本批未动**）：`site_settings.subtitle_label`（0146）
  由 `/site-profile` 下发但前端零消费，字幕区显示名实际仍走字典。术语表落地后它应
  由包预置规则（字幕→歌词）取代并退役，留给下一批清理。

### 通用建站定位四审 L7 第二批：后端校验详情三语化 + 棘轮门禁（2026-09-26，无迁移）

- **译文表按「原句」查，不改任何调用点**：新增 `apps/api/i18n/validation_details.tsv`
  （`zh-CN 原句 → zh-TW → en`，`include_str!` 编译期内嵌、运行时零 IO），
  `errors.rs` 在响应信封出口把 `Validation` / `TorrentInvalid` 的详情与
  `FieldErrors` 的逐字段消息一起过这张表。
  为什么不做成「每条一个 key」：那要改 500+ 个调用点（多数在别人正在写的文件里），
  而且 key 名本身是第二份要维护的账；代价是**中文原文改了就要同步表**——
  这条由下面的棘轮当场拦住。
- **表里没有的原样透出**：宁可露中文，绝不现场编译文（编出来的错话没人审得动）。
- **棘轮门禁** `scripts/validation_i18n_guard.py`：静态校验串必须进表、
  `format!` 动态串条数不许上涨；基线**按文本**记账而不是按行号
  （`i18n_guard` 按行号记，插删一行就把存量判成新增——这是它现有的坑，新门不再犯）。
  已实测会红：临时插一条未译静态串 + 一条 `format!` 串，两条各点名一次、exit 1；
  删掉后转绿。
- 现状数据（诚实记录，不是「已收口」）：静态详情句 **505** 条（出现 596 处），
  第一批覆盖 **24** 条（`auth_http` 登录/注册/改密/自身设定/自定义字段 + 术语面板
  四条），覆盖率 **4.8%**；另有 `format!` 动态串 **94** 处，必须先参数化成 key
  才能覆盖，属下一批的活。
- 补一处自己上批留的洞：`scripts/terms_guard.mjs` 写了但**没接进 CI**，
  等于挂了个不会红的门——现已接进 `build-test.yml` 的 web job；
  新棘轮接进 rust job，两者也都进了 `scripts/ci_local.sh`。
- 与术语表（0205）的分工（容器级复验实测出来的，初版文档写错了）：出口顺序是
  **先查译文表、再过术语层**。所以①已进表的英文译文不会被中文术语规则动到；
  ②**没进表**的句子回落中文后，仍会按站点词汇改词——即便请求方是 en。
  即「站点词汇优先于源文案」，中文兜底不是一块死文本。
- 复验：`cargo test` i18n 7 条全过（含一条端到端：同一个 `Validation` 在
  `Accept-Language: en / zh-TW / 缺省` 下分别得到
  `Validation failed: Current password is incorrect` / `參數校驗失敗: 舊密碼不正確` /
  `参数校验失败: 旧密码不正确`）；表自身形状有单测锁（三列齐、原句不重复、
  三语 `{...}` 占位符集合必须一致）；两项门禁本地全绿。

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
