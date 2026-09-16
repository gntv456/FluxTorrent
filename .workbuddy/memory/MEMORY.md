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
