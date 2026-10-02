# FluxTorrent

通用 PT 建站系统——不偏向任何 PT 类型，站长自由搭建、轻松自定义功能和字段。

- **11 种预置站型**（综合/教育/影视/音乐/动漫/电子书/体育/游戏/软件/纪录片/无损）+ custom 克隆：选一个起点，其余全部可改；
- **自定义能力**：用户字段与内容维度共用同一套六类型字段引擎；自定义页面/菜单/术语/主题令牌；30 个模块开关四端联动；
- **零魔改升级**：全部变更走 sqlx 迁移（0001 起），启动自动执行——不碰代码就永远可以 `git pull && up -d`；
- **架构**：Rust（Actix-web）四服务 api / worker / tracker / web（Next.js 15）+ PostgreSQL 16 + Redis，Turborepo + Cargo workspace monorepo。

技术架构与 29 模块 PRD 见 `_doc/FluxTorrent 策划方案.md`（内部）；**面向站长/用户/运维的成体系手册见 [`docs/`](docs/README.md)**。

## 目录结构

```
FluxTorrent/
├── apps/
│   ├── api/          # Rust Actix-web 业务 API（认证/种子/促销/经济/管理/兼容层）
│   │   ├── migrations/   # sqlx 迁移（0001 起，启动自动执行）
│   │   └── adapters/     # wasmtime 适配器 SDK 与内置示例（douban）
│   ├── worker/       # Rust 异步任务（45 个 job 应用内调度 + NP 导入器 np_import）
│   ├── tracker/      # Rust 私有 Tracker（HTTP + UDP BEP15 + IPv6 BEP7 + scrape）
│   └── web/          # Next.js 15 前端（RSC / 三语 i18n / PWA / 移动端全适配）
├── packages/
│   └── domain-types/ # 前后端共享契约（信封/错误码/枚举/魔法数字）
├── docs/             # 产品文档中心（站长/用户/自定义/运维四册）
├── docker/           # docker-compose.yml + 各服务 Dockerfile + .env.example + 监控栈
└── _doc/             # 内部策划案与审查报告（不对外）
```

## 快速开始

```bash
cp docker/.env.example docker/.env && vim docker/.env   # 必填 DB_PASSWORD/REDIS_PASSWORD/JWT_SECRET
docker compose -f docker/docker-compose.yml up -d
# 浏览器打开 http://localhost:3000/setup 完成三步向导：
#   ① 选站型 → ② 站点名称 + 管理员 → ③ 合规勾选 → 完成
```

管理员账号已预置 `root`（初始密码 `password123`，首次登录强制修改）。完成前所有业务 API 处于装机封锁状态（设计行为）。

**装完必读**：[docs/webmaster/launch-checklist.md](docs/webmaster/launch-checklist.md)——announce 公网地址 / SMTP / 注册模式三项开站检查。完整走法见 [docs/webmaster/quick-start.md](docs/webmaster/quick-start.md)。

本地开发（四进程起法）见 [docs/webmaster/troubleshooting.md](docs/webmaster/troubleshooting.md) 与 `_doc/生产部署指南.md`。

## 功能概览

核心链路（均已端到端实测，详见 CHANGELOG 各版本段）：

- **种子**：发布审核流 / 多维筛选 + 全文搜索（pg_trgm，中文友好）/ 聚合组同组版本 / pieces_hash 跨站辅种指纹 / 促销引擎（档位后台可配 + 预排 + 到期自动回退）；
- **Tracker**：BEP3 二进制安全 + compact + BEP7 IPv6 + UDP BEP15 + scrape；announce → Redis Stream → worker 异步计费（死信队列 + 游标重试）；压测 2138 req/s 零失败（见 docs/ops/performance.md）；
- **经济**：统一流水记账（spark_ledger 唯一事实源）+ 商店/银行五档/签到/站免池/券（免费/中性）/复活券 + 反通胀阀门（购买上限）；
- **用户**：等级四维门槛自动升降 + 进度卡 + 公开等级页 / H&R 全链（快照/惩罚/赦免）/ 考核引擎 / 勋章多佩戴 / 头像框 / 成就；
- **社区**：论坛（视频内嵌/投票/打赏/关注）/ 短讯好友 / 求种悬赏 / 字幕工作流（认证字幕人/评选）/ 组队玩法 / 娱乐屋；
- **管理**：45 个后台工具签 / 待审与申诉队列 / 批量调整（幂等）/ 作弊检测四件套（cheat_audit/multi_ip/highspeed/leak_scan）+ 客户端黑白名单 / 审计日志 / 群发；
- **自定义**：见 [docs/customize/](docs/customize/README.md)（字段/页面/菜单/站型/内容包/模块/适配器七篇）；
- **生态兼容**：开放 API（Token 180 天/上限 3 枚）+ RSS + `/compat/nexusphp/*` NP 形状兼容 + PT-Plugin-Plus 用户信息端点 + NexusPHP 数据导入器；
- **多机扩展**：计费流消费者组 + 任务分片（SKIP LOCKED + executed_by）+ tracker XFF 双档，三机配方已实证（`_doc/G30-多机部署三机配方.md`）。

## 运维

- 监控：`--profile monitoring` 起 Prometheus + Grafana，五条告警预置（docs/ops/monitoring.md）；
- 任务：45 个 job 后台面板可观测/可手动触发（docs/ops/jobs.md）；
- 备份：`scripts/backup.sh` + `restore.sh --drill` 两段式（docs/ops/backup.md）；
- 升级：`git pull && docker compose up -d --build`，注意事项见 CHANGELOG（docs/webmaster/upgrade.md）。

## 质量门禁

```bash
cargo fmt --all --check && cargo check && cargo test      # 144 个 Rust 单测
pnpm --filter @fluxtorrent/web exec next build            # tsc strict
scripts/ci_local.sh                                       # CI 等价本地闸门（8 个守门脚本）
```

CI（`.github/workflows/`）：build-test / e2e-smoke（含迁移幂等与装站链 11 断言）/ publish-images（semver tag → ghcr.io，amd64+arm64）/ security-audit（cargo-audit 周跑）/ release-drafter。

## 与同类开源项目

| | FluxTorrent | NexusPHP | UNIT3D |
| :--- | :--- | :--- | :--- |
| 定位 | 中立通用建站系统 | 中文 PT 事实标准 | 英文圈通用引擎 |
| 栈 | Rust + Next.js | PHP（Laravel 化中） | PHP Laravel |
| 自定义 | 六类型字段引擎 + 站型包 + 模块开关 | 改代码（魔改文化） | 少量字段 |
| 升级 | 迁移自动跑（零魔改前提） | 魔改站升级难 | git:update |
| 移动端 | 全路由适配 + PWA | 桌面优先 | 响应式 |
| 许可证 | MIT | GPL-2.0 | AGPL-3.0 |

完整对比与差距分析见 `_doc/开源生态全方位对比分析与完善策划案-2026-09-27.md`。

## 参与贡献

CONTRIBUTING.md 有开发约定（质量门禁/迁移纪律/i18n 门禁）。安全问题走 SECURITY.md 的私下披露流程，勿开公开 issue。
