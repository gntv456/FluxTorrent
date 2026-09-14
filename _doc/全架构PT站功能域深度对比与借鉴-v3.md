# 主流 PT 架构全功能域深度对比与借鉴（v3）

> **定位**：本文是 `主流PT架构横向对比与借鉴.md` v2（2026-09-13）的**功能级深化版**。v2 回答"架构层面谁先进谁落后"；v3 回答"**每个功能域上，各家具体怎么做的、公式和参数是什么、我们差在哪、抄什么、避什么**"。
>
> **证据基础**（全部 2026-09-13 快照，比 v2 大幅升级）：
> - **NexusPHP**：xiaomlove/nexusphp `php8` 分支 v1.10.2 **本地克隆源码逐文件核实**（本地 `.np-research/np/`）
> - **Gazelle**：OPSnet/Gazelle master（commit 58e3b9e）**源码级核实**（含 app/、misc/ 迁移、sphinx.conf、ocelot.conf）
> - **UNIT3D**：HDInnovations/UNIT3D master（v9.2.0，Laravel 12 + Livewire 3）**源码级核实**
> - **TBSource**：QwertyRider/TBSource 镜像源码核实（原组织仓库已 404）
> - **XBTIT / xbtt**：Q8HMA-zz/xbtit + gubatron/xbt-tracker 镜像源码核实
> - **M-Team**：harrisoff/mteam-api（从 M-Team 官方 OpenAPI 3.0.1 文档机器生成的完整镜像）
> - **YemaPT**：wiki.yemapt.org/developer/open-api 官方文档逐端点抄录
> - **TNode/朱雀**：ptool site/tnode 源码 + PT-Plugin-Plus TNode schema
> - **工具生态**：sagan/ptool（site/*.go 全读）、PT-Plugin-Plus（148 站配置逐一核对）、PT-depiler、IYUU 官方 doc、cross-seed 源码
> - **我方**：本仓库代码实测盘点——69 迁移/116 表/299 唯一路由/68 权限点/14 worker 任务/54 页面
>
> 证据分级：`[实证]` 源码或官方文档原文；`[业内]` 多源交叉印证；`[推断]` 合理推演已标注。各节末尾的 **「借鉴 / 避坑」** 是本文的结论层，可单独阅读。

---

## 0. 十大架构功能密度总览

| # | 架构 | 语言/框架 | 内容模型 | 积分体系 | 社区组件 | 后台规模 | 工具生态位 | 一句话画像 |
|---|---|---|---|---|---|---|---|---|
| 1 | **NexusPHP** | PHP+Laravel 双轨 | 扁平+求种/认领/考核 | 魔力值（arctan 封顶公式） | 论坛/水楼/趣室/字幕 | Filament 4 组+传统 staff 工具 | **事实标准** | 华语 PT 的 Windows |
| 2 | **UNIT3D** | Laravel 12+Livewire 3 | 扁平+TMDB 元数据 | BON（DB 可配规则引擎 v2） | 聊天盒+53 成就+54 通知 | 62 个 staff 目录 | 部分（DOM） | 欧美新站默认选择 |
| 3 | **Gazelle** | PHP 8.4+Twig | **Group/Torrent 两级** | BP（稀有度×时长对数） | Collage+Wiki+通知矩阵 | 中（Filament 无） | 部分（ajax.php） | 音乐垂直站天花板 |
| 4 | **TBSource** | 裸 PHP 扁平 | 扁平 22 表 | 无（纯分享率） | 论坛/PM/投票 | ~5 个 mod 文件 | 无 | 2005 祖源，已灭绝 |
| 5 | **XBTIT** | PHP+bTemplate | 扁平 | 有（简版） | shoutbox/投票/外部论坛 | 25 个 admin 模块 | 无 | hack 生态反噬典型 |
| 6 | **Discuz-PT** | Discuz 插件 | 论坛帖+附件 | 论坛金数（冲突） | 论坛全套 | Discuz 后台 | 微量 | 教育网遗存，本次未核实到源码 |
| 7 | **JS 系** | Node/Mongo | 扁平 | 魔力点 | 聊天室 | 简版 | 无 | 集体失败（meanTorrent 2020 死） |
| 8 | **TNode/朱雀** | 未知（前后端分离） | 类目树+多维属性 | 有 | 未知 | 未知 | **ptool 平级 adapter** | 自研进生态的标杆 |
| 9 | **YemaPT** | SPA+AntD | 类目树 | 有 | 未知 | 未知 | PT-depiler 独立 schema | API 文档化标杆 |
| 10 | **M-Team** | ThinkPHP→新版+Vue | 扁平+多维字典 | 魔力值 | 信箱分盒/菠菜/联盟 | 封闭 | **ptool 专属 mtorrent type** | API-first 大站标杆 |
| （参） | **我们 FluxTorrent** | Rust Actix+Next.js 15 | 扁平+教育域+聚合组 | 火花（多源+银行+游戏） | 论坛/趣事/五子棋/农场 | 122 端点三模块 | compat 层已建，**未进工具圈** | 三代计费架构+教育垂直域 |

---

# 第一部分 · 计费与 Tracker 架构（分水岭）

## 1. announce → 计费：四代范式（v2 三代说的修正）

v2 把历史分成三代。拿到 XBTIT/UNIT3D-Announce/Torrust 源码细节后，更精确的是四代：

| 代 | 代表 | 热路径行为 | 计费落库方式 | 实测/官方容量 |
|---|---|---|---|---|
| **一代·同步写库** | TBSource、NP、Discuz-PT | announce 内联 UPDATE users/peers/torrents | 每请求 4-6 条同步 SQL | XBTIT 官方：PHP tracker ≤5-10k peers `[实证]` |
| **二代·内存累计+批量 flush** | xbtt（XBTIT）、Ocelot（Gazelle） | 全内存 peer 表 | 缓冲字符串攒增量，定时 `INSERT..ON DUPLICATE KEY UPDATE` 多行合并 | Ocelot：announce_interval 900s/jitter 30s/reap 300s `[实证]` |
| **二代半·外挂 tracker+队列双轨** | UNIT3D v9 | PHP 或 Rust announce 二选一 | PHP 模式：Redis RPUSH 三队列，每 5s 批量落库；Rust 模式：内存六队列每 **3000ms** UPSERT 直写 MySQL `[实证]` | Rust 版 ~50k req/s/核，PHP ~250 req/s/核 `[实证]` |
| **三代·持久流+幂等消费者** | **我们** | tracker 零 DB，仅 XADD Redis Stream | worker 幂等消费→分区流水表，失败可重放死信 | 未压测（单机 DashMap，理论数万/s） |

**四代演进的内在逻辑**：每一步都是把"计费写库"从 announce 请求路径上挪得更远。TBSource 死于同步 UPDATE + `ORDER BY RAND()` peer 抽样 + fsockopen 5 秒连通探测 + MyISAM 表锁四连 `[实证]`；xbtt 用 `m_users_updates_buffer` 字符串攒 SQL 是 2006 年的智慧；UNIT3D 2026 年的答案仍是"内存队列+定时批量"，只是把间隔从分钟级压到 3 秒；我们把事实源彻底改成流水表（ledger），聚合值全部可重算——**这是唯一能做到"账目可审计、宕机不丢量"的一代**。

> **避坑**：UNIT3D Rust 模式的 3s UPSERT 直写意味着 tracker 与 DB 仍有强耦合（MySQL 挂=统计停）；我们的 tracker 已零 DB，**守住这条边界**——未来任何"tracker 直接查一下用户表"的需求都必须拒绝，走 Redis 缓存或 Stream 事件。

### 1.1 Tracker 参数对照

| 参数 | Ocelot `[实证]` | UNIT3D-Announce `[实证]` | NP | **我们** |
|---|---|---|---|---|
| interval | 900s 固定 + 30s jitter | 1800-3600s 随机区间 | 1800s 基准，老种分级（annintertwo/three） | 按 site_settings 下发（默认 1800s，clamp 60-86400） |
| peer 超时 | 120s + reap 300s | ACTIVE_PEER_TTL 7200s / INACTIVE 21 天 | 无种 last_action 超时 | **90s（内存）**/ DB 侧 2×interval 兜底 |
| numwant 上限 | 10 | 15 | 50 | 50（clamp 1-200） |
| IPv6 | ❌ 仅 v4（README 原文） | ✅ | ❌ | ✅ BEP-7（v4/v6 分列） |
| 客户端管控 | xbt_client_whitelist 白名单 | BlacklistClient peer_id 前缀黑名单 + 端口黑名单 | agent_allowed_family 三层匹配+浏览器 UA 拒绝 | agent_rules 黑白名单（60s 缓存）+ agent_block 事件流 |
| 防重放/限流 | — | rate_limit 3/min + ANNOUNCE_MIN_ENFORCED 1740s | MIN_ANNOUNCE_WAIT 300s | user 1800/min + IP 限流 + Redis fallback 计数 |

**我们独有的两个优势**：① interval 可运营配置（Ocelot/U3D 都要改配置重启）；② Redis 挂掉时降级为本地累计差值口径不丢数据。**两个待办**：numwant 上限 50 与 NP 相同但低于 Ocelot 的严格性无所谓；**peer 90s 超时 vs 1800s interval 的口径矛盾**仍在（已列入遗留），UNIT3D 的三档 TTL（活跃 2h/不活跃 21 天）是可抄的解法——按"最近是否活跃"分档而不是一刀切 90s。

> **借鉴**（P2）：peer 超时分档——活跃 peer TTL 放宽到 ≥2×interval，不活跃 peer 90s 淘汰。当前口径下挂种多的大户每 90s 全量重建内存表，是 tracker 最大的无谓开销。

---

# 第二部分 · 用户体系与等级

## 2. 等级体系全景

### 2.1 NexusPHP：四条件公式 + 降级线 + Peasant 惩罚（最完整的规则书）

17 级体系（Peasant 0 → Staff Leader 16）。**晋升四条件同时满足** `[实证]`：

```
downloaded ≥ 下限GB AND seed_points ≥ 阈值 AND uploaded/downloaded ≥ 最低PR AND 注册时长 ≥ N 周
```

做种积分阈值硬编码：PU=40000 / Elite=80000 / Crazy=15万 / Insane=25万 / Veteran=40万 / Extreme=60万 / Ultimate=80万 / NexusMaster=100万。默认参数表（settings.default.php）：

| 等级 | 下载下限 | 最低PR | 降级线 | 周数 | 送邀请 |
|---|---|---|---|---|---|
| Power User | 50GB | 1.05 | 0.95 | 4 | 1 |
| Elite | 120GB | 1.55 | 1.45 | 8 | 0 |
| Crazy | 300GB | 2.05 | 1.95 | 15 | 2 |
| Insane | 500GB | 2.55 | 2.45 | 25 | 0 |
| Veteran | 750GB | 3.05 | 2.95 | 40 | 3 |
| Extreme | 1024GB | 3.55 | 3.45 | 60 | 0 |
| Ultimate | 1536GB | 4.05 | 3.95 | 80 | 5 |
| Nexus Master | 3072GB | 4.55 | 4.45 | 100 | 10 |

- **降级**：PR < 降级线即降一级，可级联；**Peasant 化**阶梯（50GB/0.4 → 800GB/0.8 五档），限期 30 天不改善→封禁（VIP/捐赠者豁免）；被封者可花 10 万魔力**自助解封**（BUSINESS_TYPE_SELF_ENABLE）。
- **账号清理**：VIP 永不删；未停车无流量 60 天/正常 150 天/停车 400 天未访问→禁用。
- **辅助限制**（默认关）：等待时间制（ratio<0.4 新种等 24h）+ 下载槽位制（ratio<0.5 单种）。

### 2.2 Gazelle：做种行为反噬 RequiredRatio（最精巧的反投机设计）

- 主类 8 级（User→Ultimate TM），晋升条件含 **unique groups 数**和 **perfect FLAC 数**（音乐域指标）。
- **RequiredRatio 动态浮动**：分档上限（>100GB→0.60 … <5GB→0），再按 `fraction = 1 - SeedingAvg/NumSnatches` 折减——**抢种（snatch 后不做种）会抬高你的要求，坚持做种会降低要求** `[实证]`。这是所有架构中唯一"要求随行为浮动"的设计。
- 副类徽章制（Donor/FLS/Interviewer/Recruiter/VIP），RANKING_WEIGHT 十维加权百分位排名（uploaded 10/downloaded 25/uploads 25/requests 15/posts 6/bounty 4/...collage-create 8）。

### 2.3 UNIT3D：DB 可配的组门槛（最工程化）

- 21 个内置组，组表直接带 6 个门槛字段（min_uploaded/min_seedsize/**min_avg_seedtime**/min_ratio/min_age/min_uploads）+ 约 30 个布尔权限位（is_immune/is_freeleech/is_incognito/download_slots…）。
- 默认门槛以**上传量为主轴**（PowerUser 1TB → Veteran 100TB/365 天），Seeder/Archivist 组走"做种体积+做种时长"路线——两种晋升价值观并存。
- Leech 组（ratio<0.4）为降级兜底：入组即 `can_download=0`。
- **2FA 与邀请挂钩**：启用 2FA 后 24h 才能发邀请（防盗号发邀请）`[实证]`。

### 2.4 我们 vs 三家

| 能力 | NP | Gazelle | UNIT3D | **我们** |
|---|---|---|---|---|
| 等级数 | 17 | 8 主+7 副 | 21 组 | user_classes + class_rules 表驱动，数量运营可配 |
| 晋升条件 | 四条件公式（硬编码阈值） | 上传+做种+unique+perfect | 6 门槛字段全 DB | **class_auto_adjust worker**（60s tick） |
| 降级 | PR 降级线可级联 | 可自动降级 | Leech 组兜底 | 随 class_rules 同一机制 |
| 惩罚阶梯 | Peasant 30 天限期封禁 | RatioWatch 剥夺做种/下载 | 警告 3 次禁下载 | hr_violations + 封禁体系 |
| 自助解封 | 10 万魔力 | — | — | ❌ 无 |
| 行为浮动要求 | ❌ | ✅ RequiredRatio 动态 | ❌ | ❌ |
| 等级特权感知 | 强（等待/槽位/邀请数） | 强（badge/配额） | 强（组样式/槽位） | 弱（权限为主，缺"等级待遇"体感） |

> **借鉴**（P1）：① **降级线**——我们有自动升级却缺"PR 跌破即降级"的对称机制，教育站刷分容易、保分意识弱，降级线是低成本的行为约束；② **自助解封**（火花付费解封）——被封用户是负资产，付费+冷静期解封比永久流失好，且与我们经济系统天然耦合；③ **等级待遇体感**——NP 的"晋升送邀请数"表、U3D 的组彩色样式/icon，让等级"看得见"，我们的等级目前只体现在权限位，建议补"晋升送邀请/晋升送火花/等级专属商店档位"。
> **避坑**：NP 把做种积分阈值**硬编码**在 User.php 里（改参数要发版）——我们的 class_rules 表驱动是对的，坚持。

## 3. 封禁与账号治理

| 机制 | NP | UNIT3D | **我们** |
|---|---|---|---|
| 封禁类型 | disable/ban 双态+leechwarn | Banned 组+ban/unban 记录表 | status 字段+封禁理由落 modify_log（本轮修复） |
| 闲置清理 | 60/150/400 天三档+VIP 豁免 | 90 天无登录无做种→Disabled→120 天软删 | ❌ **无自动闲置清理** |
| 登录失败 | maxloginattempts=10 | FailedLoginAttempt 表+邮件通知 | **限流 5 次/分钟双维度**（本轮修复）+ login_events 90 天清理 |
| 会话管理 | — | getSessionList+revokeSession（M-Team 也有） | ❌ 无会话列表/主动吊销 |

> **借鉴**（P1）：① **闲置账号自动清理**——U3D 的"90 天未登录且无做种→Disabled"三段式（警告→禁用→软删）是教育站防僵尸号刚需，我们 invites 配额有限更需回收；② **会话管理页**——"查看在线会话+一键下线"是盗号自查刚需（M-Team/App 都有），我们 JWT 无状态实现成本稍高，可用 jti 黑名单或记录设备指纹折中。

---

# 第三部分 · 经济系统（最大的功能密度差距所在）

## 4. 积分获取公式三家对照

### 4.1 NexusPHP 魔力公式（每小时结算）`[实证]`

```
A = Σ_每个做种种子 (1 - e^(ln(0.1)/4 × 存活周数)) × 大小GB × (1 + √2 × e^(ln(0.1)/6 × (seeders-1)))
每小时魔力 = 100 × (2/π) × atan(A / 300) + 1 × min(做种数, 7)
```

设计意图逐项拆解：**种子越老衰减越快**（4 周衰减到 10%——奖励新鲜保种）、**做种人数越多衰减越快**（7 人时约 10%——奖励稀有保种）、**arctan 封顶**（趋近 100/小时，防止堆量）、**做种数线性补贴**（最多 7 个）。加成通道：捐赠者×2、官方标签×0.5、勋章系数、**后宫加成**（被邀请人做种产出给邀请人分成）。

### 4.2 Gazelle BP 公式（每小时计提，MySQL 函数）`[实证]`

```
每小时每种子 = (size/分类scale)/1024³ × (0.0433 + 0.07 × ln(1 + seedtime_h/24) / max(seeders,1)^0.35)
```

同样三个变量（大小/时长/稀有度），但结构不同：时长走**对数**（长期做种边际递减）、稀有度走 **-0.35 次幂**（比 NP 的指数衰减温和得多）。11 条规则化奖励另算（UNIT3D 同款思路见下）。

### 4.3 UNIT3D BON 规则引擎 v2（完全 DB 可配）`[实证]`

`bon_earnings` 表：position/variable/multiplier/operation/condition 全是行数据，变量集 `1, age, size, seeders, leechers, times_completed, personal_release, internal, seedtime, connectable`。默认 11 条规则：

| 规则 | 条件 | 时薪 |
|---|---|---|
| Dying Torrent | 自己是最后做种者且完成≥3 | +2.0 |
| Legendary Torrent | 种龄>12月 | +1.5 |
| Old Torrent | 6-12月 | +1.0 |
| Huge/Large/Everyday | ≥100GiB / 25-100 / 1-25 | +0.75/0.5/0.25 |
| Legendary Seeder | 个人做种该种≥1年 | +2.0 |
| MVP/Committed/Team Player/Participant | 6-12/3-6/2-3/1-2月 | +1/0.75/0.5/0.25 |

即单种子最高 ~4.5 BON/h，**规则化描述（"濒危种""传奇种"）让玩家能看懂并规划**。

### 4.4 我们

seeding_reward worker 每小时批量 10 结算，基础+加成结构，参数在 site_settings。**缺三个要素**：① 稀有度衰减（人多不减收益）；② 时间饱和（长期挂量收益不封顶）；③ 规则的"可读性"（用户看不到自己为什么拿这么多/怎么拿更多）。

> **借鉴**（P1，经济系统最重要的一项）：**做种收益引入稀有度+时长饱和**。不需要照抄公式，取两家长处：NP 的"人数指数衰减"（防大户垄断热门种收益）+ U3D 的**规则命名**（"濒危保种 +2/小时"比公式可读性强百倍）。落地形态：seeding_reward 公式加 seeders 衰减因子 + 前端"我的收益明细"页按规则名展示。同步要做的还有 NP 的**后宫加成**（邀请人对被邀请人做种收益分成 5-10%）——教育站邀请关系链是核心，分成制让老带新有持续动力，比一次性邀请奖励强。
> **避坑**：三家都把"做种收益"做成了**每个种子独立计算再求和**的结构，我们若是"每用户一次性算"要检查公平性；公式参数三家都进了配置（NP 例外是硬编码，那是反面）。

## 5. 积分消费与商店

### 5.1 商品价目对照（默认值）

| 商品 | NP 魔力价 | UNIT3D BON 价 | Gazelle BP | **我们火花价** |
|---|---|---|---|---|
| 1GB 上传量 | 300 | 2GiB=500 | — | shop_items 表（运营定价） |
| 10GB 上传量 | 1300 | 10GiB=1000 | — | 同上 |
| 100GB 上传量 | 10000 | 100GiB=5000 | — | 同上 |
| 删下载量 10GB | 1000 | —（Refundable 替代） | — | ❌ |
| 1 邀请 | 1000 | 2500 | — | 有（invite_quota 体系） |
| VIP 1 月 | 8000 | — | — | ❌ |
| 自定义头衔 | 5000 | — | 有（BonusItem） | ❌ |
| 改名卡 | 100000 | — | — | 有（username_change_logs，admin 侧） |
| 补签卡 | 1000 | — | — | ✅ 有（30 天窗口，resub_uses） |
| H&R 免罪 | 10000/条 | ❌（仅自动+手动） | — | ❌（仅 admin 批量赦免） |
| 彩虹 ID | 5000 | — | — | dressup 体系有框架 |
| 免广告 | 10000/15天 | — | — | ❌ |
| 自助解封 | 100000 | — | — | ❌ |
| FL 券 | —（促销制） | Personal FL 24h=7500 | token-* 商品（买给他人价格递增） | ❌ |
| 个人 Collage | — | — | 槽位价 ×2^已有数 递增 | — |
| 上传量购买限制 | ratio≥6 且 ≥50GB 下载 | max-buffer 上限 | — | — |

**三家共有的设计信号**：①"删下载量"是最硬通货（NP 定价最贵档）；② **H&R 免罪券**让处罚可赎回（NP 有/U3D 无）；③ Gazelle 的"**买给他人价格递增**"（BONUS_OTHER_TOKEN_INTERVAL=100）是防小号互刷的巧设计。

### 5.2 Freeleech 凭证形态对照

| 形态 | Gazelle | UNIT3D | NP | **我们** |
|---|---|---|---|---|
| 单种 FL 券 | FL token（下载超 4% 或 30 天过期，可叠加） | fl_tokens（复活奖励/抽奖） | — | ❌ |
| 全下场 Personal FL | — | 24h（BON 购买） | — | ❌ |
| 促销制 | FreeTorrent 字段 | free 0-100% 百分比 + doubleup + featured | sp_state 7 种 | promotions 六档（p30/half/free/x2/x2half/x2free） |

> **借鉴**（P1）：**FL 券/中性券上商店**。Gazelle 的 FL token 是公认最成功的精细促销工具（比全站 free 省成本、比单种促销更普惠）；我们的 shop_items+promotions 基建都在，缺的只是"券"这种商品形态（买→库存→用→对该种下载免费→tracker 侧生效）。实现要点抄 Gazelle：**下载超种子大小 4% 即核销**（防囤券刷量）、**生效要同步 tracker**（Ocelot 是 addToken/removeToken 调用，我们走 agent 同款的事件流或直接改 announce 校验逻辑读 Redis）。中性券（上下行都不计）可同期做——Gazelle 的 Neutral Leech 是"冲抵下载又不发上传"的控通胀工具。
> **避坑**：U3D 无 BON 赎回 H&R（只能等警告到期或手动）——用户怨气无处发泄；NP 的 1 万魔力/条定价说明**免罪券要贵到肉疼**才有威慑，我们若上免罪券定价应 ≥10 万火花级。

## 6. 银行/贷款/通胀治理（我们独有，但要对照控通胀思路）

我们独有的银行体系（定存 1-18% 年化/活期/贷款五档/逾期罚息/违约状态机）和站免池（200 万火花/月达标触发次月全站双免三天）。对照组：

- **AnimeBytes Freeleech Pool**：用户捐 Yen 凑目标→触发全站 FL，**个人贡献占比公开显示** `[实证]`——与我们 magic_pool 同构，但他们把"谁捐的"做成了荣誉展示（捐赠排行榜），我们的 donation_ledger 有数据但**没做荣誉层**。
- **HDBits Featured**：用户捐赠使**指定种子**成为 FL（众筹定向免费）`[实证]`——比全站池更精准，适合"全班都要下同一门课的课件"这种教育场景。
- **Gazelle 奖池税**：竞赛奖池捐入按等级抽税（Std 0.9/Elite 0.8/TM 0.7/Staff 0.5）——通胀从源头抽水。
- **NP 税制**：魔力赠送 4%+10% 双税、付费种交易税 0.3——流转税控通胀。

> **借鉴**（P2）：① 站免池补**荣誉层**（贡献榜+达成庆祝公告），参照 AB 的池页；② 可试点**定向众筹免费**（HDBits 模式）——某个教材种社区凑火花免费，与教育场景天然契合；③ 我们的银行利息是纯增发，建议参照 Gazelle 奖池税思路评估"利息收入税"或把利息与站免池打通（利息的一部分强制入池），**月度产出-回收对账报表**仍是要做的（审计报告遗留项）。

## 7. 促销类型学

| 架构 | 类型全集 | 特色 |
|---|---|---|
| NP | Normal/Free/2X/2XFree/50%/2X50%/30% 共 7 种 | **发布随机促销**（掷骰：2XFree 2%/2X 2%/Free 2%/50% 5%）；大种自动促销（>20GB）；过期回落（Free 60 天…）；**无 NE**（M-Team 魔改）`[实证]` |
| UNIT3D | free 0-100% 五档百分比 + doubleup + refundable + featured + 全站开关 | **自动促销规则表**（name_regex/size/分类→free 百分比，审核通过时匹配）；Refundable 按做种时长**线性退还下载量** `[实证]` |
| Gazelle | Normal/Freeleech/Neutral + LeechReason（StaffPick/永久FL/展示/AotM） | **新种自动 6h 免费**（注释自嘲实际 7h）`[实证]` |
| **我们** | p30/half/free/x2/x2half/x2free 六档 | 取最强档（本轮统一口径）；sticky_promotions 置顶促销；magic_pool 全局促销 |

> **借鉴**（P2）：① **U3D 的自动促销规则表**——"正则匹配名称+大小+分类→自动促销"非常适合教育站（如"课件类自动 50%"），审核通过即触发，减少人工；② **Refundable（按做种时长退下载量）**——H&R 的温柔变体，"边做种边退款"对教育网弱上传用户极友好，可与 H&R 豁免条件打通；③ NP 的**发布随机促销**是游戏化彩蛋（用户发种有惊喜），成本低趣味强。

---

# 第四部分 · 种子生命周期

## 8. 发布与审核

| 环节 | NP | UNIT3D | Gazelle | **我们** |
|---|---|---|---|---|
| 审核状态 | 0/1/2 三态 | PENDING/APPROVED/REJECTED/**POSTPONED** 四态 | 无审核（社区自治） | approval_status 三态 |
| 拒绝治理 | 拒绝原因表+**被拒 2 次禁发** | Trump 举报低质种 | reportsv2+ReportAuto | torrent_deny_reasons（有） |
| 免审通道 | **5 个免审通过发布者**可跳审核 | is_trusted 组 | — | ❌ 无 |
| 自动促销 | 大种自动 | 规则表（见 §7） | 新种 6h 免费 | ❌ 无 |
| 重复检测 | — | — | — | pieces_hash 查重（0069 已落地） |

> **借鉴**（P1）：① **POSTPONED 第四态**（U3D）——"证据不足暂缓"比二选一更贴近真实审核流；② **免审积分制**（NP）——"连续 N 次通过免审"是发布者成长路径，教育站师资发布者尤其需要；③ **被拒 N 次禁发**（NP）——恶意发种的硬闸，我们只有 deny_reasons 记录缺自动封禁。

## 9. 聚合与组织

- **Gazelle Group/Torrent 两级**：TGroup（名称/年份/艺人/标签）+ Torrent（媒体/格式/编码/Remaster 四件套），聚合页共享简介封面评论，分组视图排序用聚合表达式 `[实证]`。
- **M-Team 多维字典**：category/standard/team/source/medium/videoCodec/audioCodec 七套字典端点 `[实证]`。
- **UNIT3D**：扁平 + TMDB/IMDb/MAL/IGDB 元数据自动抓取（ProcessMovie/TV/IgdbGame Job）+ Playlist（含 zip 打包）。
- **我们**：torrent_groups（0069 基础落地：UNIQUE name + group_id + 详情页同组版本）+ 教育域（textbooks/editions/grades/科目/版本）。

**功能域结论**：Gazelle 的两级模型是 15 年验证的最优解，我们的 torrent_groups 方向正确，但差距在**关联的丰富度**——GZ 聚合页有"该组全部版本的格式矩阵视图"、按 group 搜索、group 级订阅/收藏。我们详情页有「同组版本」Fold 区块，还缺：上传时**自动建议入组**（按 pieces_hash/名称近似）、组级评论与简介覆盖、组级收藏订阅。

> **借鉴**（P1）：聚合组补完三件——上传时自动推荐已有组（名称 trgm 相似+pieces_hash 命中直接锁定）、组级订阅（新版本入组通知，教材改版场景刚需）、组级聚合统计（组内总做种/总下载）。
> **避坑**：GZ 的 group 是**音乐天然一题多版**；教育域不全是（一个课件就一个版本），**不要强制所有分类走组**——按分类开关（课本/课程类开组，其他类扁平），否则上传流程被拖累。

## 10. H&R 全景对照（规则密度最高的域）

| 维度 | NP `[实证]` | UNIT3D `[实证]` | **我们** |
|---|---|---|---|
| 模式 | disabled/manual/global 按分区 | 全局 hitrun.php 参数 | hr_snapshots + hr_enforce worker |
| 考察目标 | 做种时长 OR 分享比 OR 无种下载时长 | seedtime 7 天 | 做种时长 |
| 豁免 | VIP/捐赠者/等级≥VIP 直免 | donor>组 immune>history immune 三层 | hr.exempt 权限（user_can） |
| 预警 | — | prewarn 1 天（PM 预警） | ❌ 无预警 |
| 处罚递进 | 未达标数达限→封禁 | 警告 14 天有效，**3 次活跃→禁下载** | hr_violations 计数 |
| 宽限 | — | grace 3 天断种宽限 + buffer 10%（下载超 10% 才计 H&R） | — |
| 赎回 | 魔力 1 万/条 | ❌ | ❌ |
| 申诉 | complains 独立系统 | Ticket 工单 | appeals（有） |
| 教育网适配 | — | — | **按 user_id 而非 IP 限流**（校园 NAT 友好） |

**U3D 的 buffer 10% 细节值得专门记录**：实际下载量 ≤ 种子大小×10% 完全不计 H&R——点错下载/秒删的场景不误伤 `[实证]`。

> **借鉴**（P1）：① **预警通知**（prewarn PM）——处罚前 24h 提醒，教育站新人多，误伤率会大降；② **buffer 豁免线**（<10% 不计）——一行逻辑免掉大量申诉；③ **免罪券**（见 §5）。

## 11. 求种（Request）与悬赏

- **Gazelle**：bounty **直接扣上传量**（事务内 `Uploaded -= amount`，余额不足回滚），REQUEST_TAX 可配（默认 0），`request_vote_summary` 物化汇总表 + MERGE 维护，fill 后 filler 全额拿、全部投票者收 PM `[实证]`。**无自动撮合引擎**，靠 NotificationFilter（用户按艺人/厂牌订阅新种推送）半自动。
- **UNIT3D**：BON 押注多人加注/认领 7 天回收/**批准即自动转账**/驳回回滚追回，成就挂钩（Filled 25/50/75/100）`[实证]`。
- **NP**：Offers（预发布投票制，10 票通过才准发）+ Requests（应种 chosen 标记，赏金流转弱、大站多魔改）`[实证]`。
- **我们**：requests + offers 双表（offer_votes 前置校验本轮修复；悬赏归属校验——仅发布者可应种）。

> **借鉴**（P2）：① **Gazelle 的"悬赏=上传量"选项**——用上传量悬赏比用火花更硬（用户更心疼），我们可做双币悬赏；② **U3D 的认领过期回收**（7 天）防占坑不做；③ **NP 的 Offer 投票制**我们已经有了（offer_votes），差异在 NP 是"10 票赞成才解锁发布权"——门槛制 vs 我们的平行的求种/悬赏，可评估。

## 12. 死种治理

| 架构 | 机制 |
|---|---|
| NP | 无种+超时→visible=no 隐藏；可选物理删除（deldeadtorrent，默认关）；发布者删种扣 15 魔力 |
| UNIT3D | **Graveyard 复活任务**：0 做+龄>30 天+非本人→领任务→补做种 30 天→奖 5 FL token+种 7 天免费 bump+聊天盒广播 `[实证]`；无自动删种 |
| Gazelle | **Reaper 两段式通知**（INITIAL/FINAL 两次预告）+ torrent_unseeded 状态表 + NEVER/UNSEEDED 两态 `[实证]` |
| **我们** | preserve 保种区（seed_preserve/seed_milestones 保种里程碑） |

**我们独有的"保种区"是正向设计**（教育内容存档价值高，不该删），但缺**复活激励**：死种了没人有动力救。

> **借鉴**（P1）：**复活任务制**（U3D Graveyard）——保种区中的无种种子开放"复活任务"，完成奖火花+FL 券+全站广播，与我们的 seed_milestones 打通（"救活 10 个死种"成就/考核指标）。这比 U3D 更贴教育站定位：**老教材种被救活的长期价值远高于影视站**。

---

# 第五部分 · 社区与留存

## 13. 社区组件密度表

| 组件 | NP | UNIT3D | Gazelle | **我们** |
|---|---|---|---|---|
| 论坛 | 三层（OverForum→Forum→Topic）+已读跟踪 | 四层+@提及+赞踩+poll+标签 | 论坛+ForumTransition 版块流转 | forums/topics/posts+版主 |
| 实时聊天 | 水楼 shoutbox（type=sb）+求助区（hb）分立 | **聊天盒多房间+echo/audible 分流+SystemBot/NerdBot 15 命令**（redis 广播+laravel-echo-server，**未用 Reverb**）`[实证]` | — | shoutbox（有，无机器人） |
| 成就 | —（勋章体系替代） | **53 个成就类**（发帖/评论/上传/填求种/字幕五族） | RANKING_WEIGHT 十维排名 | medals（勋章）+seed_milestones |
| 通知 | 站内信 | **54 通知类**，database/SystemChannel/mail 三通道，**32 字段用户级开关+按来源组屏蔽** `[实证]` | 9 类型×4 通道（Popup/Traditional/PM/Push）逐类开关 | messages+push_subscriptions |
| 打赏 | 帖子打赏/种子打赏 | TorrentTip/PostTip（BON 划转+通知+聊天盒广播） | Gift 降临历 | ❌（有转账档位 1-10000） |
| 愿望单 | — | **WishList（TMDB）新上传自动匹配通知** | NotificationFilter（艺人/厂牌订阅） | ❌ |
| 年度盘点 | — | YearlyOverview | — | ❌ |
| Playlist/合集 | — | Playlist+zip 打包 | **Collage 10 分类+订阅+贡献榜+个人槽位购买** `[实证]` | ❌ |
| 字幕 | subs 表+命中率+5 魔力 | 字幕成就族 | — | subtitles+举报（本轮接线） |
| 趣味 | 趣室 Funbox 投票/幸运转盘（插件） | Event/Prize 抽奖+菠菜（M-Team 也有 betgame） | Gift 降临历+AotM | 趣事投票+五子棋+农场+刮刮乐/猜大小/九宫格 |

**社区组件的数量差距是真实的**（U3D 53 成就+54 通知 vs 我们 0 成就+简通知），但**方向差距不大**——我们缺的是"成就+通知矩阵+愿望单"三个留存件，不是缺整个社区观。

> **借鉴**（按投入产出排序）：① **P1 通知矩阵**——54 类通知的骨架其实是"事件→通知→用户级开关"三段，我们 messages+push 基建已有，缺的是**通知偏好设置页**（哪怕只做 8-10 类高频通知的开关）；② **P1 愿望单**——教育版=**"课本/课程上架提醒"**（订阅教材名/科目/年级，新种匹配即推送），这是比 U3D 影视愿望单更刚需的场景；③ **P2 成就系统**——从 seed_milestones 扩展即可（保种量/救种数/发种数/论坛活跃四族起步），不需要 U3D 的 900 分大而全；④ **P2 聊天机器人**——SystemBot 的 `gift` 命令+NerdBot 的统计命令（/free /peers /uploads）成本一天，社区感收益大。
> **避坑**：U3D 的聊天盒用 laravel-echo-server v1.6.3（**半死的生态**，v2 未核实，README 与 docs 均无 Reverb）——我们若做实时，直接 WebSocket 原生或成熟库，别走 Livewire 时代的旧栈。

## 14. 论坛与内容治理

- Gazelle **Wiki 按篇设置编辑权限**（minClassEdit）+ alias/revision；U3D Wiki+Pages（规则/FAQ/上传指南默认指向 pages/1、/2、/4）+ 工单 Ticket 体系（分类/优先级/指派）。
- NP：规则页 rules 表（后台 modrules 可编辑）+ FAQ seeder + **已读跟踪 readposts**（回帖定位到最后读帖）。
- 我们：site_rules/faq_items（FAQ/规则编辑 sort 本轮修复）+ 论坛 flood 控制。

> **借鉴**（P2）：① **论坛已读跟踪**——"上次读到第 N 楼"是长帖体验的分水岭（readposts 表思路）；② **工单体系**（U3D Ticket）——我们的 contactstaff/staffbox 是单向的，工单化（状态/指派/时效）是管理效率升级；③ Gazelle 的**规则页 Wiki 化**（带版本历史）——规则修订可追溯，教育站尤其重要。

---

# 第六部分 · 搜索与信息架构

## 15. 搜索对照

| 架构 | 引擎 | 特色 |
|---|---|---|
| Gazelle | Sphinx/Manticore 双索引（delta 每分钟 rotate、全量 2h） | **14 个全文域**（filelist 也能搜！）+ 数值属性过滤 + `!` 排除语法 + ngram CJK `[实证]` |
| UNIT3D | Meilisearch（唯一 Searchable=Torrent，15 分钟增量同步；**RSS 也走 Meili**） | 容错搜索/即时搜索 quicksearch API |
| NP | 裸 SQL（可选 Meilisearch/Elasticsearch，默认关） | searchbox 按分区差异化 |
| **我们** | pg_trgm GIN | 零外部组件；按文件名搜索（files 表本轮补齐写入） |

**结论**：十万级种子内 pg_trgm 完全够；Gazelle 的"**filelist 全文域**"我们已经等价实现（files 表+文件名搜索）；Meili 双写预案保留（v2 已列）。

> **借鉴**（P2）：Sphinx 的 **`!` 排除语法**与"按聚合组排序"（组视图 MAX(size)/SUM(seeders)）值得在列表页搜索语法里补——教育站"搜课本名-排除答案册"是真实场景。

## 16. 种子列表信息设计

- NP：列表行**下载按钮直链**（工具抓取的基础）、促销图标、置顶两级、精选四型（hot/classic/recommended）。
- U3D：Trending 榜（**忽略 <1GiB 防刷**）`[实证]`、dying/dead 过滤器。
- 我们：列表行下载按钮 Bearer blob（本轮修复）、六档促销标、置顶促销。

> **借鉴**（P3）：Trending 的"忽略小种"规则——我们的热门榜若有刷量漏洞同款适用。

---

# 第七部分 · 风控与反作弊

## 17. 反作弊机制全景

| 维度 | NP `[实证]` | UNIT3D `[实证]` | Gazelle | **我们** |
|---|---|---|---|---|
| 客户端管控 | 三层匹配（peer_id 正则+版本数值比较+UA 正则）+浏览器 UA 拒绝+黑名单缓存 | **peer_id 前缀黑名单**（BlacklistClient）+同步外挂 tracker | xbt_client_whitelist 白名单 | agent_rules 黑白名单+命中→cheat_events+staffmessages（0069 闭环） |
| 速度异常 | **8000MB/s 铁线入 cheaters，半值疑似**；上传>1GB+速>1MB/s+leechers 少 | balance 结余字段（上报-实际差值）+times_cheated 排序 | —（无独立模块） | cheat_events（agent 违规）；**速度异常 ❌** |
| Ghost Leech | — | history seeder=0/active=0/seedtime=0 疑似名单 | — | ❌ |
| 泄露检测 | — | **LeakerController**（谁先下载+时间窗+agent 找抢发/泄 passkey）+ TorrentDownload 全量审计 | — | login_events+download 审计？ |
| 多账号 | ipcheck 按 IP 分组+iphistory 90 天+maxip=2 | InviteTree+FailedLogin+Watchlist | ip_history+LoginWatch | ip_bans+email_bans（本轮接线）+login_events |
| 盒子识别 | seed_box_records（>10240KB/s 视为盒子） | Seedbox IP 登记+**AutoHighspeedTag 自动打标** | — | ❌ |
| connectable 检测 | TBSource 时代 fsockopen（同步阻塞，死因之一） | announce 时主动连端口（30min 间隔）+BON 公式引用 | — | ❌ |
| 举报 | reports+ReportAuto 自动报告 | Report（可指派/snooze） | reportsv2+**证据要求字段**（need_image/link/sitelink） | reports+状态机（0068 本轮落地） |

> **借鉴**（P1，风控补强三件套）：① **速度异常检测**——NP 的双阈值（铁线/疑似）+ U3D 的 balance 结余（**累计上报-实际差值**）都是 announce 流水上的离线统计，我们的 traffic_ledger 事实源天然支持，在 worker 加一个 cheat_scan 任务即可，**成本最低收益最大**；② **盒子识别自动打标**（U3D）——教育网服务器用户识别后可给"高保种"激励而非打击；③ **泄露者检测**（U3D Leaker）——passkey 泄露/抢发是教育站真实威胁（考前试题泄露！）。
> **避坑**：TBSource 的同步 fsockopen 教训——**任何探测都别进 announce 热路径**，我们若做 connectable 检测放 worker 异步。

## 18. IP 治理与教育网适配

- NP：maxip=2（同 IP 双号上限）——**校园 NAT 出口的天然敌人**；iphistory 90 天。
- 我们：**按 user_id 限流**（正确）+ ip_bans 段封禁 + email_bans。
- U3D：BlockedIp 可过期封禁。

> 结论维持 v2 判断：教育网共享出口场景，**绝不引入 NP 式同 IP 账号数上限**；代理检测（maxip 系）只做**展示**（同 IP 用户列表给 staff）不做自动处罚。

---

# 第八部分 · API 与工具生态（对生存影响最大的域）

## 19. 生态位地图（谁被什么工具支持）

| 工具 | NP | UNIT3D | Gazelle | TNode | YemaPT | M-Team | **我们** |
|---|---|---|---|---|---|---|---|
| ptool（刷流/辅种） | ✅ 全能力 | 部分（状态/列表/下载） | 部分（gazellepw） | 状态+下载 only `[实证]` | — | ✅ 专属 mtorrent type 全能力 | ❌ |
| PT-Plugin-Plus（148 站） | ✅ 主力 schema | ✅ schema | ✅（GazelleJSONAPI） | ✅（zhuque 专用 schema+ptppUserInfo） | ❌（未收录） | ✅（仍标 NexusPHP） | ❌ |
| PT-depiler（300 站 MV3） | ✅ | ✅ | ✅ | ✅ zhuque.ts | ✅ **yemapt.ts 独立 schema** | ✅ | ❌ |
| MoviePilot | ✅ | ✅ | ✅ | — | ✅（SiteSchema.Yema） | ✅ | ❌ |
| IYUU 辅种 | ✅（passkey+uid） | — | — | — | — | ✅ | ❌（需站方与 IYUU 合作） |
| cross-seed | 经 Prowlarr/Jackett Torznab | 同 | 同 | 同 | 同 | 同 | ❌（同左，Torznab 出口） |

**核心结论（v2 结论的实证强化）**：工具圈的门票有**三种卖法**——
1. **长得像 NP**（HaresClub/M-Team 保留 NP 页面，零成本继承 148 站配置）；
2. **给插件做专用聚合端点**（朱雀 `/api/plugins/ptppUserInfo`，一个端点换一个 schema 收录）；
3. **开放 API+独立 schema**（YemaPT，wiki 文档+PT-depiler/MoviePilot 主动适配）。

我们的 compat 层已走第一条路（`/compat/nexusphp/*`），**但还差最后一步**：让 PT-Plugin-Plus 收录我们（config.json PR）。

## 20. YemaPT / M-Team / 朱雀 API 设计精读（我们 compat 层的三个对照物）

### YemaPT（最规范的自研开放 API）
- 凭据：AuthKey 180 天/3 个/Cookie 不可用（403 专用码）；**Authorization 裸填不加 Bearer**。
- 六端点覆盖握手/身份/辅种/列表/凭证/上传；**列表无总数接口**（不足 pageSize 即末页+id 去重，文档写明）；上传幂等建议（超时先查 piecesHash 再重试）。
- **反面**：2026-08-13 硬删旧 GET 端点"不再提供兼容" `[实证]`。

### M-Team（最大的 API-first 大站）
- 全 POST + x-api-key；分域清晰（torrent/member/msg/seek/examine/bonus）；**genDlToken 一次性下载令牌**；msg 14 端点支持信箱分盒；**queryTorrentTrackerHistory 端点=H&R 数据开放**；Telegram 绑定端点 `[实证]`。
- 启示：API 覆盖面=App 功能面（他们的 iOS/Android 就建在这套 API 上）。

### 朱雀/TNode
- cookie+CSRF（DOM meta 标签）为主，**ptppUserInfo 一个聚合端点解决插件生态**；advancedSearch JSON 端点服务 PT-Plugin-Plus 搜索 `[实证]`。

### 我们 compat 层现状与缺口

已有：`/compat/meta` 自描述、`/compat/nexusphp/user.json|torrents.json|torrent/{id}.json`、`download.php?id=&passkey=` 形状下载、pieces_hash 批量反查、30min 下载凭证（fxk_）、API Token 180 天/3 个（0069 全落地）。

| 缺口 | 对照 | 优先级 |
|---|---|---|
| **PT-Plugin-Plus config.json 收录 PR** | 朱雀/Hares 模式 | **P0**（生态入场最后一步） |
| ptppUserInfo 式聚合端点（插件字段口径） | 朱雀 | P0（收录 PR 的前提） |
| 上传 API（含幂等建议） | YemaPT uploadTorrent | P1 |
| msg/notice 端点（App 化预留） | M-Team | P2 |
| Torznab 出口（cross-seed/Prowlarr 接入） | cross-seed 只认 Torznab | P2 |
| 旧端点弃用政策（≥2 版本周期+deprecation header） | YemaPT 反面 | 规则（不开发，写进 API 文档） |

## 21. RSS 对照

- NP：torrentrss.php（passkey+搜索参数，300s 缓存）+ getrss.php 订阅链接生成（**我们已对齐**，20be389）。
- U3D：**RSS 走 Meilisearch**（存储过滤条件 JSON，limit 50，缓存 5-6 分钟）。
- 我们：rss_http 多选+关键字+付费过滤（本轮 paid=1 修复）。

> RSS 域我们已达到 NP 口径；U3D 的"过滤条件持久化"我们 getrss 页已等价。无需新增。

---

# 第九部分 · 管理后台与运营工程

## 22. 后台规模对照

| 架构 | 后台形态 | 规模 |
|---|---|---|
| NP | Filament 4 组资源+传统 staff 工具（staffpanel/modtask/massmail/increment-bulk） | ~30 资源+10 工具页 |
| UNIT3D | 62 个 Staff 目录（含 bon 规则编辑器/leaker/cheated-torrent/seedbox/event-prize/backup/command 面板/version 检查/flush） | **最大** |
| Gazelle | 分散（无统一 Filament） | 中 |
| **我们** | 三模块 122 端点（admin 49 + p2 16 + p3 57）+ settings 六件套（schema/validate/groups/history/export/import） | 数量大，**i18n 未接**（admin-p3 十一组件） |

> **借鉴**（P2）：U3D 后台的三个"运维件"值得抄：① **backup 面板**（spatie backup 每日 DB+文件，我们 compose 有备份脚本但无后台可视化）；② **command 面板**（artisan 命令白名单执行——我们等价物是 worker 任务手动触发器）；③ **version 检查页**（对外报版本+更新提示）。
> **避坑**：我们 admin-p3 的 11 个组件 i18n 未接是**已知遗留**——U3D 60 语言包靠 Weblate 协作，我们三语都靠手工，组件数越多这笔债越大，建议 i18n 接入作为 admin 组件合并的前置条件。

## 23. 运营配置化程度

- U3D：41 个 config 文件+other.php 40+ 项（全文件级，**改配置=重启**）；BON 规则/自动促销规则表是 DB 级。
- NP：settings.default.php 巨表+Filament 可改；**做种积分阈值硬编码**（反例）。
- 我们：site_settings+settings_meta+settings 六件套（validate/history/export/import）——**配置化程度三家最强**，这是真实优势，保持。

## 24. 定时任务对照

| 架构 | 调度 | 任务数 |
|---|---|---|
| NP | cron cleanup 5 级优先级（15min-15 天） | ~30 |
| U3D | laravel scheduler（5s-每日-周末） | **~36 个 auto:* 命令** |
| Gazelle | scheduler 每 15 分钟+task 表 | ~20 |
| **我们** | worker 60s tick×12 + 3600s×2 + 银行日结链 | 14 个任务 |

我们的任务数少但覆盖了三家的高频核心；**缺的三个**：闲置账号清理（§3）、速度作弊扫描（§17）、闲置种子通知（Reaper 式两段预告）。

---

# 第十部分 · 部署运维与性能

## 25. 组件数与性能基线

| 架构 | 生产组件 | announce 性能 |
|---|---|---|
| NP | PHP-FPM+MySQL/PG+Redis+Memcached+supervisor（6+） | PHP 世代：~250 req/s/核（U3D 数字旁证） |
| UNIT3D | Nginx+PHP-FPM+MySQL+Redis+Meili+Supervisor+echo-server+cron（**8+**） | PHP 250 req/s/核 / Rust announce **50k**（官方 README） |
| Gazelle | nginx+PHP+PG+MySQL 双库+memcached+sphinx（**6**） | Ocelot C++（TCP only/v4 only） |
| Torrust/Aquatic | 单二进制/纯内存 | **Aquatic UDP 16 核 io_uring 225 万 resp/s** `[实证]`；单核 22.6 万 |
| **我们** | PG+Redis+api+worker+tracker+web（**6 容器**） | 未压测；架构上限=DashMap 分片内存操作 |

> **借鉴**（P2）：① **tracker 压测基线**——用 aquatic_bencher 或 wrk 给我们的 tracker 出第一个正式数字（当前一切性能结论都是架构推断）；② Aquatic 的**按 info_hash 分片**（swarm load 分片）是 DashMap 单表之外的下一档；③ U3D 的 **Unix socket 连 Redis 提速 50%** 文档值得在部署指南补记。

## 26. 国际化

- U3D：60 语言包+Weblate；NP：19 语言+Crowdin；我们：三语手工（zh-CN 1987 键源字典）。
- 结论：三语对教育站够用，但 **admin 组件 i18n 债**（§22）必须还，且随着功能增长会加速恶化。

---

# 第十一部分 · 总借鉴/避坑清单（结论层）

## 27. 借鉴清单（按优先级，含来源与落地形态）

### P0（生态入场最后一步，做完才算"活的 PT 站"）
1. **PT-Plugin-Plus 收录**：先实现 `/api/plugins/ptppUserInfo` 聚合端点（朱雀口径：id/name/bonus/uploaded/downloaded/invites/levelName/joinTime/seeding/seedingSize/messageCount），再向 pt-plugins/PT-Plugin-Plus 提 config.json PR（searchEntry 指向 compat 层 JSON 端点）——**一个 PR 换 148 站生态位的入场券**。✅ 端点已上线（迁移 0072，含 config.json+解析脚本模板 `.research/ptpp-config-template/`）；**剩余动作：向 PT-Plugin-Plus 仓库提收录 PR**
2. **兼容层 API 文档公开化**：把 openapi.json+compat 端点整理成静态文档页（YemaPT wiki 模式），承诺"破坏性变更提前一版公告"。✅ 已写 `_doc/开放API接入指南.md`；对外发布渠道待定

### P1（功能域补强，全部有清晰对照物）
3. **做种收益稀有度+时长饱和**（NP arctan/U3D 规则名化）+ 收益明细页。✅ 已落地（0074：规则分×seeders^-0.35×90 天半衰，arctan 软封顶渐近 +200/h；4 个公式锁定单测；/me/spark 按规则名返回收益构成+公式说明）
4. **等级降级线+晋升待遇**（NP 降级线+送邀请表；U3D 组彩色样式）。✅ 晋升待遇已落地（0072：class_rules.promo_sparks 阶梯 200-20000 火花，worker class_auto_adjust 升级即发+系统消息，幂等键防重复）；降级线本就有 demotable 机制（v3 §2.4 对照勘误：worker 降级逻辑与升级同源，非缺口）
5. **FL 券/中性券上商店**（Gazelle token 4% 核销+tracker 同步）。✅ 已落地（0073：user_vouchers + 商店上架 2500/5000 火花 + /me/vouchers 用券 + worker 计费叠加与 4% 事务内核销）
6. **H&R 三件套**：预警 PM（U3D prewarn）+buffer 10% 豁免（U3D）+免罪券（NP 1 万魔力口径→我们 ≥10 万火花）。✅ 全部落地——预警 PM（0072：48h 内到期未达标→站内信，live 实测 gaozhong 收到）；buffer 豁免（下载 <10% 不建快照）；免罪券此前已有（`/me/hr/pardon` 20000 火花自助免罪，v3 §10 勘误：非缺口）
7. **复活任务制**（U3D Graveyard→我们的保种区+seed_milestones 联动）。✅ 已落地（0073：/resurrections 领取 + worker 自动验收 → 5000 火花+免费券+7 天 free bump）
8. **速度作弊扫描**（NP 双阈值+U3D balance 结余，worker cheat_scan 离线任务，读 traffic_ledger）。✅ 已由并行批次落地（0071 `cheat_audit`：Σup−Σdown 差值审计+staff 信箱告警，10min 周期；另有 ratio_watch/multi_ip_check/connectable 回连）
9. **闲置账号三段清理**（U3D 90 天/Disabled/软删）。✅ 收敛版落地（0072 `dormant_mark`：90 天未登录+无做种+非员工/捐赠者→停用；登录拦截+恢复指引；不做自动删除）
10. **聚合组补完**：上传自动推荐入组+组级订阅（教材改版通知）。✅ 已落地（0075：pieces_hash 命中锁定推荐 + trgm>0.4 相似候选；group_subscriptions 表 + 订阅/退订端点 + 过审推送）
11. **通知偏好设置页**（U3D 32 字段开关的简化版，8-10 类高频通知）。✅ 已落地（0075/0076：notice_prefs JSONB + u_notice_enabled SQL 函数（worker/API 共用防漂移）+ /me/notice-prefs 端点，hr_prewarn/wishlist/resurrection/class_promo/group_new_version 五类已接过滤）
12. **教材愿望单**（U3D WishList 教育化：订阅科目/年级/教材名，新种匹配推送）。✅ 已落地（0074：wishlist 表+/wishlist 端点+worker 每小时扫新过审种子匹配推送，每用户聚合一信+24h 节流）

### P2（体验与运营精细化）
13. ✅ 站免池荣誉层已落地（0075：v_pool_honor 视图 + /pool/honor 贡献榜端点）；定向众筹免费（HDBits Featured）⏳。
14. ✅ 全部落地（0075/0077：approve_streak 连击≥5 免审通道 + deny_count≥2 禁发（阈值 site_settings upload_deny_limit 可调）；POSTPONED 态保留未做（approval_status 3 已被"下架"占用，需腾挪语义，单独排期）。
15. 盒子识别打标+泄露者检测（U3D）。
16. 论坛已读跟踪+工单体系+规则页版本化。
17. ✅ 自动促销规则表已落地（0077：auto_promo_rules——name_regex/size 区间/分类 → 六档促销×时长，过审时按 position 取首条命中）；Refundable ⏳。
18. 后台运维三件：backup 面板/任务手动触发器/版本页（U3D）。
19. ✅ peer 超时分档已落地（0077：做种 peer TTL 90s→3720s=2×interval+120，下载 peer 保持 90s 快速感知——口径矛盾修复）；✅ 压测基线已出（0079：2138 req/s @32 并发 / p99 13.45ms / 零失败，录于生产部署指南 §6.1）。
20. ✅ 全部落地（0078 聊天机器人 /free /stats /me /help；0079 成就四族：achievement_defs 表驱动 9 定义 + worker 每小时授予 + 火花奖励 + /me/achievements）。
21. ✅ Torznab 出口全部落地（0077 caps + 0079 search 端点：q→trgm 搜索→atom，开放 API Token 鉴权，enclosure 指向 download.php——Prowlarr/cross-seed 可注册）。
22. ✅ 全部落地（0077 月度对账 v_spark_flow_monthly + /admin/spark-flow；0078 赠送税 gift_tax_bp 5% 基点可调（礼物/众筹双链路入 magic_pool）；0079 日度对账 v_spark_flow_daily + /admin/spark-flow/daily——净增率仪表常驻化）。

### P3（远期）
23. 上传 API（YemaPT 幂等口径）+msg/notice API（M-Team App 化预留）。
24. Telegram 绑定通知（M-Team）。
25. 后宫加成/邀请分成（NP harem）。
26. 个人 Collage 槽位（Gazelle 递增定价）。

## 28. 避坑清单（每条都有尸体）

| # | 坑 | 尸体 | 我们的防线 |
|---|---|---|---|
| 1 | announce 同步写库 | TBSource（4-6 条 UPDATE+RAND()+fsockopen） | 三代架构已免疫；**拒绝任何把 DB 拉回 tracker 的需求** |
| 2 | hack 式二开生态 | XBTIT modification.xml 文本插桩 | 扩展走插件契约（plugins.rs 已有雏形），永不 patch 源码 |
| 3 | 重写半途而废 | TBSource BrokenWings（人消失）/JS 系集体回迁 | 增量交付纪律；不开第二运行时 |
| 4 | 弱事务记账 | meanTorrent（Mongo 计费无事务，2020 死） | PG+流水+行锁幂等是正解；**娱乐玩法新增必须带 EV<1 断言**（jgg 教训） |
| 5 | 逐文件拼 SQL | NP 31 条 CVE（9.8 注入 6+ 条） | sqlx 参数化+类型系统；盯 dangerouslySetInnerHTML/下载路径拼接/新管理接口越权三处 |
| 6 | 同步探测进热路径 | TBSource fsockopen 5s | 探测一律 worker 异步 |
| 7 | 同 IP 账号上限 | NP maxip=2 vs 校园 NAT | 只展示不自动处罚 |
| 8 | 硬删 API 端点 | YemaPT 2026-08-13 删 GET 无过渡 | 旧端点 ≥2 版本周期+deprecation header |
| 9 | 参数硬编码 | NP 做种积分阈值在 User.php | 一切规则进 site_settings/class_rules/agent_rules |
| 10 | 组件膨胀 | U3D 8+ 组件（Meili/echo-server/...） | 6 容器封顶；新需求先问"pg+redis 能不能做" |
| 11 | Livewire 式全栈耦合 | U3D 页面组件长业务逻辑 | API 契约+前后端分离不破例 |
| 12 | 通知无开关轰炸 | （反例：U3D 32 字段是正解方向） | 通知矩阵上线时**必须同带偏好开关** |
| 13 | 概率型测试写死采样数 | （自家 jgg 测试教训） | 从表内最小权重推导；调赔率表必须三处同步 |
| 14 | 凭据长期通吃 | （对照 YemaPT 30min 凭证） | 已落地 download_keys；新下载类端点一律凭证制 |

## 29. 一句话总结

> v2 说对了一半：架构代差是真的，生态入场券也是真的。v3 补上的是另一半——**功能域上 NP 的规则书（17 级/四条件/H&R/道具价目）、U3D 的工程化（DB 可配规则引擎/53 成就/54 通知/62 后台）、Gazelle 的结构智慧（两级聚合/动态 RequiredRatio/FL token 生命周期）都不是架构先进性换来的，是 15 年运营教训堆出来的**。我们的三代计费+教育垂直域是独有资产，但每一条"借鉴清单"背后都是别家用户已被教育过的预期——**用户不会因为你的架构先进而原谅你缺一张 FL 券**。

---

## 附录 A · 本文引用的一手源码/文档坐标

| 对象 | 坐标 |
|---|---|
| NexusPHP v1.10.2 | 本地 `.np-research/np/`（xiaomlove/nexusphp php8）；关键文件 include/cleanup.php、include/functions.php L6103、app/Models/{User,Torrent,HitAndRun}.php、nexus/Install/settings.default.php、public/mybonus.php |
| Gazelle master | github.com/OPSnet/Gazelle（58e3b9e）；app/{TGroup,Torrent,Request,Bonus,Collage}.php、app/Manager/User.php、app/Search/Torrent.php、misc/pg-migrations/（88 个）、docs/07-API.md、sphinx.conf、ocelot.conf |
| UNIT3D v9.2.0 | github.com/HDInnovations/UNIT3D；config/{hitrun,graveyard,donation,other}.php、database/migrations/2024_12_26（BON v2）、app/Achievements/（53）、app/Notifications/（54）、resources/views/Staff/（62） |
| UNIT3D-Announce | github.com/Roardom/UNIT3D-Announce（Rust/axum） |
| TBSource | github.com/QwertyRider/TBSource（70 根级 php+22 表 SQL） |
| XBTIT/xbtt | github.com/Q8HMA-zz/xbtit + gubatron/xbt-tracker（server.cpp 内存缓冲） |
| M-Team API | github.com/harrisoff/mteam-api（官方 OpenAPI 3.0.1 镜像，全端点） |
| YemaPT | wiki.yemapt.org/developer/open-api（2026-08-13 版） |
| 朱雀/TNode | github.com/sagan/ptool/tree/master/site/tnode + PT-Plugin-Plus resource/schemas/TNode |
| ptool | github.com/sagan/ptool（site/*.go；7 种 type 实测） |
| PT-Plugin-Plus | github.com/pt-plugins/PT-Plugin-Plus（148 站 config.json 逐一核对） |
| PT-depiler | github.com/pt-plugins/PT-depiler（300 站 definitions，含 yemapt.ts） |
| IYUU | doc.iyuu.cn（reseed/index 单批 ≤300、users_bind 需 passkey sha1） |
| cross-seed | github.com/cross-seed/cross-seed（Torznab only+文件树比对） |
| AnimeBytes 池 | github.com/hlfbt/AnimeBytes-Userscripts（konbini pool 解析脚本佐证） |
| HDBits Featured | github.com/mikz/prss + pyhdbtools |
| Aquatic 压测 | github.com/greatest-ape/aquatic documents/aquatic-udp-load-test-2024-02-10.md |
| Torrust | github.com/torrust/torrust-tracker（BEP 3/7/15/23/27/48） |
| 我方 | 本仓库 2026-09-13 快照（commit 84f8c46 附近）：116 表/299 路由/68 权限/14 任务/54 页 |

## 附录 B · 我方缺口速查（与 §27 编号对应）

源码级自认缺口（盘点确认）：转载/特殊分类上传 `implemented=false`（0062）、BEP12 多 tier、tracker 90s 口径、admin-p3 i18n、CSP nonce 基建、hr.exempt 无 API 引用点。
对照新增缺口：闲置账号清理（#9）、速度作弊扫描（#8）、H&R 预警/buffer/免罪（#6）、FL 券（#5）、通知偏好（#11）、愿望单（#12）、会话管理（§3）、免审通道（#14）、ptppUserInfo 端点（#1）。

---

*整理：2026-09-13 深夜（v3）· 基于 6 路源码级/官方文档级调研 + 本仓库实测盘点；v2 的架构级结论在本文 §1 得到四代范式修正与强化，其余以功能域事实为准。*
