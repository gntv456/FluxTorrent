# FluxTorrent

通用 PT 建站系统（11 种预置站型：教育/影视/音乐/动漫/电子书/综合/体育/游戏/软件/纪录片/无损，模块可开关）—— Rust (Actix-web) + Next.js 15 + PostgreSQL 16 + Redis，基于 Monorepo（Turborepo + Cargo workspace）。

> 策划总纲见 `_doc/FluxTorrent 策划方案.md`（技术架构、29 模块 PRD、数据模型、UI 规范、路线图）。

## 目录结构

```
FluxTorrent/
├── apps/
│   ├── api/          # Rust Actix-web 业务 API（认证/种子/促销/经济/管理）
│   │   ├── migrations/   # sqlx 迁移（0001 init + 0002 sequences）
│   │   └── src/          # domain(纯业务)/repo(sqlx)/http(handlers)/bencode/…
│   ├── worker/       # Rust 异步任务（促销到期/保种移出/做种收益/announce 消费）
│   ├── tracker/      # Rust 私有 Tracker（passkey 鉴权/内存 peer 表/compact 响应/scrape）
│   └── web/          # Next.js 15 前端（设计 Token §7.1 / RSC / 多端）
├── packages/
│   └── domain-types/ # 前后端共享契约（信封/错误码/枚举/魔法数字）
├── docker/           # docker-compose.yml + 各服务 Dockerfile + .env.example
└── _doc/             # 策划方案与参考资料
```

## 快速启动

### 一键（Docker Compose）

```bash
# 注意：compose 的 project 目录是 docker/，env 文件须放在那里
cp docker/.env.example docker/.env && vim docker/.env   # 必填 DB_PASSWORD/REDIS_PASSWORD/JWT_SECRET（缺失会拒绝启动）
docker compose -f docker/docker-compose.yml up -d
# web: http://localhost:3000  api: http://localhost:8080/api/v1/health
# ⚠️ 起来后先访问 http://localhost:3000/setup 完成安装向导（见下一节）
```

### 本地开发

```bash
# 1. 基础设施
docker compose -f docker/docker-compose.yml up -d postgres redis

# 2. API（自动执行迁移）
DATABASE_URL="postgres://flux:fluxdevpass@127.0.0.1:5432/fluxtorrent" \
REDIS_URL="redis://127.0.0.1:6379" \
JWT_SECRET="dev_secret_change_me_at_least_32_bytes!" \
cargo run -p flux-api

# 3. Worker（定时任务）
DATABASE_URL="postgres://flux:fluxdevpass@127.0.0.1:5432/fluxtorrent" \
REDIS_URL="redis://127.0.0.1:6379" \
cargo run -p flux-worker

# 4. Tracker（方案 §2.5 自研轻量路线）
DATABASE_URL="postgres://flux:fluxdevpass@127.0.0.1:5432/fluxtorrent" REDIS_URL="redis://127.0.0.1:6379" TRACKER_BIND="0.0.0.0:7070" cargo run -p flux-tracker

# 5. Web
pnpm install
pnpm --filter @fluxtorrent/web dev
```

### 安装向导（装完 compose 后的第一件事）

```bash
# 1. 启动全部服务（迁移由 api 启动时自动执行）
cp docker/.env.example docker/.env && vim docker/.env   # 必填 DB_PASSWORD/REDIS_PASSWORD/JWT_SECRET（缺失会拒绝启动）
docker compose -f docker/docker-compose.yml up -d

# 2. 浏览器打开 http://localhost:3000/setup 完成三步向导：
#    ① 选站型（11 种预置：综合/教育/影视/音乐/动漫…，决定分类/维度/模块缺省）
#    ② 站点名称 + 管理员登录
#    ③ 合规勾选 → 完成安装（幂等，可重复访问）
```

**管理员账号**：系统已自动预置 `root`（初始密码 `password123`，首次登录强制修改）。向导第 ② 步用改密后的新密码登录即可。完成前所有业务 API 处于装机封锁状态（只放行登录/改密），这是设计行为。

**开站前三项检查**（不检查也能跑，但建议先改）：

1. **tracker announce 地址**：后台「站点设定」把 `announce_url` 从 `http://127.0.0.1:8080/announce` 改成你的公网域名——否则用户下载的 .torrent 里是内网地址，无法做种。
2. **邮件 SMTP**：后台「站点设定 → 邮件」填 SMTP 服务器/端口/发件人即生效（找回密码/邀请信/群发都用它）；也可用环境变量 `SMTP_URL`/`SMTP_FROM`（后台设定优先）。
3. **注册模式**：默认邀请制（invite_only）；要开放注册改 `registration_mode=open`，配套在「邀请管理」里发邀请码。

### 引导首个普通用户

注册默认邀请制。装好后在后台「邀请管理」给用户直发邀请码，或让 LV3+ 用户在 `/invites` 自助生成；也可切换为开放注册（见上）。

## 质量门禁（方案 §8.4）

```bash
cargo fmt --all --check   # 格式 ✅
cargo check               # 零警告 ✅
cargo test                # 79 单测（促销/密码/JWT/Bencode/peer 表/计费倍率/签到/利息/站免池窗口/BEP-7 peers6/snapshot v4v6 分列）✅
pnpm --filter @fluxtorrent/web exec next build   # tsc strict ✅
```

## 已验证的端到端链路（实测通过）

| 链路 | 结果 |
| :--- | :--- |
| 注册（邀请码一码一用，错误码 2005/2006 区分） | ✅ |
| 登录 + JWT + `/me` + 无 token 401 | ✅ |
| 登录限流（第 6 次/分钟 → 401） | ✅ |
| 上传 .torrent（Bencode 解析/info_hash/重复检测/待审核） | ✅ |
| 审核 → 列表/详情（筛选 Chip/游标分页） | ✅ |
| 评论（UTF-8 中文）/感谢（重复 → 5002）/收藏 | ✅ |
| 下载 .torrent（注入本站 announce + passkey + private=1） | ✅ |
| 促销引擎（保种区 seeders>7 自动移出 + 3 天免费延续） | ✅ |
| **Tracker announce 全链路**（BEP3 二进制安全：percent-encoded info_hash/peer_id + compact 响应 + 错误 passkey 拒绝） | ✅ |
| **announce → Redis Stream → worker 计费**（BEP3 累计量转增量计费 + 游标失败暂停重试 + 损坏事件死信 + 促销倍率裁决） | ✅ |
| **下载 info_hash 一致性**（上传 raw 存储 → 下载重注入 announce + private=1 → 重算 SHA1 与库中一致） | ✅ |
| **用户上/下载量/种子计数快照回填**（权威在 traffic_ledger 流水，worker 周期刷新） | ✅ |
| **M11 商店**：18 商品（旧站全集）、幂等购买（同键重放零重复扣款）、上传量商品即刻到账 | ✅ |
| **M11 银行**：7/30/90/180/365 天五档利率、到期利息（10000×18%×365d=1800 实测）、提前支取仅本金 | ✅ |
| **M12 签到**：首签+10、连签+5、里程碑 10/20/30 天加 200/500/1000（封顶 1000）、当日重复签到拒绝 | ✅ |
| **M13 站免池**：捐赠入池、进度百分比、捐赠排行 | ✅ |
| **M14 勋章**：图鉴（14 枚起步集）、购买、赠送、单佩戴位切换 | ✅ |
| **M15 论坛**：9 版块（旧站口径）、发主题/回帖、浏览计数、发帖火花奖励（+2/+1） | ✅ |
| **M16 短讯与好友**：收发件箱、按用户名发送、好友列表 | ✅ |
| **M17 求种/候选/字幕**：悬赏冻结与转移、候选 1 火花投票、staff 转正为官种、字幕 +5 火花 | ✅ |
| **M18 课本中心**：课本库浏览（科目/版本/年级）、种子关联 | ✅ |
| **M20 排行榜**：上传榜 Top21（含做种体积聚合） | ✅ |
| **M21 绩效考核**：指标全系统流水采集（零手工填报）、最低要求线拒绝、达标月数加成、幂等领取 | ✅ |
| **M22 任务中心**：认领限流 + 重复认领拒绝、限时任务表 | ✅ |
| **M19 保种区**：待保种列表（低做种优先）、认领（worker 自动移出规则已上线） | ✅ |
| **web 全站页面**：14 页渲染真实数据（含娱乐屋游戏页） | ✅ |
| **M24 娱乐玩法（首批）**：刮刮乐（五档奖池/EV 0.66）、猜大小（平局返本/2x）、统一交易管线 + 下注上限 + 每时限次 fail-close 风控 | ✅ |
| **全站代码审查修复**（2026-09-09）：BEP3 增量计费、earn_spark 行锁幂等、封禁即时生效、bencode 深度限制、Tailwind 接入、Dockerfile 全修、双 URL、对比度 4.6:1 等 30+ 项 | ✅ |
| **折叠屏适配基线**：viewport-segment 铰链预留 CSS（需真机验证）、外屏 ≤360px 降级、三折叠栅格 | ✅ |
| **0069 生态适配**（2026-09-13，对照《主流PT架构横向对比与借鉴》v2）：`pieces_hash` 跨站辅种二级指纹（上传写入 + worker 存量回填）、Tracker **BEP-7** IPv6 peers6、NP 兼容端点（`/compat/nexusphp/*` 含 download.php 形状）+ `/compat/meta` 架构自描述、**30 分钟临时下载凭证**（与长期 Token/passkey 解耦）、API Token 180 天时效 + 上限 3 枚、聚合组 torrent_groups（详情页「同组版本」）、agent_rules 命中落 `cheat_events` + 管理组信箱告警、`/stats` 对象级读缓存（60s TTL） | ✅ |
| **多语言三语**（2026-09-09）：zh-CN/zh-TW/en Cookie 切换、零新增依赖（自研字典 + task-local 后端错误本地化，Accept-Language 协商）、全站 21 页 + 9 组件文案抽取 | ✅ |
| **全文搜索（标题外）**（2026-09-09）：pg_trgm GIN 索引 ×4，标题/副标题/简介/文件名四列 ILIKE 子串匹配（中文友好），零外挂搜索服务 | ✅ |
| 做种收益结算（spark_ledger 流水 + 幂等重跑） | ✅ |
| Web 三页渲染真实数据（首页统计/列表/详情含免费徽章） | ✅ |

## API 概览（/api/v1）

| 方法 | 路径 | 说明 |
| :--- | :--- | :--- |
| GET | /health | 健康检查 |
| POST | /auth/register · /auth/login | 注册（邀请码）/登录 |
| GET | /me · POST /me/passkey/rotate | 当前用户 / passkey 重置 |
| GET | /torrents · /torrents/{id} | 列表（筛选+全文搜索+游标分页）/详情 |
| POST | /torrents（multipart） | 发布（待审核） |
| GET | /torrents/{id}/download | 动态生成 .torrent |
| GET/POST | /torrents/{id}/comments | 评论 |
| POST | /torrents/{id}/thanks | 感谢（一人一次） |
| PUT | /torrents/{id}/bookmark | 收藏开关 |
| GET | /stats | 站点统计（旧站首页口径；Redis 60s 读缓存） |
| POST | /invites | 邀请发放（LV3+ 周配额） |
| GET/POST | /torrents/{id}/group | 聚合组查询 / 挂入（同资源多版本） |
| GET | /compat/meta | 架构自描述（第三方适配器接入入口） |
| GET | /compat/nexusphp/user.json · torrents.json · torrent/{id}.json | NexusPHP 字段口径兼容端点（API Token 鉴权） |
| GET | /compat/nexusphp/download.php?id=&passkey= | NP 形状下载端点（passkey 鉴权 + 限流） |
| POST | /downloads/keys · GET /downloads/{id}?token= | 30 分钟临时下载凭证（与长期 Token/passkey 解耦） |
| GET | /me/tokens · POST /me/tokens · /me/tokens/revoke | API Token 管理（180 天时效、上限 3 枚、可吊销） |
| GET | /admin/cheat-events | 客户端黑白名单命中记录（staff） |

统一信封 `{code, message, data, request_id}`；错误码分段见 `packages/domain-types`。

## 后续路线（对照方案 §9 路线图）

当前完成度 ≈ **Phase 2–5 主体 + M24 首批玩法**（M01–M22 后端全量 + M24 刮刮乐/猜大小 + web 14 页 + 折叠屏基线）。
Tracker 采用方案 §2.5 的自研轻量路线（~300 行核心，passkey 鉴权 + DashMap 内存 peer + Redis Stream 事件），已实测 30 万 announce/s 目标的架构基础（零 DB 依赖路径）。
经济系统：统一流水记账（spark_ledger 唯一事实源）+ 商店 18 商品的幂等购买 + 银行五档定期（提前支取不计息）+ 签到连签里程碑（10/20/30 天，单日封顶 1000）+ 站免池进度与捐赠排行。
下一步按方案排期：M24 农场/九宫格 → M26 PWA → 上线准备（压测/安全审计/部署文档）。
