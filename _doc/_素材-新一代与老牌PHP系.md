> 素材文件：由调研子代理产出，供《种子页面横向深度对比》主文档引用。

# 种子列表/详情页横向竞品调研（新一代/非 PHP 系 + 老牌非 Gazelle 系）

> 调研范围：Torrust、Arcadia、NyaaPantsu、老牌 PHP 系（XBTIT/BTiT、U-232、TBDev/TBSource、TorrentTrader）、meanTorrent、rartracker/sqtracker，及未能定位的 YemaPT/TNode。结论聚焦「与 NexusPHP(NP)/UNIT3D 的功能密度差距」。

## Torrust Index（github.com/torrust/torrust-index-gui + torrust-index）
**技术栈与现状**：后端 Rust（Axum），前端 Nuxt + Vue 3（即 `torrust-index-gui`），API/前端分离。定位是「现代重写的 BitTorrent 索引站」，支持公私两种模式。 `[实证]`

**列表页能力**：`pages/torrents.vue` 为入口，渲染 `TorrustList`（卡片）或 `TorrentTable`（表格），由顶部 `Default / Table` 布局切换按钮控制（**两种视图模式**）。 `[实证-torrents.vue]` 筛选维度：搜索框（`search`）、`TorrustSelect` 多选用分类筛选（`categoryFilters`）、`TorrustSelect` 多选用标签筛选（`tagFilters`，带搜索）——**注意：标签筛选在 UI 中确实存在，复用通用下拉组件**；排序下拉 `sortingOptions` 在 UI 仅暴露 4 项：`Uploaded (Newest first)`、`Uploaded (Oldest first)`、`Seeders (High to low)`、`Leechers (High to low)`（API 文档另列 `name/size` 方向，但 UI 未开放）。 `[实证-torrents.vue]` 分页由 `Pagination.vue` 提供，每页条数 `<select>` 提供 **20 / 50 / 100** 三档，用户可切换，默认 50（`defaultPageSize = 50`，并写入 URL `?pageSize=`）。 `[实证-Pagination.vue + torrents.vue]`

**详情页能力**：`[infoHash].vue` 下由一组 Tab 组件构成——`TorrentDescriptionTab`（描述，支持 Markdown 内嵌 PNG 图片，经后端代理+配额）、`TorrentFilesTab`（文件列表）、`TorrentTrackersTab`（trackers）、`TorrentCommentTab`、`TorrentCreatedByTab`（创建者）、`TorrentCreationDateTab`、`TorrentEncodingTab`、`CanonicalInfoHashGroup`（多 info_hash 格式归组，现代亮点）、`TorrentActionCard`（下载 .torrent / 生成带个人 announce 的下载）。种子/下载数（seeders/leechers）在 API 中返回。 `[实证]`。**关键缺失**：API 的 torrent 对象字段为 `title/description/category_id/file_size/seeders/leechers/files/info_hash/uploader`，**没有 poster/cover 字段**，也没有 NFO、MediaInfo、截图独立区块——图片只能靠描述里手贴 Markdown 链接（且仅对登录用户代理，有带宽配额）。 `[实证-API schema + discussion #519]`

**判定**：**弱（功能密度低）**。它是「现代重写派」里最薄的一个：干净的 SPA 外壳与不错的列表交互（Tag 筛选/视图切换/可配每页条数），但详情页能力明显弱于 NP/U3D，甚至不如 2010 年的 U-232。它的价值在架构（Rust 性能、API 化），不在页面富度。

## Arcadia（github.com/Arcadia-Solutions/arcadia）
**技术栈与现状**：后端 Rust + actix + sqlx + PostgreSQL，前端 TypeScript + Vue 3 + PrimeVue（客户端渲染 SPA），现已自带 Rust tracker（早期计划复用外部 tracker）。明确对标 Gazelle/UNIT3D，「content-agnostic」多内容类型框架。 `[实证-官方文档/README]`

**列表页/详情页能力**：文档确认已有「torrent search（种子搜索页）」「user profile」，以及 `master groups / title groups / edition groups / torrents` 四级分组、`Collections` 聚合、`series/authors`。元数据模型极丰富：电影（长片/短片）、剧集（季/集跟踪）、音乐（专辑/EP/单曲/原声/混音等）、书籍（含 manga/comics/visual novel）、软件、游戏、期刊。强调 **image and icons first**（图片优先，暗示海报/封面是原生概念，区别于 Torrust）。还提供「Visual layout configuration」可配置信息密度。 `[实证]`。论坛、审核、发种/下载/做种已可用，但论坛/审核标注 WIP。 `[实证-progress report]`

**判定**：**中—强（数据模型强，UI 完整度 WIP）**。在「元数据组织 + 分组 + 海报优先」上是最接近 Gazelle/UNIT3D 野心的现代项目，但仍在早期，评论/审核等页面细节尚未坐实。

## NyaaPantsu / Nyaa 系（github.com/NyaaPantsu/nyaa，Go）
**技术栈与现状**：Go 重写 Nyaa.se（MIT，2017 年后基本停更），模板在 `templates/`，逻辑在 `controllers/`，公开站。 `[实证-repo 结构]`

**列表页能力**：分类下拉（动漫/漫画/软件等 + 子分类如「English-translated」）、搜索（支持正则/排除运算符）、按分类与排序（种子数等）、可按用户筛选。 `[实证-官方 help + repo]`
**详情页能力**：文件列表、描述、`.torrent` 下载、磁力链接、评论（Nyaa 原生有评论区）、Trusted/Remake 行染色。 `[业内/实证]`

**判定**：**中（作为公开索引合格，但天然缺失 PT 能力）**。**明确缺失的 PT 元素**：无促销/免费种(freeleech)、无做种统计与魔力/积分、无用户等级/邀请、无比例(ratio)强制、无 HnR、无截图/NFO/海报体系——因为它是公开站，这些 PT 特有维度从设计上就不存在。 `[推断/业内]`

## XBTIT / BTiT（github.com/KaRpIkS/xbtit 等）
**技术栈与现状**：PHP + bTemplate 模板系统；`torrents.php`（浏览列表）、`details.php`（详情）、`comment.php`、`peers.php`、`thumbnail.php`、`thanks.php`。 `[实证-repo 文件树]`
**列表/详情页能力**：分类浏览、详情页含评论、peers 列表、截图/缩略图（`thumbnail.php` 证明 2010 前后已有图片预览）、thanks（感谢/赞）。 `[实证]`
**判定**：**中（年代功能已全，UX 旧）**。具备截图与评论，但非 SPA、无自动元数据。

## TBDev / TBSource（含 bytemonsoon  lineage）
**技术栈与现状**：2005–2010 系 PHP Tracker 鼻祖；U-232、多数老 PHP 站均声明「based on TBDev.net / tbsource / bytemonsoon」。 `[实证-U-232 源码注释]` 本身源码未在本次抓取，能力以下游 U-232 推断。
**列表/详情页能力（推断）**：分类、描述、NFO、filelist、peers、评论、评分、用户等级、比例统计；具体字段以 U-232 实证为准（见下）。 `[推断/业内]`
**判定**：**中**。作为 lineage  baseline，功能清单与 U-232 同源。

## U-232（github.com/StNr/U-232-V3、Bigjoos/U-232-V4）
**技术栈与现状**：PHP Tracker，源自 TBDev/TBSource/bytemonsoon，含 Memcache、IMDB 类、字幕、freeleech slots。 `[实证-repo 文件树 + details.php 注释]`
**列表/详情页能力**：`browse.php` 浏览，`details.php` 详情，`filelist.php`、`peerlist.php`、`viewnfo.php`、`report.php`、`multidetails.php`、`subtitles.php`、`uploadsub.php`、`thanks.php` 等一系列配套页。

**U-232 `details.php` 实际暴露的字段（逐条，来自 `SELECT` 语句）** `[实证-U-232-V3 details.php 源码]`：
- `seeders` / `leechers`：做种/下载数
- `banned`：是否被封
- `thanks`：感谢数
- `info_hash`：种子 hash
- `checked_by`：审核人
- `filename`：文件名
- `search_text`：搜索文本
- `LENGTH(nfo) AS nfosz`：**NFO 存在性/长度**
- `name`：标题
- `comments`：评论数
- `owner`：发布者
- `save_as`：保存名
- `visible`：可见性
- `size`：大小
- `added`：添加时间
- `views` / `hits`：浏览/点击
- `id` / `type`：主键/类型
- `poster`：**海报**
- `url`：外部链接
- `numfiles`：文件数
- `times_completed`：完成次数
- `anonymous`：匿名发布
- `points`：积分/评分点
- `allow_comments`：是否允许评论
- `description`：描述
- `nuked` / `nukereason`：Nuked 及原因
- `last_reseed`：最近求种时间
- `vip`：VIP 属性
- `category`：分类
- `subs`：**字幕**
- `username`：用户名
- `newgenre`：类型/流派
- `release_group`：发布组
- `free`：**免费种(freeleech)**
- `youtube`：预告片
- `tags`：**标签**
- `rating_sum` / `num_ratings`：**评分汇总/次数**

**判定**：**中（年代功能已相当全，但 UX 停留在 2010）**。反直觉结论：U-232 2010 前后**已具备海报、NFO、截图、字幕、评分、举报、peer 列表、标签、免费种、YouTube 预告**——Torrust 反而没这些。与 NP/U3D 的差距不在「功能有没有」，而在现代 UX/工程化。

## TorrentTrader
**技术栈与现状**：2005–2012 系 PHP Tracker（TorrentTrader v2/v3 一脉），与 TBDev/XBTIT 同代。 `[业内]`
**列表/详情页能力**：未抓到明确源码；按同期 PHP 站推断含 `browse`/`details`、分类、描述、NFO、文件列表、peers、评论。 `[推断]`
**判定**：**中（推断，未获取到源码细节）**。 `[未获取到-源码]`

## meanTorrent（github.com/WXpiero/meanTorrent，MEAN.js）
**技术栈与现状**：MongoDB + Express + AngularJS + Node.js，全栈 JS 私 Tracker CMS。 `[实证-README]`
**列表/详情页能力**：发种时填 TMDB ID **自动拉取电影元数据**（海报/简介），种子可绑多个 attr **tag 用于搜索筛选**，评论全 Markdown，上传者/管理员可**上传编辑截图**，独立**字幕面板**，**电影合集(collections)**，VIP 属性种，资源组(resources group)，HnR、积分/邀请、IRC 公告、论坛、聊天、请求系统、RSS、多语言。 `[实证-README 功能列表]`
**判定**：**强（2017 即接近 NP/U3D 密度）**。海报(TMDB)、截图、字幕、合集、标签、请求系统一应俱全；唯一短板是 AngularJS 技术栈偏老。功能密度实际与早期 NP 持平。

## 现代小众：rartracker / sqtracker
- **rartracker**（github.com/Xirg/rartracker）：AngularJS + PHP 的 SPA，**响应式 + Bootstrap 移动友好**，含 bonus/leeches bonus/种时/请求系统/RSS/多语言。 `[实证-README]` 判定：**中—强（UX 现代，但 PHP 后端、功能偏 scene 站）**。
- **sqtracker**（github.com/tdjsnelling/sqtracker）：Node.js + React + MongoDB + Docker 的「现代私 Tracker 平台」，截图含 Home/Torrent/Upload/Categories/Profile。 `[实证-README/截图]` 判定：**中（现代技术栈，具体种子页字段未深入披露）**。

## 未能获取：YemaPT（野马PT）/ TNode
- **YemaPT（野马PT）**：按「前端 SPA + Ant Design + 公开 API wiki + 瀑布流海报墙/批量下载」等描述多次检索，**未找到可核实的公开 GitHub 仓库或官方 wiki 源码**，疑为闭源/社群内部项目。其种子页具体形态**未获取到**，不编造。 `[未获取到]`
- **TNode**：检索无确定对应开源仓库，**未获取到**。 `[未获取到]`

## 核心结论：与 NexusPHP / UNIT3D 的差距有多大？
**结论：差距巨大且集中在 4 个维度，但「新一代重写派」内部分化严重。**

1. **海报/媒体资产体系**（差距最大）：NP/U3D 有原生海报墙、截图画廊、MediaInfo/NFO 解析、TMDB/IMDB 自动元数据。Torrust **完全没有 poster 字段**（仅靠描述内嵌图）；老 PHP（U-232）虽有 poster/NFO/截图/字幕但非自动富媒体；Arcadia「image-first」设计上对齐，meanTorrent 凭 TMDB 对齐。
2. **元数据组织与分组**：NP/U3D 的组别/合集/请求/字幕/多语言已是标配。Arcadia 用 master/title/edition 分组在模型层反超 Torrust；老 PHP 靠 `release_group/newgenre/subs` 勉强够用；NyaaPantsu 作为公开站天然无此层。
3. **PT 经济与治理**：免费种、魔力/积分、HnR、邀请、用户等级、举报/审核流——这是 NP/U3D 的护城河。**仅 meanTorrent、老 PHP、rartracker 有部分**；Torrust 几乎没有（只有基础上传/下载计数）；NyaaPantsu 因公开站天然缺失。
4. **现代 UX/工程化**：SPA、响应式、信息密度可配置、API 化。Torrust/Arcadia/sqtracker/meanTorrent/rartracker 胜出；老 PHP 与 NyaaPantsu 模板式页面最弱。

**一句话判断**：「现代重写派」并非都强——Torrust 是「架构现代、页面极简」（列表交互尚可：Tag 筛选/视图切换/20-50-100 每页可配，但详情页空），实际种子页能力**弱于** NP/U3D 且不如 2010 的 U-232；真正在页面功能密度上接近 NP/U3D 的是 **Arcadia（模型层）与 meanTorrent（功能层）**。老牌 PHP 系的教训是：它们早就有海报/NFO/截图/评分，缺的只是现代外壳；而 Torrust 恰恰反过来——外壳新、内涵空。

---
**证据可靠性说明**：所有 `[实证]` 均来自本次 WebFetch 的 GitHub 仓库文件树/源码/官方 API 文档或 README；`[业内]` 为 PT 圈共识性描述；`[推断]` 为基于组件/字段缺失的合理推测；`[未获取到]` 为检索无果、未做任何虚构。
