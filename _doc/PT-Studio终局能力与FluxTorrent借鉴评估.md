# PT Studio 终局能力 × FluxTorrent 借鉴评估（对标「成熟后形态」）

> 前提：上一次对比看的是「PT Studio 现在写了多少代码」；本篇看的是它的**设计文档/路线图规划的终局形态**——即它成熟后会有什么。结论基于 `docs/design/09-roadmap.md` §4「未完成与待做」+ 各插件 `01-需求规格说明书.md`（已读 anticheat 全文、16 个插件功能表、lessons-learned）。  
> 判定分五档：🔴 高价值抄（缺+设计成熟+低成本）/ 🟡 中期补（有价值但量大或依赖多）/ 🟢 已领先（它规划了但我们更强）/ ⚪ 战略观察（代差，按需）/ ⬛ 不抄。

---

## 0. 先破一个误区：PT Studio 的「架构优越」对 FluxTorrent 边际价值有限

`docs/design/08-lessons-learned.md` 坦白它的架构是为了解前人的坑：

| 前人的坑                                            | PT Studio 对策                 | FluxTorrent 现状                                         |
| ----------------------------------------------- | ---------------------------- | ------------------------------------------------------ |
| NexusPHP 19 CVE（SQL 注入/XSS）                     | sqlx 参数化 + 插件 SQL 角色沙箱 + CSP | ✅ 已 Rust + sqlx 原生规避（**但 native 插件裸 SQL 是自家漏，见 §1.2**） |
| UNIT3D PHP announce 250 次/s/核 → Rust 外挂 5 万/s/核 | announce 一开始就独立部署、水平扩展       | ✅ 已独立 Rust tracker（HTTP+UDP+IPv6），**领先**               |
| 每次 announce 直写 DB                               | Redis Streams 窗口聚合 + 幂等落库    | ✅ 已 Redis + worker 分区聚合，**领先**                         |
| 站点规则硬编码只能 fork 魔改                               | 规则全进插件 + 术语表 + 预设            | ⚠️ FluxTorrent 单体，但已有内容包/模块开关/站型，部分覆盖                  |
| 清理结算挤一个 cron 长锁表                                | 分布式锁 + 分区增量                  | ✅ 已 worker 全表拆分 + tick 分层，**领先**                       |

> **结论**：别为「架构而架构」。PT Studio 的架构红利，FluxTorrent 作为 Rust 原生重写已吃掉大半。真正值得对标的是它**规划中但 FluxTorrent 缺失的治理/运营/互操作能力**——这些才是「成熟后」的真实差距。



---

## 1. 🔴 高价值抄（FluxTorrent 缺、PT Studio 设计成熟、成本低）

### 1.1 crossseed-api 辅种接口（PT 刚需，我们有数据基础）

PT Studio 设计（FR-XSEED-001~009）：`POST /v1/lookup {pieces_sha1[≤500]}` / `info_hash v1/v2`（含**历史 hash**）→ 返回本站种子 + 下载链接；个人 API token（作用域 `crossseed`）；限流 10/min + 5 万指纹/日；最低等级门槛；**IYUU 兼容层**。

- **FluxTorrent 现状**：兼容层/详情已透出 `info_hash` + `pieces_hash`（辅种/查重数据**已在手**），但**没有对外辅种 API，更没有 IYUU 兼容**。
- **借鉴**：🔴 直接抄。数据基础已有，加一个 `crossseed-api` 路由 + IYUU 兼容格式即可。辅种是 PT 用户最高频自动化需求，成本极低、感知极强。

### 1.2 反作弊「运营层」（我们有检测，缺判定+案件+申诉闭环）

PT Studio anticheat（FR-ACH-001~044，全文已读）是一整套**案件管理系统**，核心是我们完全没有的：

- **信号与判定分离**：accounting 只出 6 种基础信号（cross_check_mismatch/speed_exceeded/peer_not_in_swarm/negative_delta/counter_jump/client_not_allowed），判定交给独立规则引擎。
- **评分引擎**：JSON DSL 规则集（版本化 + 影子模式 + **回测模拟**）；风险分 = Σ证据规则 × 乘数（权重×严重度×命中函数 log₂×时间衰减 2^(−天/半衰期)），分段 none/low/watch/high/critical。
- **案件工作流**：自动/手动开案 → 复核队列（认领互斥 + SLA 高亮）→ 处置（警告/冻结流量/锁钱包/禁下载/封禁/回收虚报流量/重置 passkey）→ **期望状态协调器**（幂等落地、外部覆盖检测）→ **四眼原则**（封禁需第二人批准）→ **申诉**（被封用户凭令牌申诉、撤销自动回滚流量回收）→ **误判反馈闭环**（按规则统计准确率，低于阈值告警）→ **熔断器**（1 小时自动处置 >50 暂停，防错误规则大面积误伤）。
- 借鉴 TU Wien 论文：swarm 守恒 + 跨 swarm 聚合。
- **FluxTorrent 现状**：检测层强（BT 三阶段探测、connectable 四档、幽灵做种四条件、ledger_guard、交叉上报），但判定即自动处置，**无案件队列/评分模型/规则版本化/回测/申诉/误判统计/熔断**。
- **借鉴**：🔴 抄「运营层」，保留我们的检测层。最高价值单品：**规则集版本化 + 影子 + 回测**（改反作弊规则不怕误伤）、**申诉四眼 + 撤销回滚**、**熔断器**、**误判准确率闭环**。这正是我们反作弊「敢不敢开自动处置」缺的信任基础设施。

### 1.3 registration-policy 注册治理中心（防滥注册，我们只有 mode 开关）

PT Studio 设计（FR-REGP-001~086）是一个完整注册治理系统：

- 三通道 open/invite/application 各自开关 + **紧急开关 + 限时窗口（名额+标题）+ 周期时段（按周几）**。
- **全站用户上限 + 窗口名额 + 名额占位（TTL 600s 可重入）**。
- 策略组合求值顺序：紧急→通道→可用性→关卡（username_hold→email_domain→captcha→quiz→ip_limit→device_limit→window_slots→user_cap）。
- 关卡：邮箱域名（白/黑名单 + **一次性邮箱库 + 远程同步**）、验证码（**可插拔提供方 + 默认 PoW + 蜜罐 + 填写时间 + 票据**）、答题（题库/抽题/判分/防刷）、**IP/网段/设备（HMAC）限制 + 豁免 CIDR + 摘要化保留**。
- **申请通道（application）**：自定义字段 + 证明链接 + 推荐人 + 截图 → 版主审核队列 → 通过后**无密码建号 + 邮箱验证激活**。
- 预演（不写数据）、公开状态、漏斗统计、MCP。
- **FluxTorrent 现状**：`auth_http/register_mode.rs`（模式开关）+ `invites.rs`（邀请）。**没有**：名额/窗口/时段、申请审核队列、验证码、答题、一次性邮箱库、IP/设备防批量注册。
- **借鉴**：🔴 抄骨架。PT 站被批量注册/羊毛党打是高频事故。优先抄：**一次性邮箱库 + IP/设备限制 + 申请审核通道 + 限时窗口名额**。验证码/答题可后期。

### 1.4 webhook 硬化（我们的是「尽力而为广播」，太裸）

PT Studio 设计（FR-WHOOK-001~011）：**HMAC 签名**（`X-PT-Signature: t,v1=HMAC_SHA256(secret, t+"."+body)`）+ 指数退避重试（1m/5m/30m/2h/6h 共 6 次）+ 连续失败 50 次或 24h 全败自动停用 + 投递日志（保留 14 天）+ 格式转换（discord/slack）+ **防 SSRF**（DNS 解析后拒私有/回环/链路本地/元数据地址）+ 事件白名单 + 谓词过滤 + 测试事件。

- **FluxTorrent 现状**：`ops_webhook.rs` 只做 Discord/TG 双发，失败仅落日志，无签名/重试/停用/SSRF 防护。
- **借鉴**：🔴 抄。尤其 **HMAC 签名 + 防 SSRF + 重试/自动停用**——这是「运营敢把站点事件外发」的前提。工作量小。

### 1.5 mediainfo 一致性校验（防标题党，内容质量刚需）

PT Studio 设计（FR-MINFO-001~011）：解析 MediaInfo 文本/JSON + BDInfo + **中文标签本地化** + **脱敏（去路径中的用户名/UNC 主机名）** + 规格摘要（resolution/codec/hdr/bit_depth/audio）+ **`compare(summary, fields)` 与种子字段一致性比对（返回差异列表）** + NFO 解码 + 异步解析（订阅 torrent.created）+ 上传前预览接口。

- **FluxTorrent 现状**：无专门 mediainfo 服务（有 attachment_video）。
- **借鉴**：🔴 抄 `compare` 一致性比对——**上传时用 MediaInfo 校验标题声明的分辨率/编码/HDR 是否属实，直接打击假 4K/HDR 标题党**。这对内容质量和对用户信任的价值极高，且可挂进现有发布流程。脱敏也重要（MediaInfo 的 Complete name 会泄露站长目录/用户名）。

---

## 2. 🟡 中期补（有价值，但工程量大或依赖其它系统）

### 2.1 ratio-policy 分享率考核（Gazelle 分档表 + 做种减免 + 警告顺延）

PT Studio 设计（FR-RATIO-001~016）：**按下载量分档的要求分享率表（≤20 档，Gazelle 风格）** + **做种减免**（`required = table × (1 − seeding_ratio × reduction)`）+ 豁免（下载量/注册天数/等级/管理员）+ **警告（Gazelle 顺延语义：新警告从现有警告到期时刻顺延）** → 限制下载 → 自动恢复 → 可选进一步封禁 + 警告期禁下新种 + 警告期禁发邀 + **订阅 `traffic.settled` 脏用户 15 分钟增量评估 + 每日全量**。

- **FluxTorrent 现状**：有 `hr.rs`（H\&R）和考核，需核对是否有「分档要求表 + 做种减免」模型。
- **借鉴**：🟡 抄「分档要求表 + 做种减免 + 脏用户增量」模型。做种减免是 Gazelle 类大站护肝核心（鼓励做种而非裸比分享率），我们大概率没有。

### 2.2 newbie-exam 新手考核（多阶段 + 增量基线 + 失败补救）

PT Studio 设计（FR-EXAM-001~017）：考核 = 期限 + **多阶段**（每阶段截止天数 + 目标列表）；指标用**增量**（uploaded/downloaded/seed_secs/uploads_count/balance，分配时记基线）+ ratio 绝对值；**按注册方式/邀请人等级自动分配** + 手动分配；阶段判定 → 失败处置（none/restrict/ban）→ **失败后补救（remedy_days 内补齐自动解除并视为通过）**；豁免/延期/通过奖励（勋章/币种）/临期提醒/统计。

- **FluxTorrent 现状**：有 `task_jobs/exam.rs` + `ops_http/exams.rs` + jixiao。
- **借鉴**：🟡 对照补齐：**多阶段 + 增量指标基线 + 失败补救期** 这三样是 NexusPHP 类站考核的成熟形态。需先核对我们 exam 实现到哪一层。

### 2.3 seedbox 盒子识别（补 connectable 误伤的下一公里）

PT Studio 设计（FR-SBOX-001~011）：用户**申报盒子 CIDR + 审核**；`is_seedbox(user, torrent)` 判定；**自动识别**（数据中心 ASN 抽样 → `seedbox.detected`，仅告警不自动处罚）；**规则标志**（`hnr_exempt`/`bonus_factor`/`anticheat_speed_relax` 供其它插件读取）；冲突检测；隐私（IP 段仅本人+管理者可见）。

- **FluxTorrent 现状**：反作弊有 connectable 四档（`-2 SUSPECT` 加密/CGNAT 不误伤）+ ip_trust，但**没有「显式申报 + 盒子标志 → 反作弊速度放宽 / H\&R 豁免」的联动**。
- **借鉴**：🟡 抄「申报 + 标志联动」——用**显式申报**替代对 SUSPECT 的猜测，盒子用户速度规则放宽、H\&R 豁免、倍率加成。这是把我们的 connectable 判定从「猜」升级为「用户自证 + 系统识别」的关键，直接降低误伤投诉。

### 2.4 terms 术语中台（解我们的 i18n 硬编码债）

PT Studio 设计（FR-TERMS-001~030）：术语键**统一注册/退役** + **多语言回退链**（请求语言→基码→站点默认→en→键名）+ 管理员覆盖（**乐观锁 If-Match**）+ 图标上传（服务端重编码去元数据）+ **快照缓存（Redis by version）** + 渲染模板 `{term:key}` + MCP 查询 + 导入导出 + **每日覆盖率巡检** + 分类展示。

- **FluxTorrent 现状**：有 `site_settings.currency_name`（魔力）+ `sitetype.rs` 术语改名，但 i18n 硬编码是老大难（今日门禁 `validation_i18n_guard` 红：动态串 196、`i18n_guard` 反复漂移）。
- **借鉴**：🟡 借「术语键统一注册 + 快照 version 缓存 + 覆盖率巡检」机制，治理我们的 i18n 债。不是功能，是机制。

### 2.5 shop 三段式履约（冻结→履约→扣款）

PT Studio 设计（FR-SHOP-001~022）：商品类型 9 种（upload_credit/invite_quota/rename/custom_title/medal/vip/makeup_card/promo_card/**hnr_removal**）；**冻结金额 → 履约 → 扣款**，履约失败解冻；**幂等 + 崩溃恢复（中间态 >2min 由恢复任务推进）**；库存/限购/**递增价格**（price_step/price_max）；**强制经商城改名**（shop 处理 user.before_rename 拒绝非商城自助改名）；退款（先撤销履约再冲正）；赠送。

- **FluxTorrent 现状**：有 shop。
- **借鉴**：🟡 抄「三段式履约 + 崩溃恢复 + 递增价格」。三段式是虚拟商品交易不丢单/不超卖的关键。

### 2.6 lottery 可验证公平（commit-reveal + drand 信标）

PT Studio 设计（FR-LOT-001~023）：**开售生成 32 字节 seed 存密文并公布 `commit = SHA-256(seed)`**；停售算 `tickets_root`（票据哈希链）；**开奖 `rand = HMAC-SHA256(seed, round_id‖tickets_root‖beacon)`**；公开 seed + **验证页（commit/seed/tickets_root/beacon + 算法说明 + 票据清单下载）**；可选 **drand 信标**（停售后第一轮随机值）；四眼结算；奖池滚存；最少票数取消退款；竞猜市场。

- **FluxTorrent 现状**：有 `lottery.rs` + `gacha_http`（casino/pool/fishing 等）。
- **借鉴**：🟡 抄 **commit-reveal 可验证公平 + 验证页**。赌坊类功能，「公平可验证」是用户信任的唯一基石——我们现在大概率是服务端随机，用户无法验证。 drand 信标可后期。

### 2.7 medals 成就系统（事件计数 + 统计阈值 + 回填）

PT Studio 设计（FR-MEDAL-001~022）：**双模式成就**——事件计数（监听事件累加字段达阈值授予）+ 统计阈值（周期性只读视图评估上传/做种/注册天数达阈值授予）；**成就回填**（新建成就可全量评估 + 事件计数类从历史事件回放）；成就进度（当前值/目标）；佩戴（≤3）+ 头衔激活；限量 + 到期失效 + 自动收回（可选）。

- **FluxTorrent 现状**：有 medals。
- **借鉴**：🟡 抄「双模式成就 + 回填」。我们的勋章大概率是手动/简单授予，缺「自动成就 + 历史回填 + 进度展示」。

---

## 3. 🟢 已领先（PT Studio 规划了，但 FluxTorrent 已实现且更强）

| 能力         | PT Studio 规划                                      | FluxTorrent 现状                                                                         |
| ---------- | ------------------------------------------------- | -------------------------------------------------------------------------------------- |
| 字幕         | subtitles 插件（上传/格式嗅探/审核/奖励）                       | ✅ `content_http/subtitles_*.rs` + `worker/jobs/subtitle_*`：**完整任务流 + 认证 + 奖励，比它设计得更深** |
| 论坛         | forum 插件（分区/权限/投票/合并拆分/未读/关联种子讨论帖）                | ✅ `community_http/{forums,forum_perm,topic*,post_*}` 已实现论坛；投票/合并拆分/未读可作增量              |
| 反作弊**检测层** | accounting 只出 6 种基础信号                             | ✅ BT 三阶段握手探测 + connectable 四档 + 幽灵做种四条件 + 交叉上报，**协议级深度它没有**                            |
| Tracker    | pt-tracker 刚自研、只 HTTP、未压测（目标 1.7 万/s）             | ✅ HTTP+UDP+IPv6，实测 announce p50 4ms/100 并发 104ms                                       |
| 元数据多源      | metadata-bangumi/douban/imdb/igdb/musicbrainz 插件化 | ✅ wasmtime 适配器（douban 已内置），形态更灵活；可补 IYUU 式多源但不紧迫                                       |
| 通用站无码定制    | type-* 资源类型插件                                     | ✅ 11 站型 + 用户字段 + 页面菜单 + 内容包主题 + 30 模块开关，**无码定制面更广**                                    |

---

## 4. ⚪ 战略观察（代差，按需，不急）

- **LLM / MCP**：PT Studio 已落地 llm-claude + ai-assistant + ai-moderation + ai-upload-review + mcp 网关（MCP 是「给 AI 工具调站点」的协议）。FluxTorrent 缺。但——这是**可选战略方向**，不是 PT 刚需。若要「AI 原生」，最低成本路径是在**现有 wasmtime 适配器层**加 LLM 接入（而非照搬它的 ctx.llm/MCP 内核），对标其终局即可，不必追平。
- **search-meilisearch 语义/混合搜索**：Meilisearch 已是它的实现插件，规划语义/混合/多实例。FluxTorrent Postgres 够用则不必追；若种子量上到百万级再评估。

---

## 5. ⬛ 不抄（不适配 FluxTorrent 单体/设计还不如我们）

- **全微内核 + 每插件独立 schema/role/Redis 前缀**：运维重、正确性未生产验证。只抄**插件 SQL 隔离**这一层（见 §6）。
- **tracker 只用 HTTP（D15）**：我们有 UDP，不倒退。
- **申请通道无密码建号**：与现有注册体系耦合，评估后再说。

---

## 6. 一条贯穿所有借鉴的主线：先补「插件 SQL 隔离」

PT Studio 规划里的 anticheat/ratio-policy/registration-policy/shop 全是「持数据库能力的第三方逻辑」。它之所以敢让插件跑，是因为有 **per-plugin role + `SET ROLE` + search_path 锁定 + 敏感能力二次授权**（`storage/migrate.rs:166`）。FluxTorrent 的 native 插件现在是 `&AppState` 裸 SQL（`plugins.rs:235`）——**在上面任何一条借鉴落地前，必须先把这个洞堵上**，否则「反作弊运营层/注册治理/商城」这些高权限插件一上，就是 NexusPHP CVE 重演。

> 优先级：**插件 SQL 隔离（前置必需）→ crossseed-api / webhook 硬化 / mediainfo 一致性（低成本高感知）→ 反作弊运营层 / registration-policy（高价值大工程）→ ratio-policy / seedbox / lottery 公平 / shop 三段式 / medals 成就 / terms 中台（中期）**。
