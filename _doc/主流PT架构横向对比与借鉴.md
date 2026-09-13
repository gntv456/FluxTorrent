# 主流 PT 架构全谱系深度研究与借鉴（v2）

> 视角：以本仓库 FluxTorrent（Rust Actix-web + Next.js 15 + PostgreSQL 16 + Redis + 自研内存 Tracker）为基准，横向剖解 **TBSource / NexusPHP / XBTIT / Gazelle / UNIT3D / Discuz-PT / JS 系 / TNode / YemaPT / Custom / Rust-Go 新一代** 共 11 个派系，输出**可借鉴的优点**与**需避免的缺点**。
>
> **版本说明**：本文是 `主流PT架构横向对比与借鉴.md` v1（2026-09-12）的升级版。v1 的骨架与结论大多保留，本次主要做三件事：① 把 v1 中用推测写出的 TBSource / XBTIT / Discuz-PT / JS 系 / TNode / YemaPT 六派换成**可核查的一手事实**；② 新增**第三方工具生态适配**这一决定生死的维度（§4）；③ 修正 v1 中若干不准确表述。本侧事实以仓库代码为准（2026-09-13 快照）。
>
> **证据分级**：`[实证]` = 抓到官方仓库/官方文档/官方 CVE 原文；`[业内]` = 多篇从业者文章交叉印证；`[推断]` = 基于间接证据的合理推演，已标注。

---

## 1. 谱系与现状：一手证据表

| 派系 | 出身与血缘 | 技术栈 | 现状证据 | 判定 |
| :--- | :--- | :--- | :--- | :--- |
| **ByteMonsoon** | 2004，祖源 | PHP + MySQL | TBSource 官方自述"Originally based on the ByteMonsoon source" `[实证]` | 已灭绝 |
| **TBSource** | 2005-11 注册 SF，承 ByteMonsoon | PHP 98.4% + CSS 1.6%，GPL-2.0，**扁平单文件结构**（约 80 个根级 `.php`：`announce.php`/`takeupload.php`/`browse.php`/`my.php`…） | 最后一版 **2010-03-11**，仓库 README 明写 "This is TBSource Classic final SVN! No further updates" `[实证]` | **2010 停更** |
| **NexusPHP (NP)** | 浙大 Nexus 团队，官方自述 **"originated as a fork of the TBSource project"** `[实证]` | PHP + MySQL + Memcached（1.10+ 起 **过程式 PHP 与 Laravel 双轨** + Filament 后台） | 上游 GitHub 停更至 2011，分支（xiaomlove 等）续命；M-Team / HDSky / TTG 等大站在役且在持续魔改 `[业内]` | 活跃靠分支 |
| **TBDev / TorrentTrader** | 2004，Tbdev 源自 TorrentBits | PHP | XBTIT README 致谢清单明确列入 TorrentBits / TorrentTrader / Bytemonsoon / Tbdev `[实证]` | 已灭绝 |
| **XBTIT** | 2005-09 起 Lupin 接手 TorrentTrader → BtiTracker → 2006 重写为 xbtit | PHP + bTemplate 模板引擎 + MySQL；**两套 tracker**：PHP tracker（≤5–10k peers）与 **xbtt C++ 后端**；改版 BSD 许可 | 3.x 主分支 README 自称 "MASTER — DEVELOPMENT **& TESTING ONLY** (Not yet suitable for live environments)" `[实证]`；v3.1 要求 PHP 7.1–7.4 + MariaDB 10.1+ `[实证]` | 实质停滞 |
| **Gazelle (GZ)** | What.CD 2007 音乐站血统 | PHP + Twig，MySQL（OPS 分支迁 PG 中），Memcached，Sphinx/Manticore；tracker = **Ocelot（C++，仅 TCP）** | RED / OPS / PTP / BTN / GGn 在役 `[业内]` | 活跃（债重） |
| **UNIT3D (U3D)** | 2017 起 | **Laravel + Livewire + AlpineJS**，MariaDB strict，Redis 队列，**Meilisearch**，**Reverb** (WebSocket)，外挂 **UNIT3D-Announce** | GitHub ≈2.2k star，AGPL-3.0，活跃提交以天计 `[实证]` | **新站首选** |
| **Discuz-PT** | Discuz! X 论坛改 PT（`pt.php`），announce 混在 PHP 进程 | PHP 论坛插件 | **无公开仓库**，多为自写。代表：六维空间、北交知行、天雪PT `[业内]` | 遗存 |
| **JS 系** | Node 全栈尝试 | **meanTorrent**（MongoDB+Express+AngularJS+Node）、**sqtracker**（Node+React+Mongo+Docker）、**rartracker**（PHP+AngularJS） | meanTorrent **自 v1.8 停止开源**且该版本存在**严重信息暴露漏洞**；衍生站 MINE 长期 522、**PTDream+ 于 2019-07 宣布回迁 NexusPHP** `[业内]` | **整体失败** |
| **TNode** | 身份未公开，现代 API 型站 | 未知（从工具侧接口反推见 §4） | **被 ptool 原生列为一等站点架构 type** `[实证]` | 活跃 |
| **YemaPT** | 2024 起新锐（野马PT） | **前端 SPA + hash 路由 + Ant Design**（第三方脚本适配 PR 提到"页面事件暴露的 Ant Form 实例"）`[实证]`；后端未公开 | 自建 Wiki、**公开开放 API 文档**（详见 §4.7），持续更新 changelog `[实证]` | 活跃 |
| **Custom 大站** | 完全自研 | M-Team（ThinkPHP→新版+Vue，App/网页双端）、HDBits、HDSky、HDHome、AnimeBytes | 闭源，无标准 API | 活跃 |
| **Rust/Go 新一代** | 重写派 | **Torrust**（Axum + Vue，AGPL-3.0，多协议 + 管理 API + Prometheus）、**Aquatic**（io_uring）、**Arcadia**（Rust + Vue，模块化）、**NyaaPantsu**（Go） | `[实证]` | 活跃（ tracker 侧为主） |

### 1.1 两条"死 roads"，比任何优点都值得记住

- **TBSource 的 BrokenWings**：作者 YeOK 曾试图用 **CodeIgniter 重写** TBSource，仓库 `TBSource/BrokenWings` 停在 2011/2012，注释写着 `the author seems to have disappeared`。**重写半途，人消失，架构绝嗣** `[实证]`。
- **JS 系的集体溃败**：meanTorrent 停止开源 + 高危漏洞 + 站方回迁 NP。**PT 的核心诉求（流量计费的一致性、账目可审计）与 MongoDB 的弱事务模型天然冲突** `[推断]`，再加上"单维护者 = 单点故障"。

这两条共同指向一个结论：**PT 架构的存活率与"技术选型先进性"关系不大，与"有没有一群人能长期维护一行 eintragen 记账代码"强相关。**

---

## 2. 结构性代差：announce → 计费的三代范式

这是唯一真正分出高下的维度。其它差异大多是皮肤。

| 范式 | 代表 | 做法 | 后果 |
| :--- | :--- | :--- | :--- |
| **一代 · 同步写库** | NP、TBSource、早期 XBTIT、Discuz-PT | `announce.php` 每次请求直接 UPDATE `users` + `torrents` + `snatched` + `peers` | **`users` / `torrents` 表过热**，行锁竞争严重；站点规模一大就必须堆硬件。从业者明确指出 NP "大量查询与写入落在 users、torrents 表上，造成数据表过热" `[业内]` |
| **二代 · 内存累计 + 批量 flush** | Gazelle + **Ocelot**、XBTIT + **xbtt** | 站点与 tracker 分离，tracker 只在**共享的少数几张表**（Ocelot 仅 8 张）里累计，定时批量回写 | 这是 2007 年就给出的标准答案；代价是 **C++ tracker 无人敢改**（GZ/OPS 的长期痛点）`[业内]` |
| **三代 · 持久队列 + 异步 worker** | **我们 FluxTorrent** | tracker 零 DB 依赖 → 只 XADD 到 Redis Stream → worker 幂等消费 → 写分区流水表 | announce 路径无锁、无 DB；事实源是**流水**而非聚合值；失败可重放、可死信 |

**我们处于第三代，这是真实的代际领先，但要清醒**：NP 用一代范式撑起了 M-Team/HDSky 级别的实际规模。**架构先进性不会自动兑换成用户**，§4 的生态问题才是入场券。

### 2.1 与 XBTIT 的关键对照：同样是"站点 ↔ tracker 分离"，差别在哪

XBTIT 在 2006 年就做对了分离（PHP 前端 + xbtt C++ 后端），它的 README 甚至精确写明了选型边界："PHP tracker 适用于拿不到 root 或 peer 不超过 5–10,000 的场景……其它情况**全部推荐 xbtt**"。这句话本身是极好的工程判断。

但它死在了别处：**hack 生态**。XBTIT 的扩展靠"hack 模板系统 + 一键 hack 安装器"让用户往核心代码里打补丁，升级则说明"reapply your hacks to the new code"。**一套鼓励 Fork 核心代码的生态，注定无法升级。**

> **对我们的警示**：我们只有 open API 而没有扩展点时，要警惕慢慢长出"为某个站定制逻辑写进 core"的诱惑。真要做二开扩展，应当学的是**模块化组件 / 插件契约**，而不是 XBTIT 的 patch-style hack。

---

## 3. 逐纬度差异矩阵（我们 vs 全谱系）

| 维度 | TBSource / NP | XBTIT | Gazelle | UNIT3D | Discuz-PT | JS 系 | YemaPT / TNode | Custom 大站 | **我们 FluxTorrent** |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| 应用形态 | 单体 + 内嵌 announce | 单体 + 可选外挂 tracker | 单体 + 独立 C++ tracker | 单体 + 外挂 announce | 论坛插件 | 单体 SPA/Mongo | **前后端分离 SPA + JSON API** | 私有全栈 | **四服务 monorepo**（api / worker / tracker / web） |
| announce → 计费 | 同步写库 | xbtt 内存批量 | Ocelot 批量 flush | 队列 + 定时任务 | 同步写库 | 同步写库 | 未知（大概率同步） | 未知 | **Redis Stream → worker 异步增量** |
| 事实源 | 表内聚合值 | 聚合值 | 聚合值 | 聚合值 | 聚合值 | 文档快照 | 聚合值 | 聚合值 | **流水**（`traffic_ledger` / `spark_ledger` 分区表） |
| 数据库 | MySQL | MySQL/MariaDB | MySQL（OPS 迁 PG 中） | MariaDB strict | MySQL | **MongoDB** | 未知 | 自研 | **PostgreSQL 16** |
| 搜索 | 裸 SQL / 外挂 ES | 裸 SQL | Sphinx / Manticore | **Meilisearch** | 论坛搜索 | Mongo 索引 | 未知 | 自研（多为 ES） | **pg_trgm GIN**（零外挂） |
| 缓存 | Memcached | 基础缓存 | Memcached 强对象缓存 | Redis + 队列 | 论坛缓存 | 无 | 未知 | 强 | Redis（**读缓存层偏薄**） |
| 内容模型 | 扁平种子 | 扁平 | **Torrent Group 聚合** | 扁平 + type/resolution | 论坛帖 | 扁平 | 类目树 + 媒体属性 | **同资源聚合** | 扁平 + **教育域（课本/科目/年级/版本）** |
| 迁移体系 | 无 | 无（手工 upgrade.php） | 有 | Laravel migration | Discuz 机制 | 无 | 未知 | 私有 | **sqlx migrate，68 个递增迁移，只增不改** |
| 类型安全 | 无 | 无 | 无 | 弱（PHP 泛型缺失） | 无 | JS/TS 参半 | 未知 | 无 | **Rust + TS strict** |
| **第三方工具适配** | **默认兼容** | 半 | 部分 | 部分 | 微量 | 微量 | **有（被适配）** | 零（只有官方 App） | **零** |
| 运维组件数 | 3–4 | 3–5 | 5–6 | **8+**（含 Meili/Redis/Reverb/Supervisor/队列） | 3 | 2–3 | 未知 | 高 | **5**（PG / Redis / api / worker / tracker / web） |

---

## 4. 【本次核心发现】第三方工具生态：入场券是一张**架构分类表**

这是 v1 完全没抓到、但对我们风险最高的一件事。

### 4.1 一张决定命运的配置表

主流刷流/辅种工具 **ptool**（Go CLI，`sagan/ptool`）在配置文件里要求用户声明站点架构 type，官方原文给出可选值：

```
可选值: nexusphp | gazellepw | unit3d | tnode | discuz | mtorrent
```

它还补充："本程序没有内置支持的 PT 站点必须通过方式 2 配置……**部分非 NP 架构站点本程序目前只支持自动辅种、查看站点状态，暂不支持刷流、搜索站点种子等功能**。" `[实证]`

同时它的 CLI 目录结构本身就印证了这一点——`site/` 下并列存在 `nexusphp/`、`gazellepw/`、`unit3d/`、`tnode/`、`discuz/`、`mtorrent/` 等多个 adapter `[实证]`，其中 `tnode` adapter 实现了 `GetAllTorrents`（带 `pageMarker` 游标分页）、`SearchTorrents`、`DownloadTorrentById`、`PublishTorrent`、`GetStatus` 等一整套能力。

**结论（很重要）**：
1. 中文/国际 PT 工具圈把世界划分为 **6 种架构方言**。你的站点要么**长得像其中之一**，要么**说服作者新增 adapter**。二者都不占 = **工具生态零覆盖**。
2. **TNode 的存在本身就是最好的反驳**——它既不是 NP 也不是 U3D，是全新自研，但因为它暴露了**符合工具预期的那组 API**，ptool 就为它写了一个与 nexusphp **平级**的 adapter。这证明"自研 ≠ 不可适配"。**这是我们最应该抄的作业。**

### 4.2 我们目前的实际处境

- 我们有 **OpenAPI 端点 + API Token**（`apps/api/src/openapi_http.rs`，sha3-256 落库，限流 60 req/min）`[实证]`。
- 但**不在 ptool 的 6 个 type 里**，也没有任何第三方脚本认识我们的信封。
- **YemaPT 走了同一条路且走通了**：靠自建 Wiki + 稳定开放 API，主动加入 PT-Plugin-Plus / ptool / auto_feed_js 的适配讨论（有 Issue 与 PR 记录）`[实证]`。

### 4.3 YemaPT 开放 API 的设计细节（最值得逐条抄的一份作业）

抓自 `wiki.yemapt.org/developer/open-api`（更新 2026-08-13）`[实证]`：

| 端点 | 设计要点 | 对我们的启发 |
| :--- | :--- | :--- |
| `POST /openApi/user/fetchBasicInfo.json` | 返回 uid / bonus / level / 上下传量 / 邀请数 | 秒查型端点是所有工具的"握手"第一步 |
| `POST /openApi/user/authenticate.json` | **RSA 签名挑战**（`{userId}\t{randomContent}`，32 位随机串），供第三方校验 uid 真伪 | 低成本的**身份反冒用**，防止用户伪造 uid 骗 third-party 权益 |
| `POST /openApi/torrent/fetchTorrentIdWithPiecesHash.json` | **批量**（≤100）按 **piecesHash**（`info.pieces` 的 SHA-1，非 info_hash）反查 torrent id | ⭐ **辅种 / 跨站识别的命脉**，见 §4.4 |
| `POST /openApi/torrent/fetchOpenTorrentList.json` | 分页公开种子列表，`pageParam.current` 上限 1000、`pageSize` ≤40，**明确告知"不提供总数接口"**，要求对接方按 id 去重 | 主动牺牲 count 换取性能，并在文档里写清楚，是很成熟的做法 |
| `POST /openApi/torrent/generateDownloadKey.json` | 生成**临时下载凭证**，有效期 **30 分钟**，与用户+种子绑定 | ⭐ **凭证与长期密钥解耦**，见 §4.5 |
| `POST /openApi/torrent/uploadTorrent.json` | multipart 上传（可自动转种），文档明确建议"网络超时且无法确认结果时，**先调 piecesHash 接口查询是否已创建，再决定是否重试**" | ⭐ **把幂等设计left to API 消费者**并写进文档 |

**凭据策略**：API AuthKey 在个人页签发，**有效期 180 天、每人最多 3 个、可即时吊销**；Cookie 登录态**不可**用于开放 API（返回专用错误码 403）。

### 4.4 ⚠️ 我们缺一个字段：`pieces_hash`

核查 `apps/api/migrations/0001_init.sql:88`：我们的 `torrents` 表只有 `info_hash CHAR(40) NOT NULL UNIQUE`，**全库 migration 中没有 `pieces_hash` 字段** `[实证]`。

为什么它是刚需：

- **辅种（cross-seed）的本质**是"在不改变文件的前提下，用另一个站的同一种子文件去继续做种"。IYUU 的原理是从下载器取本地做种的 **`info_hash`** 提交到辅种服务器比对 `[实证]`，所以 info_hash 是**必要条件**——我们有，基本盘没丢。
- 但 info_hash 会因子典外字段变化而失效。YemaPT 单独提供 **piecesHash**（只由 `info.pieces` 内容决定），是为了在 **`source` 字段不同、跨站重打包** 等场景仍能匹配 `[推断]`。这是**更鲁棒的二级指纹**。
- 成本极低：一次新增迁移 + 上传时在 bencode 解析处补算一次 SHA-1。**收益是把"可被辅种生态识别"的概率从 80% 拉到接近 100%。**

> **建议 P0**：`torrents.pieces_hash CHAR(40)` + 索引，上传时写入，并对存量种子做一次回填。

### 4.5 ⚠️ 我们缺一个习惯：下载凭证与长期密钥分离

YemaPT 的关键是：**第三方不能拿长期 Authorization 直接下载种子**，必须先换一张 30 分钟、绑定单个种子的凭证。这样即使 API key 泄露，损失窗口也是 30 分钟且可定位。

对照我们：`apps/api/src/openapi_http.rs` 的 API Token 是长期令牌 `[实证]`，且**种子下载走同一套鉴权**。这是一个具体的、可修补的安全模型差距。

---

## 5. 值得借鉴的优点（分派系，含落地路径）

### 5.1 来自 TNode / YemaPT —— 生态适配的正确姿势 ⭐（本次新增，最高权重）

| # | 借鉴点 | 為什麼值得 | 落到我们这 |
| :-- | :--- | :--- | :--- |
| 1 | **主动"长得像一个已知方言"** | TNode 证明自研站只要 API 形状对，就能拿到平级 adapter | 选定一个方言做**兼容层**（优先 NP 口径，因为中文教育站用户手里的工具默认认它），在 `/api/v1` 之外挂一层，字段与行为口径对齐 |
| 2 | **官方维护一份可执行的 API 文档 + changelog** | 脚本作者敢写、敢长期维护的前提 | 把现有 OpenAPI spec 变成**带示例的公开文档站**，并承诺"破坏性变更提前一个版本公告" |
| 3 | **RSA 挑战式身份验证** | 防止第三方冒用 uid | 参考 `authenticate.json`，为第三方发放权益时提供签名校验端点 |
| 4 | **API key 时效 + 数量上限 + 即时吊销** | 泄露损失可控 | 我们有长期 Token，补 180 天有效期 + 每人上限 + 后台吊销 |
| 5 | 主动去工具仓库开 Issue/PR | YemaPT 是被纳入生态的**主动结果**，不是运气 | 向 ptool / PT-Plugin-Plus / auto_feed_js 提交适配 PR（我们提供 extractor） |
| 6 | ⚠️ **反例：硬性删除旧接口** | YemaPT changelog 写"删除 GET 旧接口，**不再提供兼容**"——每次都让一批脚本当场失效 | 我们定一条规则：**旧端点保留 ≥2 个版本周期**，用 deprecation header 提示而非直接 404 |

### 5.2 来自 Gazelle（GZ）

| # | 借鉴点 | 为什么值得 | 落到我们这 |
| :-- | :--- | :--- | :--- |
| 1 | **Torrent Group 聚合** | 一组多版本共享简介/封面/标签/评论，避免同内容刷屏 | 教育场景天然映射 **"教材/课程聚合页"**：同一本课本的多年份、多版本、多清晰度聚合。这是**最大的产品结构缺口** |
| 2 | **站点 ↔ Tracker 契约最小化** | Ocelot 只共享 8 张表，站点重构不牵动 tracker | 我们已是"tracker 只写 Stream、只读小表"，**守住边界**，别让新需求把 tracker 拉回 DB 依赖 |
| 3 | **对象级读缓存** | Memcached 缓存用户/种子对象 + 版本号整体失效，撑住百万 PV | 我们 Redis 主要做限流。**建议补：种子详情 / 用户统计 / 列表首屏**读缓存，带 cache key 版本号 |
| 4 | **Freeleech Token / Neutral Leech 券** | 比全站促销更精细，与经济系统天然耦合 | 商店/银行已有，可加**火花兑换 FL 券**、中性券，比全局促销更省成本、更有付费感 |
| 5 | **Ratio Watch（柔性观察期）** | 分享率跌破先警告给期限，不直接处罚 | 教育网多为宿舍共享出口，硬 H&R 误伤率高。新用户/低等级先走**观察期 + 提醒**，屡犯再计入 `hr_violations` |
| 6 | **客户端白名单执行闭环** | 白名单 + 异常速度检测是标配 | 我们有 `agent_rules` 表，**缺执行闭环**：命中 → 拒绝 + 记入 cheaterbox + 通知 staff |

### 5.3 来自 NexusPHP（NP）

| # | 借鉴点 | 为什么值得 | 落到我们这 |
| :-- | :--- | :--- | :--- |
| 1 | **工具默认适配口径** ⭐ | NP 真正的护城河不是代码，是工具圈把它当默认值 | 见 §5.1.1，做兼容层而非推倒重来 |
| 2 | **魔力值做种收益公式**（体积加权 + arctan 封顶） | 饱和函数，抗"堆一堆小种刷收益" | 我们 `seeding_reward` 是"基础+加成"，建议引入**体积与时长的饱和封顶**，参数进 `site_settings` |
| 3 | **运营颗粒度沉淀** | H&R 免罪券、补签卡、申诉、勋章条件——15 年规则细节 | **抄规则不抄代码**：逐条比对我们的 `appeals` / 勋章条件，补齐参数化配置 |
| 4 | **客户端白名单的实现细节** | 从业者公开的 announce 代码里，`agent_allowed_family` 用 **regex + agent 版本号比较 + exception 列表**三层匹配并缓存到 Redis `[业内]` | 我们的 `agent_rules` 可以照这个三层模型升级，成本不高 |
| 5 | **后台运营效率** | Filament 式批量操作 | 我们 admin 是 API 化，注意别"只做接口不做效率"：批量审核、批量封禁、活动配置化 |

### 5.4 来自 UNIT3D

| # | 借鉴点 | 为什么值得 | 落到我们这 |
| :-- | :--- | :--- | :--- |
| 1 | **社区"在场感"** | 聊天盒（Reverb）、成就系统、收藏夹、关注与动态流 | 我们有论坛/短讯，**缺实时**。补 WebSocket 聊天盒 + 自动化成就 |
| 2 | **工程化交付** | 一键安装/升级、Webrate 多语言协作、细粒度文档 | 补：**一键升级脚本、备份恢复演练、安装向导** |
| 3 | **搜索体验** | Meilisearch 中文分词、容错、秒级响应 | pg_trgm 十万级够用；到百万级或用户开始抱怨时，**主搜换 Meili、pg_trgm 兜底双写** |
| 4 | ⚠️ **反例：Livewire 全栈耦合** | 迭代快但重构难；v9.x 依赖 Meili + Redis + Reverb + Supervisor + 队列，运维组件膨胀 | 坚持 **API 契约 + 前后端分离**，业务逻辑绝不写进页面组件 |

### 5.5 来自 XBTIT / TBSource

| # | 借鉴点 / 教训 | 落到我们这 |
| :-- | :--- | :--- |
| 1 | ✅ **明确的选型边界声明**：XBTIT 在 README 里写清"≤10k peer 用 PHP tracker，其它一律 xbtt" | 我们文档也应写明 tracker 的容量边界与触发扩容的条件，避免拍脑袋 |
| 2 | ✅ **模板与代码分离**：XBTIT 98% HTML 移出 PHP 文件，早于时代 | 已免疫（Next.js），但**警惕前端重新长出页面级业务逻辑** |
| 3 | ❌ **hack 生态 = 鼓励 fork 核心** | 真要做二开，用**插件契约/模块化组件**，绝不用 patch 式 hack |
| 4 | ❌ **BrokenWings：重写半途人消失** | **不要开第二条运行时**。这点 U3D/NP 都在付出代价，我们必须警觉 |

### 5.6 来自 Discuz-PT / JS 系

| # | 教训 | 落到我们这 |
| :-- | :--- | :--- |
| 1 | **Discuz：论坛与 PT 两套积分打架** | 我们论坛独立模块是对的，**不要把论坛积分与火花打通到分不清主次**（打通可以，主从必须明确：火花为主） |
| 2 | **Discuz：announce 混在 PHP 进程，负载一上来就崩** | 已彻底解耦，保持 |
| 3 | **JS 系：MongoDB 弱事务 vs 计费强一致** | 我们的 PostgreSQL + 流水 + 行锁幂等是这个失败模式的正解，**不要为"灵活"引入最终一致的账本** |
| 4 | **JS 系：单维护者 = 单点故障** | 保持团队的文档化与接口稳定性，避免只有一个人懂经济系统 |

### 5.7 来自 Custom 大站（M-Team / HDBits）

| # | 借鉴点 | 落到我们这 |
| :-- | :--- | :--- |
| 1 | **同资源多版本聚合** | 与 GZ Group 结论一致且被商业站验证，教材聚合页优先级上调 |
| 2 | **App / 移动端一等公民** | 补 PWA + Web Push（新种/私信/审核结果），成本低收益大 |
| 3 | **统一风控中台** | 我们有 `cheaters` + `agent_rules`，补**信号汇聚**：异常速度、设备指纹、信用分联动 → 一个风控面板 |
| 4 | ⚠️ **反例：全封闭生态** | 保持开放 API + 兼容层，靠标准而非锁死 |

### 5.8 来自 Rust/Go 新一代

| # | 借鉴点 | 落到我们这 |
| :-- | :--- | :--- |
| 1 | **按 info_hash 分片无锁**（Aquatic） | 我们 DashMap 单表高 annonce 下有锁竞争；规模上来后按 info_hash 分 N shard |
| 2 | **完整 BEP 覆盖 + 可观测性**（Torrust） | 已有 `/metrics`，补：延迟直方图、peer 数、按 shard 指标、死信队列长度告警 |
| 3 | **IPv6（BEP-7）** | ⭐ **教育网刚需**。当前 `peers.rs` 明确"IPv6 简化版暂不进 compact 列表"，纯 v6 用户**拿不到 peer**。当前最实在的协议缺口 |
| 4 | UDP（BEP-15） | 私有 tracker 意义有限（passkey 无标准位置）；GZ 的 Ocelot 也是 TCP-only，优先级放低 |

---

## 6. 需要避免的缺点（带实证）

| # | 坑 | 出处与证据 | 我们怎么防 |
| :-- | :--- | :--- | :--- |
| 1 | **逐文件拼 SQL → 注入遍地** | NexusPHP 在 CVE 库有 **31 条**记录，其中 **CVSS 9.8 的 SQL 注入至少 6 条**：`takeconfirm.php`(classes/conusr)、`linksmanage.php`(id)、`nowarn.php`(usernw)、`cheaterbox.php`(delcheater)、`staffbox.php`(setanswered)、`forummanage.php`(sort)、`modrules.php`(id)、`reports.php`(delreport)；另有大量反射/存储 XSS 与 CSRF `[实证]` | **根因是"每个 .php 自己拼 SQL + 无统一鉴权中间件"，不是 PHP 的错**。我们 sqlx 参数化 + 类型系统已免疫一大类；盯住三处：① 前端 `dangerouslySetInnerHTML`（种子简介/NFO）② 文件下载路径拼接 ③ **新增管理接口的越权——新接口必须进回归脚本** |
| 2 | **20 年技术债** | GZ：PHP/MySQL 债、Sphinx 难运维、Ocelot（C++）无人敢改、PG 迁移"进行中"多年 `[业内]` | 保持迁移纪律：**只增不改、可回滚**；业务规则**不要硬编码进 Rust**，一律进 `site_settings` / `class_rules` / `agent_rules` |
| 3 | **双轨并存是最大技术债源** | NP 过程式 PHP + Laravel 并行 `[实证]` | 我们已是单一技术栈，**切忌为"快速上线某功能"引入第二套运行时** |
| 4 | **重写半途而废** | TBSource BrokenWings `[实证]`、JS 系集体回迁 `[业内]` | 任何"重构"必须以**可 incremental 交付**为前提，禁止 big-bang rewrite |
| 5 | **AGPL 传染性** | UNIT3D、Torrust 均为 AGPL-3.0 `[实证]` | 自研是优势；引入任何 AGPL 依赖前先确认许可证策略 |
| 6 | **性能陷阱** | 列表页 N+1、`count(*)` 深分页、announce 同步写库、users 表行锁 | 已用游标分页（对）；总数走缓存；**绝不为了"实时"把计费放回 announce 同步路径** |
| 7 | **经济系统通胀** | NP / U3D 共同病史 | 我们产出源更多（签到、做种、银行利息、考核、任务、娱乐玩法），**风险更大**。建议月度"产出—回收"对账，娱乐玩法保持 EV<1 并加产出上限 |
| 8 | **照抄规则忽略人群差异** | GZ/NP 的 H&R、同 IP 多账号检测在**校园网 NAT 出口**下大面积误伤 | 我们按 `user_id` 而非 IP 做 announce 限流是对的，**继续坚持** |
| 9 | **协议层无法根治作弊** | 从业者分析指出：上传量自报、假保种、缓存 Peer List 免费下载、甚至可刷他人上行把人 ban 掉，BT 协议层面几乎无解，只能靠统计 `[业内]` | 接受"反作弊是统计问题"这个现实，把预算投在**信号汇聚**（§5.7.3）而不是追求协议级根治 |
| 10 | **破坏性 API 变更毁生态** | YemaPT changelog 硬性删除旧 GET 接口 `[实证]` | 定规则：旧端点保留 ≥2 版本周期 + deprecation 公告 |

---

## 7. 落地优先级（本次重排，生态项大幅上调；✅ = 2026-09-13 已落地，见迁移 0069）

| 优先级 | 事项 | 来源 | 理由 | 状态 |
| :--- | :--- | :--- | :--- | :--- |
| **P0** | **`torrents.pieces_hash` 字段 + 上传时写入 + 存量回填** | YemaPT | 辅种/跨站识别的二级指纹，成本一次迁移，收益是把"可被识别"拉到接近 100% | ✅ 已落地 |
| **P0** | **IPv6 peer 进 compact 响应（BEP-7）** | 新一代 | 教育网（CERNET2）刚需，纯 v6 用户当前**完全拿不到 peer** | ✅ 已落地 |
| **P0** | **工具兼容层**（NP 口径）+ 公开 API 文档 | TNode / ptool | 决定用户是否愿意把刷流/辅种工具连过来 | ✅ 已落地（`/compat/nexusphp/*` + `/compat/meta` + openapi.json；向 ptool 提 adapter PR 待做） |
| **P0** | 客户端白名单（`agent_rules`）执行闭环 | GZ / NP | 反作弊基础设施，成本小收益大 | ✅ 已落地（命中 → `cheat_events` + 管理组信箱告警，1h 去重） |
| **P1** | **下载凭证与长期 API key 解耦**（30min 临时凭证） | YemaPT | 泄露损失窗口收敛；我们目前长期 Token 通吃 | ✅ 已落地 |
| **P1** | API key 时效（180d）+ 数量上限（≤3）+ 即时吊销 | YemaPT | 凭据治理 | ✅ 已落地（吊销此前已有） |
| **P1** | 教材/课程聚合层（Torrent Group 化） | GZ + Custom(MT) | 扁平种子的最大产品结构缺口 | ✅ 基础落地（`torrent_groups` + 挂入/查询端点 + 详情页「同组版本」；上传页表单联动待做） |
| **P1** | 对象级读缓存（种子详情/用户统计/首屏） | GZ | 撑住列表页与详情页读放大 | ✅ 首块落地（`/stats` 60s TTL；详情/列表待扩） |
| **P1** | 搜索升级预案（Meili 双写，pg_trgm 兜底） | U3D | 提前设计，等用户抱怨再改伤口碑 | ⏳ 预案，未做 |
| **P2** | 实时聊天盒 + 成就系统 | U3D | 留存与社区感 | 🟡 部分（有论坛/短讯） |
| **P2** | PWA + Web Push | Custom(MT App) | 移动端一等公民的低成本路径 | 🟡 有 manifest/SW |
| **P2** | FL 券 / Neutral Leech 券 + 做种收益饱和封顶 | GZ + NP | 经济系统精细化、抗刷 | 🟡 部分（有 promotions） |
| **P2** | 个人化 RSS / 官方 RSS 桥 | BTN | 比全站 RSS 更高订阅价值 | 🟡 有全站 RSS |
| **P2** | 一键升级脚本 + 备份恢复演练 | U3D | 上线后运维刚需 | 🟡 部分（有 Compose + 部署指南） |
| **P3** | Tracker 按 info_hash 分片无锁 + 细粒度指标 | Aquatic/Torrust | 下一阶段性能目标 | 🔴 |
| **P3** | UDP announce（BEP-15） | Torrust | 私有站收益有限，可后置 | 🔴 |

---

## 8. 一句话总结

> **抄 TNode/YemaPT 的「主动适配生态」姿势，抄 GZ 的内容聚合与缓存纪律，抄 NP 的工具口径与规则颗粒度，抄 U3D 的社区组件与交付工程化，抄 Custom 大站的移动端一等公民；
> 不要抄 TBSource 的重写冒险，不要抄 XBTIT 的 hack 生态，不要抄 NP 的逐文件拼 SQL 与同步写库，不要抄 U3D 的 Livewire 耦合与组件膨胀，不要抄 Discuz 的双积分别扭，不要抄 JS 系的弱事务记账，不要抄 Custom 的全封闭。**

我们的护城河是：**教育垂直域模型 + 流水式计费架构 + 类型安全 + 极简运维组件**。
我们的致命风险**不是功能不够**（113 张表 / 360 路由 / 53 页，功能密度已经很高），而是：**用户带着工具进来发现一个都连不上**（生态适配），以及 **教育网纯 IPv6 用户根本连不到 peer**（BEP-7）。这两件事都不需要重架构，只需要做对几个小东西。

---

*整理：2026-09-13（v2）· 基于上游官方仓库 README / 官方 API Wiki / MITRE CVE 库 / ptool 官方 README，以及本仓库代码快照*
