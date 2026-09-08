# FluxTorrent

下一代教育类 PT 站点引擎 —— Rust (Actix-web) + Next.js 15 + PostgreSQL 16 + Redis，基于 Monorepo（Turborepo + Cargo workspace）。

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
cp docker/.env.example .env   # 修改密钥
docker compose -f docker/docker-compose.yml up -d
# web: http://localhost:3000  api: http://localhost:8080/api/v1/health
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

### 引导首个账号

注册采用邀请制（M01）。冷启动由站长直接向 `invites` 表发码：

```sql
-- root(站长, class 99) 为引导示例；发放一枚邀请码
INSERT INTO users (username, email, pass_hash, passkey, class_id)
VALUES ('root', 'root@flux.local', 'stub', 'rootpasskey0000000000000000000ff', 99);
INSERT INTO invites (inviter_id, code, expires_at)
VALUES (1, '<恰好32位字符的邀请码>', now() + interval '3 days');
```

## 质量门禁（方案 §8.4）

```bash
cargo fmt --all --check   # 格式 ✅
cargo check               # 零警告 ✅
cargo test                # 25 单测（促销/密码/JWT/Bencode/peer 表/计费倍率/签到/利息/站免池窗口）✅
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
| **announce → Redis Stream → worker 计费**（游标消费不丢不重 + 促销倍率裁决：free 种子下行计 0） | ✅ |
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
| 做种收益结算（spark_ledger 流水 + 幂等重跑） | ✅ |
| Web 三页渲染真实数据（首页统计/列表/详情含免费徽章） | ✅ |

## API 概览（/api/v1）

| 方法 | 路径 | 说明 |
| :--- | :--- | :--- |
| GET | /health | 健康检查 |
| POST | /auth/register · /auth/login | 注册（邀请码）/登录 |
| GET | /me · POST /me/passkey/rotate | 当前用户 / passkey 重置 |
| GET | /torrents · /torrents/{id} | 列表（筛选+游标分页）/详情 |
| POST | /torrents（multipart） | 发布（待审核） |
| GET | /torrents/{id}/download | 动态生成 .torrent |
| GET/POST | /torrents/{id}/comments | 评论 |
| POST | /torrents/{id}/thanks | 感谢（一人一次） |
| PUT | /torrents/{id}/bookmark | 收藏开关 |
| GET | /stats | 站点统计（旧站首页口径） |
| POST | /invites | 邀请发放（LV3+ 周配额） |

统一信封 `{code, message, data, request_id}`；错误码分段见 `packages/domain-types`。

## 后续路线（对照方案 §9 路线图）

当前完成度 ≈ **Phase 2–5 后端主体**（M01–M22 全部后端模块；含 announce 计费全链路、经济系统、考核任务）。
Tracker 采用方案 §2.5 的自研轻量路线（~300 行核心，passkey 鉴权 + DashMap 内存 peer + Redis Stream 事件），已实测 30 万 announce/s 目标的架构基础（零 DB 依赖路径）。
经济系统：统一流水记账（spark_ledger 唯一事实源）+ 商店 18 商品的幂等购买 + 银行五档定期（提前支取不计息）+ 签到连签里程碑（10/20/30 天，单日封顶 1000）+ 站免池进度与捐赠排行。
下一步按方案排期：web 前端页面补全（商店/论坛/勋章/考核等）→ 折叠屏全形态 → M24+ 玩法与生态 → 上线准备。
