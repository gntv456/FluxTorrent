# 素材：华语 NP 站 + 国际闭源大站首页（子代理调研，2026-09-22）

> 证据源：本地 `.np-research/np` = xiaomlove/nexusphp 1.10.3 全量 clone（NP 主线 [实证]）；
> `UI参考/real_pages/index.php.html` = "好学"教育站首页完整快照（[实证-本地快照]，gntv/观众系同源模板）；
> `UI参考/baozi_live/index.html` = baozi 站首页快照（[实证-本地快照]）；
> PT-Plugin-Plus `resource/sites/*/config.json`（[实证]）；sagan/ptool `site/` 适配器（[实证]）；
> M-Team 新版 SPA 逆向脚本（`zxszx/mteam-scroll-optimizer` 等，[实证-适配代码反推]）。

## A0. NexusPHP 主线默认首页（1.10.3）[实证]

文件：`public/index.php`（666 行）。自上而下区块：

| 顺序 | 区块 | 开关（`include/config.php` `$MAIN[...]`） | $Cache 缓存时长 |
|---|---|---|---|
| 1 | Recent News 最近消息（news 表，klappe 折叠，首条展开） | 恒显 | `recent_news` 86400s（整页缓存） |
| 2 | Funbox 趣味盒（fun 表，iframe fun.php，有趣/无聊投票） | `showfunbox` | 内容 1043s / 票数 756s |
| 3 | Shoutbox 群聊区（iframe shoutbox.php + 表单 + 表情） | `showshoutbox` | 无（实时） |
| 3.5 | `apply_filter('nexus_home_module', ...)` **首页插件挂载点** | — | — |
| 4 | Latest Forum Posts 论坛最新帖子（5 条） | `showlastxforumposts` | 无缓存，实时 SQL |
| 5 | Latest Torrents 最新种子（5 条纯文字表格，**无海报**） | `showlastxtorrents` | 无缓存 |
| 6 | Top Uploader 上传排行（"最近30天/全部时间" tab，前10） | `main.show_top_uploader` | `index_top_uploader_all/recently` 各 60s |
| 7 | Polls 投票（polls 表，20 选项，已投票显示条形图） | `showpolls` | 内容 7226s / 结果 3652s |
| 8 | Stats 站点数据（用户/种子/流量/等级分布三段） | `showstats` | `stats_users` 3000s / `stats_torrents` 1800s / `stats_classes` 4535s |
| 9 | Tracker Load 服务器负载（`exec('uptime')` 输出） | `showtrackerload` | 无 |
| 10 | Disclaimer 免责条款 | 恒显 | 无 |
| 11 | Links 友情链接（links 表） | 恒显 | `links` 86400s |
| 12 | 浏览器/客户端推荐 + Powered by 尾注 | 恒显 | 无 |

关键事实：**主线默认首页没有签到区块、没有海报墙、没有魔力卡片**——这些全是大站定制/插件。签到是独立页 `public/attendance.php`（`$attendance_initial/step/max/continuous_bonus` 魔力奖励配置在 config），魔力页为 `mybonus.php`。尾部 `last_home` 更新 + 清 `unread_news_count` 缓存：首页即"已读公告"判定点。

## A1. "好学"站首页快照 [实证-本地]

在主线骨架上的定制（快照要点）：

- 顶部导航：首页/论坛/勋章墙/课本（官种/保种区）/资源库（按学段多级分类）/发布/帮助/工具/**娱乐市场**/**站免池**/PM管理/我的数据/**火花银行**
- 用户信息条：分享率/上传/下载/当前活动/连接数/认领/**签到已得120, 补签卡:8**/火花余额/邀请/站内信
- 最新消息（首条展开）+ 大段文字公告（招聘/开注时间/QQ/TG 群）
- **上传排行 = 领奖台化**：TOP1-3 冠军/高光席位（头像+"领先15254种"压制描述）+ 追赶梯队 4-10 名，双 tab（最近30天/全部时间）——主线 top uploader 的深改
- 群聊区 → 论坛最新帖子 → 聊天 → 趣味盒（"53个用户共93票"）→ 投票
- **游戏挂载位**："🎮 水浒传卡牌游戏"卡片 = `nexus_home_module` 过滤器挂的插件（导航还有五子棋/农场/刮刮乐/九宫格抽奖/猜大小/卡牌合成）
- 站点数据（今日/本周访问、注册 4645/5000、男女比例、等级分布 Peasant→Nexus Master 全列）、服务器负载（uptime 23 天）、免责、友链；footer "Redis 42 reads 1 writes / page created in 0.031 sec"（**$Cache 底层是 Redis**）

## A2. 逐站 [实证：PT-Plugin-Plus config / ptool site 反推]

- **M-Team (馒头)** `kp.m-team.cc`：旧版 NP 页面 movie/music/adult 三分频道；**新版独立 "mTorrent" SPA**（ptool 专设 `site/mtorrent` 适配器）：纯 JSON API `/api/torrent/search`（POST，pageSize=100）、`/api/torrent/genDlToken`、`/api/member/profile`；免费/折扣 `status.discount: NORMAL/PERCENT_50/PERCENT_70/FREE + discountEndTime` [实证]。新版列表 = 海报缩略图表格（`img.torrent-list__thumbnail`）+ ant-design 组件 + 搜索面板 [实证-用户脚本选择器]。首页具体模块未见公开截图 [未获取]，可确定"首页即种子发现流 + 顶栏个人数据"的 SPA 形态。
- **HDSky (天空)** `hdsky.me`：selectors 仅 torrents/details → 工具侧只适配列表；首页细节 [未获取]（[业内] 传统 NP 首页）。
- **Audiences (观众)** `audiences.me`：仅 userExtendInfo；"好学"快照即观众系同源模板（gntv 管理组），首页形态可参照 A1 [实证-同源推断]。另 `_ref/audiences/` 有其 userdetails 抓取（hero banner + swiper 轮播体系，站级视觉参考）。
- **OurBits (皇后)**：插件"保种列表"；[业内] 传统 NP。
- **HDHome (家庭)**：特色 LIVE 频道 `/live.php`（演唱会）[实证]；[业内] NP 标准。
- **PTHome**：config 空壳 → [未获取]。
- **U2 (动漫花园)** `u2.dmhy.org`：userExtendInfo + bonusExtendInfo；"家用宽带上传流量抵扣"政策；NP 标准 + 自定义配色 [业内]。
- **BYRBT (北邮人)**：cat401-410 全分类走 torrents.php [实证]；传统 NP 首页 [业内]。
- **HD Dolby (黄豆)**：12 分类含"Study 学习" [实证]；[业内] NP。
- **海棠 HTPT** `htpt.cc`：bonusExtendInfo + 种子列表/详情插件 [实证]；新站（2024+）。
- **PTer (蒲公英)**：保种/官方种列表插件 + music.php 频道 [实证]。
- SSD(春樱)、TJUPT、HDTime、白兔、北斗、OPEN.CD、DICMusic、HD4Ffans、eastgame 等在 PTPP 均有 config [实证]，形态均为 NP 标准首页 [业内]。
- 银子（silver 系）、铂金短剧 PTSKit：PTPP 目录无匹配 → [未获取]。

**NP 站事实"标配区块"** = A0 主线 12 项 + 大站普遍加装：签到入口、魔力/银行（mybonus/bank）、海报墙式种子列表（torrents 页）、娱乐/游戏插件、勋章、保种区。

## B. 国际闭源大站

- **PassThePopcorn (PTP)**：Gazelle 系 [业内]；首页 = 新闻/公告 + 最新上传海报行 + 论坛枢纽 [业内，弱证据]。
- **Broadcasthe.net (BTN)**：Gazelle 系，"剧集组"粒度 [业内]；首页 [未获取]。
- **HD-Bits (HDB)**：自研，极简 [业内]；[未获取]。
- **AnimeBytes (AB)**：自研（类 Gazelle 布局），首页新闻+海报 [业内]；[未获取]。
- **MyAnonamouse (MAM)**：自研，首页 = 公告+最新种子+论坛+签到/免费种/推荐书目等大量自定义区块 [业内]；[未获取]。
- **IPTorrents (IPT)** / **TorrentLeech (TL)**：自研；首页 = 最新种子表为主 [业内]。
- **BeyondHD (BHD)**：UNIT3D 架构 [业内公认]；可参照 UNIT3D 实证。
- **FileList (FL)**：登录页实测 [实证]：含 "Login on any IP" 勾选、recover/signup、**iRC web chat 客服**；登录后首页 [未获取]。
- **Blutopia**：UNIT3D 开源版即其同源（见 UNIT3D 素材）。

## C. 汇总

### C1. 华语 NP 系首页「标配区块」并集 [实证为主]

| 区块 | 出现证据 |
|---|---|
| 公告/最近消息 news（折叠展开） | NP主线 + 好学 + baozi |
| 签到（入口在用户条/独立页 attendance.php） | NP主线文件 + 好学（"签到已得120 补签卡8"）+ baozi（首页整月签到日历） |
| 魔力/银行（火花银行、bonusPerHour） | NP主线 mybonus + PTPP mteam/u2/htpt 选择器 |
| 个人数据条（分享率/上传/下载/做种/当前活动/邀请/私信） | 好学 + baozi 快照 |
| 群聊区 shoutbox | NP主线 + 好学 + baozi |
| 论坛最新帖子 | NP主线 + 好学 |
| 最新种子（首页仅 5 条文字；海报墙在 torrents.php） | NP主线；M-Team 新版海报缩略图表格 [实证] |
| 上传排行（可领奖台化/30天-全部 tab） | NP主线 top uploader + 好学 podium |
| 投票 poll | NP主线 + 好学 |
| 趣味盒 funbox | NP主线 + 好学 + baozi |
| 游戏插件挂载位 nexus_home_module | NP主线代码 + 好学水浒卡牌 + baozi 幸运大转盘流水 |
| 站点数据大表格（含等级分布/男女比例） | NP主线 + 好学 + baozi |
| 服务器负载 uptime | NP主线 + 好学 |
| 免责条款 + 友情链接 | NP主线 + 好学 + baozi |
| 站免池/保种区/勋章墙/绩效考核（大站定制） | 好学导航 [实证]；ourbits/pter "保种列表"插件 [实证] |
| 新增资源统计图表（30 天柱状图） | baozi 独有 [实证-本地快照] |

### C2. 国际站首页「标配区块」并集

| 区块 | 出处 |
|---|---|
| News/公告（主列， AJAX 分页） | Gazelle [实证]、UNIT3D news [实证] |
| 最新上传海报行/墙 | Gazelle Latest Uploads [实证] |
| Featured 精选轮播 | UNIT3D featured-carousel [实证] |
| Chat 聊天 | UNIT3D chat [实证]；FL IRC 客服 [实证] |
| Poll 投票 | Gazelle + UNIT3D [实证] |
| Top Torrents / Top Users | UNIT3D livewire [实证] |
| 最新话题/帖子/评论 | UNIT3D [实证] |
| Online 用户列表 | UNIT3D [实证] |
| 竞赛/leaderboard | Gazelle Contest Leaderboard [实证] |
| Stats 侧栏（精简版，无等级分布/男女/负载） | Gazelle [实证] |
| 用户级区块排序/显隐 | UNIT3D {block}_position [实证] |
| 免责/友链/服务器负载 | **均无**（Gazelle/UNIT3D 实证缺位） |

### C3. 两派设计哲学差异

1. **华语 NP = 社区运营面板**：首页堆叠 15+ 区块纵向长页；首页承担**留存运营**（每日签到回访、魔力经济、荣誉激励），资源发现让位给 torrents.php；区块由管理员全局设定（主线 7 个 `$MAIN` 开关），用户个性化≈0。M-Team 新版例外：SPA+纯 API，首页直接做成**资源发现入口** [实证]。
2. **国际 Gazelle = 新闻发布板+论坛枢纽**：主栏 News 流（旧闻归档论坛）+最新上传海报墙+侧栏（AOTM/竞赛/投票/Stats），刻意精简，无运维面板元素。
3. **国际 UNIT3D（BHD/Blutopia）= 可配置发现面板**：Featured 海报轮播+随机媒体+Top 榜，杀手锏是**每用户可见性+拖拽排序**。
4. **共性交集**：News、投票、聊天、最新内容、统计。**差异最大的五点**：签到/魔力经济（华语独有）、趣味盒/游戏（华语独有）、服务器负载与免责/友链（华语独有）、Featured 海报轮播（国际独有）、用户级区块排序（UNIT3D 独有）。

### 证据缺口

- M-Team 新版 SPA 首页首屏模块明细 [未获取]。
- PTP/BTN/HDB/AB/MAM/IPT/TL 登录后首屏：本轮 wiki/评测源超时或 404，仅 [业内] 定性结论。
- 银子（silver 系）、PTSKit、HDArea 首页 [未获取]。
