# 更新日志（Changelog）

本文件记录面向部署者的显著变更。格式参照 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)；
版本号在首个语义化 tag（v0.x）发布后启用。

## [0.5.1] - 2026-10-26

v0.5.0 之后的 13 个提交（fix / ci / docs / chore，无 feat、无迁移）。定位：发布后
回读发现的 CI 红账清偿 + 依赖安全补丁版。**无数据库迁移，升级即拉镜像重启。**

### 修复（对 0.5.0 已知问题的清偿）

- **wasmtime 49.0.3→49.0.4**：RUSTSEC-2026-0379（aarch64 大 `br_table` 跳错目标，
  9.9 critical）——0.5.0 已知问题里点名的承重面告警，本版落账
- **rustls-webpki 0.103.15→0.103.16 / webpki-roots 1.0.9→1.0.11**：证书验证可接受
  不受信中间证书 / 内置根信任 emSign 不受信中间证书（RUSTSEC-2026-0382/0381）
- **api 挂 GeoLite2 只读卷**：CI 构建的镜像内无 GeoLite2 库，GeoIP 归属地静默变空
  （本地构建有库而 CI 版没有，部署后才发现）；compose 已补卷挂载
- **e2e/闸门口令依赖收口**：装机步随机化 root 口令后，处置闸门改自铸职员令牌、
  e2e 作业改写死凭据口径，不再按口令登 root（避免把真 root 推进防爆破锁定）

### 治理

- **两支守门的红账清偿**（v0.5.0 的 524 commit 带入、此前被 CI 早停在 Fmt/Line
  limit 掩盖，fmt 清零后才首次跑到）：`ui_design` 野断点 2 处真修（works.css 860px
  收窄到 md 档 768px、ui.css 767px 改 `not all and (min-width:768px)` 等价写法），
  色值 +14 / z-index +5 系续补漏交源文件带入的存量，按实登记基线；
  `validation_i18n` 30 条静态串补译进 `validation_details.tsv`（覆盖率 27.5%→31.1%），
  动态 format! 串 229→245 按实登记（H&R 三档/加价/赠礼等新功能带入）
- cargo fmt 归一（49 处→0）、路由清单刷新、line/layer 存量按实登记——
  六支守门本地全 rc=0
- **crypto-bigint（RUSTSEC-2026-0387）进 `.cargo/audit.toml` 豁免清单**：修复需
  p256 0.13→0.14（semver-breaking，牵动 passkey 登录 + WebPush crypto），属迁移级
  变更不塞补丁版；暴露面评估与复查条件已写进豁免注释，p256 升级批一并移除
- tokio unsound 警告（RUSTSEC-2026-0386）与 wasmtime medium 条目维持观察（无承重面）

### 发布前置已验

`cargo check --workspace --all-targets` rc=0；`cargo fmt --all --check` rc=0；
`ui_design_guard` / `validation_i18n_guard` 本地 rc=0；Cargo.lock 仅 3 成员版本 +
2 个 webpki 升级，无其他漂移。

## [0.5.0] - 2026-10-26

524 个提交（feat 14 / fix 359 / test 86 / docs 15 / chore 42）：自动审种全批落地、
作品域与论坛引用种子、可见性与资金守恒两族深测收官。**含迁移 0393–0421（29 个），
升级自动执行**（api 启动时跑）。上一版 v0.4.0（10-18）未在此记档，本段起点即 v0.4.0 之后。

### 新增

- **自动审种全批**：规则内核 + 影子引擎（只判不改）+ 裁决同尺 + 管理面 + 邮件 outbox，
  通知四要素与风暴闸
- **作品域**：`/works` 读口与 `work_meta` 缓存；作品门面 + 种子行 + 单行相关推荐
- **论坛**：帖子/回帖引用种子卡片（`!torrent(id)`）；系统播报默认迁出 shoutbox，
  独立 `site_activity` 动态流
- **商城**：赠送魔力真正可赠（接收人参数 + 效果记在收礼方）
- **推送**：三档逐类开关，接上此前「只存不拨」的档位
- **域名库**：面板「从文件导入」，大清单不再受 JSON 文本框长度限制
- web 路由骨架全覆盖：补齐 32 个 `loading.tsx`

### 修复

- **可见性一族**：相关种/同组口不再对不可见种放行、感谢口拒坏载荷与隐种、字幕上传补
  准入门（P-643~P-645）；收藏与详情同口径 + 角标等于点进去看到的条数（P-439）；
  RSS/Torznab 不再向外部客户端广告无存档种（P-433，两出口共用 `HAS_ARCHIVE_CLAUSE`）
- **资金守恒一族**：批量收种链补 CSRF（浏览器整条 403 的洞）、商店两段写与落空退款
  改业务口径（P-434）、抽奖奖池参与不足额时不再静默蒸发、写口幂等键三种错法收口
- **口径分叉**：IMDb 检索读幻键 + 漏 `imdb_id` 列（P-436）、邮箱大小写两侧不同尺导致
  混合大小写邮箱永久收不到重置信（P-437）、等级写口整对象 PUT 省略字段即静默反做
- **tracker**：含空格字节的 `info_hash` announce 恒 unregistered（裸 `+` 未
  percent-decode）

### 治理

- 常驻深测闸门 265 支 `scripts/pt_audit_*.py`；本批新增 TV（可见性 oracle）/
  TW（感谢金额）/ TX（受限版块读口 **17 条**，含 `/forums/side` 侧栏双榜与
  数据层 A/B 负控）。条数一律按运行时 `[PASS]` 行数计——闸门里的 `ok()` 大多
  写在循环内，静态数不出来。升级后 13 支常驻闸门在现网复跑：
  **235 条断言全绿、0 红、残料 0**（`forums=9 / topics=47 / posts=54` 前后逐字一致）。
- 新增漏交文件守门 `scripts/repo_hygiene_guard.mjs`（mod 可解析 / 迁移无断档 /
  `include_*!` 字面路径 / tsconfig references 四口径）

### 已知问题（不影响运行，影响 CI）

推送后回读实测，`build-test` 的两个作业都**在守门之前就更早停止**：rust 作业停在
`Fmt`（`cargo fmt --all --check` 现存 49 处 diff，本批未碰其中任一文件），web 作业停在
`Line limit guard`（55 条超宽/超限）。⇒ 本地量出的 `layer_guard` 57 条与
`route_manifest` 5 条差异，这两步**在 CI 里根本没被执行到**。

`security-audit` 同时报红：6 条 RUSTSEC 告警，其中一条是 **wasmtime 9.9 critical**
（RUSTSEC-2026-0379：Cranelift 后端在 aarch64 上处理大 `br_table` 可跳到错误目标；
修复 `>=49.0.4`）。本站 `adapter_seed.rs:13` 直接内置 douban wasm 适配器、
`adapter_runtime.rs:106` 起 wasmtime 引擎，而发布 CI 又构建 arm64 镜像
（`publish-images.yml` 双平台矩阵）⇒ 这条告警落在承重面上，不是纸面风险。
当前 `Cargo.lock` 为 wasmtime 49.0.3，只差一个补丁版本；**本版未随发版改依赖**，
建议单独一笔 `cargo update -p wasmtime --precise 49.0.4` 后复跑 cargo check + 闸门。
其余五条：`crypto-bigint`（定时无变性，需 `>=0.7.6`）、`rustls-webpki`（可接受
已被不信任的中间证书）、`webpki-roots` 1.0.9（内置根信任 emSign 中间证书，需 `>=1.0.10`）、
`tokio` 1.53.1（`AsyncFd::new` 违反 I/O safety，unsound 警告）、
wasmtime 另一条 medium（component model stream 降级 panic）。

发布前置两条已验：`cargo check --workspace --all-targets` rc=0；`_sqlx_migrations`
415 行与 `apps/api/migrations` 415 个文件双向求差为空。

## [0.3.2] - 2026-10-04

10 个提交：部署可观测、NP 口径对齐、邮箱后缀策略。**含迁移 0276–0282（7 个），升级自动执行。**

### 新增

- 后台「检查更新」：运维卡一键比对 GitHub 最新 Release（离线不影响使用）
- 匿名使用统计（opt-in 默认关）：站长开启后每日上报匿名版本与规模计数
- 页脚「Powered by FluxTorrent」链到 `/about` 版本页（程序版本/开源仓库/建站文档）
- 注册/邀请邮箱后缀策略（0282）：不限制/白名单/黑名单三模式；受限后缀发邀请时按钮实时禁用+提示
- `/api/v1/health` 附版本号，报障对齐口径
- CI 可选同步镜像到 Docker Hub（配 `DOCKERHUB_*` 密钥后生效，公开拉取计数）

### 改进

- **默认等级体系对齐 NexusPHP 官方**（0277）：Peasant→Nexus Master 十级 + NP 门槛梯度（50G/4周→3TB/100周）；已运营站可后台改回
- **经济默认值对齐 NP**（0278）；**商店实跑价目定稿**（0279–0281）：NP 相对结构 + 本站 ×10 水位维持饥饿感；邀请/自定义头衔高定价
- 猜大小三区 tie→triple（豹子）口径；九宫格返本档不占灯格

### 修复

- 等级要求页「页面出错了」：`/classes` 双路由冲突，数组形态抢占了对象形态接口
- compose 不再默认挂宿主迁移目录（v0.3.1 升级实测踩坑：旧目录遮住镜像内新迁移）

## [未发布] - 2026-10-04

### 新增

- 一键部署脚本：端口冲突自动避让；向导期间自动开放 3000、装完自动收回
- 反向代理一键补全脚本（宝塔/系统 Nginx 通用）
- 安装向导：Tracker 地址智能建议与收尾指引
- 站内帮助中心 `/help`；对外文档站 [wiki.ptang.top/ft](https://wiki.ptang.top/ft/)

### 改进

- 宝塔部署手册重排：全程地图、每步完成标志、向导四屏对照

## [0.3.0] - 2026-10-03

151 个提交：娱乐屋全生态、抽卡收藏、多机部署、UDP tracker、四语化。**含迁移 0227–0272（46 个），升级自动执行。**

### 升级注意

- `JWT_ALG=rs256` 须挂载密钥（hs256 默认用户零感知）
- 对外列表（兼容层/Torznab/RSS）默认含零做种新种
- `.env` 的 `CORS_ORIGINS` 生产必须改为自己的域名
- 0230 起流水归档可配；论坛帖永不归档

### 新增

- 娱乐屋九款玩法（钓鱼/猜大小/九宫格/刮刮乐/大转盘/扭蛋/农场/牧场/宠物）+ 经济安全三闸 + 运营后台面板
- 抽卡与收藏册：十连真实单抽、碎片分解兑换、个人收藏页
- 多机部署：计费流消费者组、worker 分片、tracker 挂 LB（三机配方已实证）
- UDP tracker 首次真可用（修复 BEP-15 魔数，此前静默丢弃全部标准客户端）
- 四语化（zh-CN/en/ja 段级回落）；passkey/WebAuthn 第二通道；可插拔验证码
- 开放 API 深化 + `openapi.json`（4→27 路径）+ 第三方工具维护者对接页
- 深度运维：读库分流、流水月分区归档、余额基线表、监控告警、升级演练脚本

### 修复

- PT 硬度深测四轮修复：tracker 种子白名单、`sum()` numeric 归零、H&R 三口径统一、RSS 付费种过滤等
- UI 深色模式完全失效等四 P0；品牌资产全套重制
- 生态兼容复测：RSS 直链、Torznab caps、Prowlarr/Jackett 添加索引器修复

## [0.2.0] - 2026-09-27

竞品对比行动批（C 批）。**含迁移 0226，升级自动执行。**

### 新增

- 文档中心 `docs/` 四册 22 篇（站长/用户/自定义/运维）；README 全量重写
- 仓库卫生：根目录 520+ 开发残留清出（`_attic/` 本地化，不入库）
- 版本号全仓统一 0.2.0，首个语义化 tag `v0.2.0`
