# FluxTorrent 工作区长期备忘

## 项目定位（2026-09-15 用户明确）
- **FluxTorrent 是通用 PT 程序，可建成任何类型站点（教育/影视/音乐/综合等），默认是综合站，对标 NexusPHP（华语 PT 事实标准）。**
- 设计任何功能时**默认按综合站 + NP 合并式语义**，不要绑定教育站特化前提；站型差异用 `site_type` 参数化（开/关 + 指标口径），而非写死。
- 涉及"考核&任务"时：综合站默认要 NP 那套 exam+task 合并式（含面向用户的新人转正/周期考核引擎），不能只做 U3D 式任务+成就路线。

## 部署与协作约定（2026-09-15 确立）

- **运行方式是 Docker**：六容器（postgres/redis/api/worker/tracker/web），改完代码必须
  `docker compose -f docker/docker-compose.yml up -d --build api worker web` 才会生效；
  sqlx 迁移在 api 启动时自动执行。
- **并行会话撞号事故**：2026-09-15 有两个会话同时在此仓库工作，迁移版本号撞车
  （对方把 0087_metadata_unify 重编号为 0088，还加了 0087_p2_hardening / 0089_home_layout）。
  约定：**创建迁移文件前先 `Glob apps/api/migrations/009*.sql` 等查当前最大版本号**，
  从最大号 +1 开始，不要假设自己上次编的号还有效。
- 排障时 PowerShell 输出可能被吞：用 `| Out-File <file> -Encoding utf8` + Read 工具兜底；
  docker netstat 判活不可靠，用 `docker ps` 查容器状态。
- **端到端验证套路**（容器部署下）：铸 HS256 JWT（claims 必须含 `iat`，否则 401）→
  curl 带双 Cookie（`flux.session=1` + `flux.token=<jwt>`）抓 `/torrents` 渲染 HTML →
  用 python 提取标记比对。API 直连验证用 `Authorization: Bearer <jwt>`。
- **前端/后端参数约定（踩过的坑）**：`api.get` 不自动补 `/api/v1` 前缀；SSR fetch 必须走内网
  `API_SERVER_URL`（web 容器里 localhost:8080 指自己）；页面收到的 searchParams 必须显式转发给
  API（否则筛选静默失效）；列表端点的 count 查询必须与列表同套谓词（否则总数与实际行数不符）。

## 竞品更新机制调研（2026-09-16）
- **PHP 系（NexusPHP / UNIT3D）天然近零停机**：PHP 解释型，覆盖文件即生效。
  NexusPHP 有 `php artisan nexus:update`（CLI）或 `public/update/update.php`（Web），自动 rsync 覆盖代码（保留 .env）+ 合并配置 + 跑迁移；UNIT3D 用 `php artisan down → git:update/migrate → up`，维护模式仅几秒，队列 worker 独立 restart。
  共同点：统一向后兼容迁移（artisan migrate），前端资源单独编译，不绑后端重启。
- **编译型新一代（Torrust Rust+Vue / Arcadia Rust+TS/Vue）走"官方镜像 + pull 升级"**：Torrust 提供 `torrust-index`/`torrust-tracker` 官方镜像，用户 `docker compose pull && up -d` 即可，不必装 Rust 工具链。
- **FluxTorrent 当前最突兀短板**：无官方镜像仓库，逼用户本地 build 整个 Rust workspace（release 编译数分钟）。对齐 Torrust/Arcadia 发 ghcr 预构建镜像（升级方案 L1），是缩短用户升级成本、逼近零停机的最短路径。
- Tracker-only（XBT/Ocelot/Chihaya/aquatic，C++/Go/Rust）更新 = 重新编译 + 重启进程（秒级），BT 客户端自动重连。

## Docker 磁盘治理（2026-09-16，C 盘 91%→50%）

- **膨胀元凶**：BuildKit build cache 累积 378GB（每次改 Rust 代码重建都留中间层，从不回收）；vhdx 只增不减。
- **处置**：`docker builder prune -af`（377GB）→ `docker image prune -af --filter until=168h`（20GB）→ 导出 DB 备份 → wsl --shutdown + `Optimize-VHD -Mode Full`（需管理员，UAC 弹窗）→ vhdx 404GB→14GB。
- **坑**：Optimize-VHD 后 Docker Desktop 引擎起不来（pipe 500）——必须**彻底杀 Docker Desktop/com.docker.backend 进程 + wsl --shutdown 再冷启**，只等是等不来的。
- **防复发（已落地）**：`~/.docker/daemon.json` builder.gc：20GB 上限 + 7 天未用即回收；项目 .dockerignore 追加 `_*`/`**/_*`/backups/target-test/UI参考 等根目录一次性测试文件。
- **迁 D 盘**（如仍需要）：Docker Desktop → Settings → Resources → Disk image location 改到 D 盘，会自动搬迁（搬完 C 盘 `%LOCALAPPDATA%/Docker/wsl` 才会真正释放）；WSL 也可 `wsl --export/--import` 手动迁。本次 vhdx 已缩到 14GB，迁移耗时很短，随时可做。
- 备份：`backups/pgdata-20260916/{fluxtorrent,p2}.dump`（pg_dump -Fc 格式）。

## 已有能力清单（2026-09-17，做新功能前先查，避免重复造轮子）

**教训**：为"社交经营玩法"做了五轮凭空设计，第六轮读代码才发现其中"防伪体系"和"经济模型"FluxTorrent 早已实现；第七轮又发现连《抢救断种》都早已实现（`ops_http.rs` 复活任务）。**根因是调研漏了接口层和前端页面。**

**设计任何新功能前，四步走（缺一不可）：**

```bash
ls apps/api/migrations/                              # 1. 表有没有
grep -n "pub async fn" apps/worker/src/jobs.rs       # 2. 定时任务有没有（2521 行）
grep -rni "<关键词>" apps/api/src/*_http.rs          # 3. 接口有没有 ← 关键，别漏
ls "apps/web/app/(main)/"                            # 4. 前端页面有没有 ← 关键，别漏
```

只查表结构会得出"功能不存在"的错误结论——**"功能是否存在"最直接的证据在接口层和页面目录**。

**已验证存在的功能（不要重建）：**
- **资源抢救/复活**：`ops_http.rs:937`（0073，U3D Graveyard 口径）`GET /resurrections` / `POST /resurrections/claim` / `GET /resurrections/mine`；表 `resurrections`（`torrent_id UNIQUE`/`user_id`/`required_hours`/`reward_sparks`/`status`）；前端 `app/(main)/resurrections/`。已含"不能领自己的种"、CAS 防双领、30 天活动窗、每小时验收、免费券联动
- **保种认领**：`seed_preserve` + `jobs.rs::preserve_settle` / `preserve_exit`
- **做种激励**：`jobs.rs::seeding_reward`

**关键已有能力（直接复用，不要重写）：**

| 能力 | 位置 | 要点 |
|---|---|---|
| 做种激励 | `jobs.rs::seeding_reward` | 规则分档（濒危2.0/高龄1.5/老1.0/大体积0.75/中0.5/日常0.25）× `seeders^-0.35` × 时长半衰 `1/(1+h/2160)`；arctan 软封顶 `base+400/π·atan(Σ×6/50)`；反假靠 `NOT(connectable=0 AND uploaded=0)` |
| 保种认领结算 | `jobs.rs::preserve_settle` / `preserve_exit` + `seed_preserve` 表 | 每满 24h 发 `preserve_bonus_per_day`(100)；`seeders>7` 移出+免费 3 天 |
| 幂等流水账本 | `spark_ledger`（分区表） | `idempotency_key`，分区表无法全局唯一约束，**应用层先查后插**；`kind` 区分来源 |
| 反作弊事件 | `cheat_events` | user_id/agent/peer_ip/reason/hits |
| 站型包 | `site_type_packs` | 11 种 code（education/movie/music/anime/ebook/general/sports/game/software/documentary/lossless），`modules` JSONB 存模块开关 |
| 任务/勋章/好友/小游戏 | `tasks`·`task_claims` / `medals`·`user_medals` / `friendships` / `games.rs`·`fun_items`·`farm_*`·`gomoku_games` | |

**术语**：站内货币的**内部标识**是 spark（`users.spark_balance` / `spark_ledger` / `earn_spark` / `gift_spark`），
代码注释里也用「火花」当领域词。但**用户可见的名字由站长配置**：
`site_settings.currency_name`（默认 **「魔力」**），前端走 `{magic}` 占位符 →
`useI18n().currency` / `getDict().currency` + `.replace("{magic}", currency)`。
**新增用户可见文案一律用 `{magic}`，不要硬编码货币名**（2026-09-18 用户决策：默认态统一「魔力」，见 0122 迁移）。

**存档**：`_doc/落地修正-基于现有代码的增量方案.md` 有完整的复用/新增边界与 0102 迁移脚本草案。

## 站点身份 / 品牌配置链路（2026-09-18 摸清，改品牌必看）

**站点「叫什么」分散在多处，改默认/去教育化要全覆盖：**

| 位置 | 作用 | 默认值现状 |
|---|---|---|
| `site_settings.site_name` | **站点简称 / 前台品牌**（`site-profile` 的 `brand`：site_name 优先，取不到才回落当前站型包 `site_type_packs.brand`） | FluxTorrent（0118 起） |
| `site_settings.SITENAME` | 全站标题 / 邮件署名 / **RSS 频道名** | FluxTorrent（0120 起；原种子 `好学 FluxTorrent`） |
| `site_settings.site_title` | 站点副标题 | ''（原种子 `baozi`） |
| `site_settings.site_subtitle` | 站点口号 | ''（原种子英文口号） |
| `site_settings.titlekeywords`/`metakeywords`/`metadescription` | SEO | 中性（原种子 `教育,PT,种子` 等） |
| `site_type_packs.brand` | 各站型包品牌占位（`site-profile` 兜底） | 全部 ''（0118 清空） |
| `apps/web/public/` → `apps/web/app/manifest.ts` | PWA 安装名（**动态**读 `site_profile.brand`，`force-dynamic`） | 跟随 site_name |
| `apps/api/src/rss_http.rs` | RSS `<title>`/`<description>`（**0120 起改为读 SITENAME→site_name / site_desc→metadescription**，不再硬编码） | 跟随设定 |
| `apps/api/src/games_http.rs` | 小游戏展示名（`农场` 原为 `好学农场`） | 中性 |

- **默认站型 = `general`**（0119 起，原 `education`）；`module_textbooks` 默认 `no`；默认分类 = general 10 类。
- **曾经的"教育化默认"来源**：`0001`(categories 教育集) / `0025`(site_name/site_title/site_subtitle) / `0034`(SITENAME/SEO) / `0037`(site_type=education) / `0039`(module_textbooks=yes) / `0043? 0110`(刻意保留 education 包品牌)。
- **`site_type` 仍是运行时可选项**：迁移只把「初装默认」翻转为 general；站长在向导里主动选 education 会被记录、不再被覆盖（迁移只跑一次）。
- **站内货币名链路（2026-09-18 摸清）**：`site_settings.currency_name`（默认「魔力」）→ `site-profile` →
  `getDict()` 的 `currency` → 前端 `{magic}` 占位符。**用户可见文案散落在三层**，改默认态要全覆盖：
  ① 三语字典 `i18n/*.ts`（`{magic}` 占位符）；② **DB 种子数据**里的标签/单位/描述
  （`shop_items.name` / `settings_meta.{label_zh,unit,hint}` / `site_settings.descr` / `modules.descr` /
  `fun_polls.question` / `staff_panel_entries.info`，见 0122）；③ **worker 发的系统私信**
  （`bank_jobs.rs`/`jobs.rs`/`task_jobs.rs`）。另：`messages` 里的历史站内信**有意保留旧词**，不重写历史。
- 迁移号：**0122** 为本轮最后一条（0117 去包子 / 0118 品牌=FluxTorrent / 0119 默认 general /
  0120 设定默认中性 / 0122 货币名统一魔力）。0121 是并行会话的论坛关注（`forum_follows`）。
