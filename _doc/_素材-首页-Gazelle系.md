# 素材：Gazelle 系首页（子代理一手调研，2026-09-22）

> 代码基线：OPSnet/Gazelle @ master（现代 Twig 重写版，PHP `Gazelle` 命名空间 + PostgreSQL）。
> GGn fork 仓库 `Gazelle-Games/Gazelle` 已删除（GitHub/GitLab 均 404），其形态只能 [业内] 标注。

## 0. 首页入口与路由 [实证]

- `sections/index/index.php` — 极薄路由：
  - 未登录 → `require __DIR__ . '/public.php'`
  - 已登录无 action → `require __DIR__ . '/private.php'`
  - `action=poll` → `sections/forums/poll_vote.php`（首页投票 AJAX 提交端点），其余 action 返回 400
- `sections/index/public.php` — 若 `SHOW_PUBLIC_INDEX` 关闭则 302 到 `login.php`；否则渲染 `templates/index/public.twig`，仅传一个 `new` 布尔（站点 0 用户=新装站）。

## 1. 首页区块清单（登录态）

数据装配全部在 [实证] `sections/index/private.php`（两个模板分别渲染侧栏与主栏）：

### 主栏 `templates/index/private-main.twig`

| 区块 | 内容 | 数据来源/缓存 |
|---|---|---|
| Contest banner | 横幅图+名称+倒计时 | `Manager\Contest::currentContest()` |
| **Latest Uploads** | 封面墙（见 §2） | `Manager\Torrent::latestUploads(5)`，缓存键 `latest_up_%d`，TTL 3600s；上传新种时在 `create()` 里主动失效 limit=5 的键 [实证 app/Manager/Torrent.php] |
| News | 最新 5 条全文框（`news[0:5]`），标题+时间+BBCode 正文，可 Hide/Show 折叠；管理员见 Edit 链接 | `Manager\News::headlines()`，缓存键 `newsv2`，**TTL 0（永不过期）**，create/modify 时 `delete_multi(['feed_news','newsv2'])` [实证 app/Manager/News.php] |
| Load more news | AJAX `news_ajax(3, admin)` 点击加载更多；旧新闻归档指向 forumid=12 | [实证 private-main.twig] |

### 侧栏 `templates/index/private-sidebar.twig`

| 区块 | 内容 | 数据来源/缓存 |
|---|---|---|
| Album of the Month | 大封面图 + [Discuss] 论坛帖链接 | `Manager\FeaturedAlbum::findByType(AlbumOfTheMonth)` |
| Showcase | 同上（注：模板里链接误用 `aotm.tgroup.link`，疑似 bug） | `findByType(Showcase)` |
| Staff blog（仅 `users_mod`） | 最新 5 条标题，未读加粗（对比 `staff_blog_read_%d` witness） | `Manager\StaffBlog::blogList()`，键 `sblog`，TTL 1,209,600s（14 天） [实证 app/Manager/StaffBlog.php] |
| Blog | 最新 5 条标题链接 → blog.php | `Manager\Blog::headlines()`，键 `blogv2`，写时失效；单篇 ID 键 `zz_blog_%d` TTL 7200 [实证 app/Manager/Blog.php] |
| Contest Leaderboard | Top 3 表格（排名/用户/条数）；若自己排名>3 追加自己一行；结束 15 天后整块隐藏 | `$contest->leaderboard(CONTEST_ENTRIES_PER_PAGE)` 截取前 3 [实证 private.php] |
| Latest Poll 🗳 | 未投票=单选表单（含 Blank/Show results）+ AJAX 投票；已投票=各选项百分比条形图+总票数，自己选项标 ➔ | `Manager\ForumPoll::findByFeaturedPoll()`，键 `polls_featured` TTL 7 天；threadId→poll 键 TTL 7200 [实证 app/Manager/ForumPoll.php] |
| Stats | 启用用户数/今日·本周·本月活跃（含百分比）/Torrents/Releases/Artists/"Perfect" FLACs/Collages/Requests（填充率）/Snatches/Peers/Seeders/Leechers/做种比 | `Stats\Torrent`（键 `stat_global_torrent` 等）、`Stats\Request`（键 `stats_req`，TTL 3600+随机300 防雪崩）、`Stats\Users`（多键，86400s 级） [实证 app/Stats/*] |

### 匿名首页 `templates/index/public.twig`

- [实证] 标题 "This is a mirage"，一首莎士比亚诗歌（Orpheus 弹琴典故）+ Enter/Register/Referral/Recovery 动作条。**无统计、无新闻泄露**——闭源站常见的"极简门面"策略。

## 2. 「最新种子」区块形态

[实证] `templates/index/private-main.twig` + `app/Manager/Torrent::latestUploads(5)`：

- **形态：`<ul class="collage_images">` 封面缩略图墙，不是表格**。每项 118px 宽封面图，`tooltip_interactive` 悬停提示（名称/标签列表/上传者/时间），`image_cache(width=150)` 走图片代理。
- 行数：5，且 **按 GroupID 去重**（5 个不同 release group）。
- 过滤条件（音乐站特化）：近 3 天、`Encoding IN ('Lossless','24bit Lossless')`、WikiImage 必须是 http(s) URL、上传者账号启用、排除 HIDDEN_TAGS 标签、尊重上传者 paranoia（'uploads' 则跳过）；查询失败有两次重试。
- 结论：**这是"精选封面墙"而非信息密集表格**。

## 3. Top10 区块

注意：top10 在 OPS 是**独立页面** `top10.php`，不在首页渲染；旧版 Gazelle（What.CD 血统）曾有首页嵌入版 [业内]。

- [实证] `sections/top10/index.php`：整体需权限 `site_top10`（无权限返回 403），按 `type` 分发 donors/history/lastfm/tags/users/votes→默认 torrents。
- [实证] `sections/top10/torrents.php`：Day/Week/Month/Year/Overall "Most Active" + Most Snatched 共 6 榜；`details=all` 时全部同显。列：排名/分类/名称/（可选文件数）/大小/Snatches/Seeders/Leechers/Transferred。默认 10 行，可选 10/100/250；支持 tags/format/freeleech/groups 等过滤参数。
- [实证] `sections/top10/users.php`：7 榜 — Uploaders、Downloaders、Torrents Uploaded、Request Votes、Request Fills、**Fastest Uploaders（上传速度）**、**Fastest Downloaders（下载速度）**；默认 10 行；无 ratio/seedsize 榜。
- [实证] `app/Top10/Torrent.php`：结果缓存 TTL 6 小时 + 独立 `_lock` 锁键（3600s）防缓存击穿。

## 4. 公告 / 博客 / 新闻分工

- [实证] **无滚动 ticker**（`private-header.twig` 已全文核对；老式 Gazelle 的 `script_start` ticker 在 OPS 重写中已移除 [业内]）。
- 分工：**News = 首页主栏头版**（全文展示+折叠+AJAX 分页，创建时自动开论坛讨论帖并附链接，见 `Manager\News::create()` 把 `[url=/forums.php...][/url]` 拼进正文）；**Blog = 侧栏标题列表**（点击去 blog.php）；**Staff Blog = 仅员工**。
- 已读机制：`WitnessTable\UserReadNews` — private.php 次访问把用户 lastRead 推到 latestId（"未读"标记生命周期很短）。

## 5. 闭源 fork 首页业内形态 [业内]

- **RED (Redacted)**：与 OPS 同源（同代 Gazelle），首页布局基本一致 — 主栏新闻 + Latest Uploads 封面墙、侧栏 AOTM/Showcase/博客/投票/统计。无公开截图可引用 [未获取]。
- **OPS (Orpheus)**：即本仓的直接部署，上文全部 [实证] 即其实际形态。
- **BTN (Broadcasthe.net)**：Gazelle 变体，首页以新闻为主，侧栏统计 + "Last 10 uploads"（剧集封面）。
- **PTP (PassThePopcorn)**：Gazelle 变体，首页主栏新闻+最新上传封面，侧栏 Featured（主页推荐电影）/投票/统计。
- **MAM (MyAnonamouse)**：深度改造，首页大量自定义区块（签到、免费种子、推荐书目）。
- **GGn (Gazelle-Games)**：仓库已删；业内已知其首页保留 Gazelle 骨架（新闻/博客/投票/统计），Latest Uploads 换成游戏封面墙。[未获取] 原文核实。
- 共同特征 [业内]：这些站均为"登录墙"——匿名首页只有 logo/登录框，不泄露任何统计。

## 6. 首页个人化（per-user）

- [实证] 投票状态（已投显示结果图+自己选项箭头）、竞赛自己排名行、staff blog 未读加粗、news witness 已读表 — 即此三项。
- [实证] **首页无订阅/收藏/通知流**。订阅在独立页 `userhistory.php?action=subscriptions`；通知靠 `user_notifications.js` + noty 弹条；RSS 个人过滤 `torrents_notify_{id}_{announceKey}` 只出现在 `<head>`。老版 Gazelle 首页侧栏曾有 "Notifications" 盒 [业内]，OPS 重写已去掉。

## 7. 优劣点评（作为首页设计参考）

**优势**
- 双栏经典布局信息密度合理：主栏内容流（上传→新闻），侧栏状态流（推荐→博客→竞赛→投票→统计），扫视路径清晰。
- 缓存体系成熟：几乎每块独立缓存键+写时失效；Request 统计 TTL 加随机抖动、Top10 有锁键，是防雪崩/击穿的好范式（值得抄）。
- 匿名面零信息泄露；paranoia（HIDDEN_TAGS、uploader paranoia）贯彻到首页封面墙。
- 投票/新闻 AJAX 局部刷新，交互轻。

**劣势**
- "Latest Uploads" 只有 5 张封面、且强过滤（仅无损+有封面+3 天内），**信息量低且对非音乐内容类型不友好**（GGn/PTP 类站点需重写）。
- 首页个人化薄弱：无个人做种/下载/订阅/收藏动态；通知靠弹条而非首页区块。
- 无 ticker/紧急公告机制——突发事件只能靠置顶新闻。
- `newsv2` 缓存 TTL=0 依赖写失效，直接改库会造成永久脏缓存；showcase 链接 bug 等小瑕疵，模板耦合度仍偏高。
- top10 不在首页，想要首页 Top10 盒子（老 Gazelle 有 `.top10` 嵌入）需自行移植。
