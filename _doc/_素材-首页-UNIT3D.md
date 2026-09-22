# 素材：UNIT3D 首页（子代理一手调研，2026-09-22）

> 仓库：`HDInnovations/UNIT3D`（由 HD-Innovations/UNIT3D-Community-Edition 更名），master 分支，Laravel 12 + Livewire 3 + Alpine.js。原地址已 404。

## 0. 总体架构 [实证]

首页是**「用户可配置区块(blocks)」拼装式仪表盘**，而非固定模板：

- 控制器把 11 个区块的 `visible` + `position` 读自用户设置表，动态排序、过滤后传入视图 [实证] `app/Http/Controllers/HomeController.php`
- 视图仅是 `@foreach($blocks)` + `@switch($block)` 循环，逐个 `@include('blocks.xxx')` 或 `@livewire(...)` [实证] `resources/views/home/index.blade.php`
- 区块默认值（全部可见、固定顺序 0~10）定义在 User 模型 `settings()` HasOne `withDefault()` [实证] `app/Models/User.php` L305-345：`news_block_visible/position`、`chat_`、`featured_`、`random_media_`、`poll_`、`top_torrents_`、`top_users_`、`latest_topics_`、`latest_posts_`、`latest_comments_`、`online_` 共 11 组

**路由与访问控制** [实证] `routes/web.php` L90：`/` 只对已登录、已验证、未封禁用户开放。**没有公开 landing page**——UNIT3D 是「登录墙内 dashboard」模式，与 NexusPHP 默认公开首页截然不同。

## 1. 首页区块清单（默认顺序）[实证]

| # | 区块 | 实现方式 | 数据来源 / 缓存 | 可配置 |
|---|------|---------|----------------|--------|
| 0 | news 新闻 | blade include `blocks.news` | `Article::latest()->limit(3)` + `unreads_exists`（未读标记），**无缓存** | 用户级开关+排序 |
| 1 | chat 聊天 | include `blocks.chat` + `@vite('resources/js/unit3d/chat.js')` | 用户/聊天室模型，前端 JS | 用户级 |
| 2 | featured 精选种子 | include `blocks.featured` | `cache()->flexible('latest_featured', [5min,10min])` → `FeaturedTorrent` 全部（带 torrent→resolution/type/category/user.group） | 用户级 |
| 3 | random_media 随机媒体 | `@livewire('random-media')` | Redis `SRANDMEMBER` 预建集合（区分含/不含成人），取 3 部电影+3 部剧 `TmdbMovie/TmdbTv`（仅 backdrop、标题、日期） | 用户级 |
| 4 | poll 投票 | include `blocks.poll` | `cache()->flexible('latest_poll', [5,10])` → 最新未过期 `Poll` | 用户级；未投过票才显示 |
| 5 | top_torrents 种子榜 | `@livewire('top-torrents')` | 实时查询 Torrent（见 §2） | 用户级 |
| 6 | top_users 用户榜 | `@livewire('top-users')` | `cache()->flexible('top-users:*', [3600,7200])`，每榜 `take(8)` | 用户级 |
| 7 | latest_topics 最新主题 | include `blocks.latest-topics` | `Topic::authorized(canReadTopic:true)->latest()->take(5)`，无缓存 | 用户级 |
| 8 | latest_posts 最新回复 | include `blocks.latest-posts` | `cache()->flexible('latest_posts:by-group:{gid}')` → `Post` take(5)，带点赞/点踩计数、BON 小费统计 | 用户级 |
| 9 | latest_comments 最新评论 | include `blocks.latest-comments` | `cache()->flexible('latest_comments')` → `Comment`（仅 Torrent/TorrentRequest 的 morph 评论）take(5) | 用户级 |
| 10 | online 在线用户 | include `blocks.online` | `cache()->flexible('online_users:by-group:{gid}', [5,10])` → `last_action > now()-60min` 的 User + Group 调色板 | 用户级 |

要点：**没有站点级的"首页区块开关"**（config 层面），完全下放给每个用户自己的设置页；管理员只能通过 Articles/Featured/Poll 后台数据间接控制内容。

## 2. 「最新种子」区块（TopTorrents）[实证]

`app/Http/Livewire/TopTorrents.php` + `resources/views/livewire/top-torrents.blade.php`：

- **5 个 tab**（`@entangle('tab').live`，点击即 Livewire 重查）：`newest`（最新，`orderByDesc('id')`）、`seeded`（做种最多）、`leeched`（下载最多）、`dying`（濒死：`seeders=1 且 times_completed>=1`）、`dead`（死亡：`seeders=0`）
- **数量：每个 tab 固定 `take(5)`，不可配置** [实证]
- **海报：有，但取决于用户设置 `show_poster`（默认 false）**。开启后每行左侧显示海报：电影/剧用 TMDB poster_small，游戏用 IGDB cover，音乐用占位图，no-meta 分类用本地上传封面 `torrent-cover_{id}.jpg`；均 `loading="lazy"` + hover 弹出 meta 卡片 [实证] `resources/views/components/torrent/row.blade.php`
- 每行字段：分类图标、分辨率+类型、种子名、上传者（支持匿名）、操作按钮（编辑/收藏/下载）、`withExists` 一次查出的用户态（已 featured/已收藏/有免费令牌/正在做种/正在下载/已完成）；另带 `comments_count` 和 `personal_freeleech`（读 cache key `personal_freeleech:{uid}`）
- **不按分类分组**，单列表格按 tab 切换；SQL `CASE WHEN category_id ... END AS meta` 打 meta 标签区分 movie/tv/game/music/no
- 成人内容过滤：`show_adult_content === false` 时排除 TMDB adult 条目

## 3. 公告/新闻系统 [实证]

- 模型是 **`Article`**（不叫 News；Staff 后台发布），首页取最新 3 条
- 形态：**可折叠面板**（Alpine `x-data={show}`），若任一条有当前用户未读记录（`ArticleUnread`），默认展开并加通知动画图标 [实证] `resources/views/blocks/news.blade.php`
- 每条预览：标题链接、`diffForHumans()` 时间、封面图（认证路由防盗链）、内容摘要（BBCode 剥除 + `Str::limit(500)`）、read-more。**不是轮播、不是弹窗，是折叠列表**

## 4. 投票与 Shoutbox [实证]

- **Poll**：仅当存在未过期投票且当前用户**未投过**时才渲染投票表单（投过整个区块消失）；缓存 5/10 分钟
- **Chatbox**：`resources/js/unit3d/chat.js` 全文仅 14 行：**Laravel Echo + socket.io-client over WebSocket(wss)**，自托管 socket.io 服务器；UI 用 Alpine `x-data="chatbox(...)"` 组件化，支持多房间、私聊、系统 bot、全屏模式、铃铛提醒。**不是 Livewire 轮询、不是 Pusher 商用服务**

## 5. 个性化 [实证]

- **无 landing vs dashboard 双形态**——`/` 本身就是登录后仪表盘
- 登录用户间差异：① 11 个区块独立开关+拖拽排序（存 `user_settings` 表）；② `show_poster` 控制种子行海报；③ `show_adult_content` 影响随机媒体与种子榜；④ 论坛区块走 `authorized(canReadTopic:true)` 按组权限过滤，缓存 key 按 `by-group:{group_id}` 隔离；⑤ poll 按是否已投消失；⑥ 在线列表尊重 `UserPrivacy::show_online` 匿名化

## 6. 缓存与格式化 [实证]

- 缓存统一用 **`cache()->flexible($key, [5min,10min] 或 [3600,7200], fn)`**（5 分钟内必命中，5-10 分钟间后台异步重算，10 分钟强制重算）——既保证新鲜又避免缓存击穿
- Key 一览：`online_users:by-group:{gid}`、`user-groups`、`latest_posts:by-group:{gid}`、`latest_comments`、`latest_featured`、`latest_poll`、`top-users:{...}`（1-2h）、`personal_freeleech:{uid}`、`user-settings:by-user-id:{uid}`（rememberForever）
- 不缓存：articles（含 per-user 未读态）、topics（含 per-user 已读态）、top-torrents（含 per-user withExists 态）——**凡是带用户态的查询都刻意绕开共享缓存**，这是其缓存设计的关键取舍
- 数字格式化：`StringHelper::formatBytes()`、`timeElapsed()`、`Number::ordinal()`、`diffForHumans()` [实证] top-users.blade.php

## 7. 优劣势点评（作为首页设计参考）

**优势**
- 区块化 + 用户级开关/排序是社区站标杆设计：信息密度高（11 块覆盖内容+社交+统计）但用户可自主裁剪
- featured 区块做成 Alpine 驱动的横向自动轮播（5s、hover 暂停、箭头循环滚动），配 TMDB 海报卡 `<x-torrent.card>`，视觉冲击力强、实现极轻（纯 scrollLeft，无轮播库）[实证] blocks/featured.blade.php
- 性能讲究：flexible 缓存 + 按组隔离 key + per-user 数据不走共享缓存；图片 lazy；头像/封面走认证路由防盗链
- 权限/隐私细节到位：论坛按组过滤、在线列表匿名化、adult 内容开关、排行榜剔除系统账号与 banned/pruned 组

**劣势**
- **无游客首页**：对开放式注册或想做宣传页的站点不友好
- 区块数量硬编码（11 个）且无站点级全局配置；新增区块要改 Controller + User 默认值 + blade 三处
- top-torrents 实时查询 + 8 个 withExists 子查询，大站首页可能有压力；`take(5)`/`take(8)`/`limit(3)` 数量全部硬编码，管理员不可调
- 无全站统计区块（在别的 stats 页），首页信息偏"动态流"；freeleech 等全局活动没有首页倒计时横幅（仅 featured 有 7 天到期时间）
- 依赖自托管 socket.io 服务，部署复杂度高于纯 HTTP 轮询方案

## 关键文件索引

均相对仓库根：`resources/views/home/index.blade.php`、`app/Http/Controllers/HomeController.php`、`resources/views/blocks/{news,chat,featured,poll,latest-topics,latest-posts,latest-comments,online}.blade.php`、`app/Http/Livewire/{TopTorrents,TopUsers,RandomMedia}.php`、`resources/views/components/torrent/row.blade.php`、`resources/js/unit3d/chat.js`、`app/Models/User.php` L305-370、`routes/web.php` L90
