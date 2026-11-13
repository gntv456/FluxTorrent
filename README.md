<div align="center">

<img src="apps/web/public/brand/logo-wordmark.png" alt="FluxTorrent" width="400"/>

**通用 PT 建站系统** —— 不偏向任何内容类型：选一个站型起步，其余全部可改。

[![CI](https://github.com/gntv456/FluxTorrent/actions/workflows/build-test.yml/badge.svg)](https://github.com/gntv456/FluxTorrent/actions/workflows/build-test.yml)
[![Release](https://img.shields.io/github/v/release/gntv456/FluxTorrent)](https://github.com/gntv456/FluxTorrent/releases)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![GHCR](https://img.shields.io/badge/ghcr.io-gntv456%2Ffluxtorrent-2496ED?logo=docker&logoColor=white)](https://github.com/gntv456/FluxTorrent/pkgs/container/fluxtorrent)

[Rust](https://www.rust-lang.org/) · [Actix-web](https://actix.rs/) · [Next.js 15](https://nextjs.org/) · PostgreSQL 16 · Redis 7

</div>

---

## 为什么是 FluxTorrent

- **11 种预置站型**（综合 / 教育 / 影视 / 音乐 / 动漫 / 电子书 / 体育 / 游戏 / 软件 / 纪录片 / 无损音乐）+ 自定义克隆：安装向导选一个起点，之后分类、字段、模块全部可改；
- **零魔改升级**：所有变更走 sqlx 迁移（0001 起，启动自动执行）。不改代码就永远可以 `git pull && docker compose up -d`——「魔改文化」导致的升级困境从架构上不存在；
- **自定义能力**：用户字段与内容维度共用同一套六类型字段引擎；自定义页面 / 菜单 / 术语 / 主题令牌；30+ 模块开关四端联动（见 [docs/customize/](docs/customize/README.md)）；
- **生态兼容**：PT-Plugin-Plus / Prowlarr / Jackett / Sonarr / Radarr / cross-seed / qBittorrent RSS / NexusPHP 老脚本，都有官方通道对接（见 [docs/integrations/](docs/integrations/README.md)）。

## 快速开始

需要 Docker + Docker Compose v2。镜像已预构建（amd64/arm64），无需本地编译：

```bash
cp docker/.env.example docker/.env
vim docker/.env          # 必填：DB_PASSWORD / REDIS_PASSWORD / JWT_SECRET
docker compose -f docker/docker-compose.yml up -d
```

打开 `http://localhost:3000/setup` 完成四步向导：**① 选站型 → ② 站点信息 → ③ 新手模板 → ④ 合规确认**。

- 管理员账号已预置 `root`（初始密码 `password123`，首次登录强制修改）；向导完成前所有业务 API 处于装机封锁状态（设计行为）；
- **装完必读**：[开站 checklist](docs/webmaster/launch-checklist.md) —— announce 公网地址 / SMTP / 注册模式三项检查；
- 完整走法（含域名 HTTPS / 反代 / 面板部署 / 离线部署）见 [站长手册](docs/webmaster/README.md)。

## 架构

```
浏览器 / PWA ──HTTPS──▶ web :3000（Next.js 15 · RSC · 三语 i18n · 移动端全适配）
                          └─REST / OpenAPI─▶ api :8080（Actix-web · 认证/种子/促销/经济/管理/兼容层）
                                               ├─▶ PostgreSQL 16 ＋ Redis 7
                                               └─▶ worker（45 个异步任务：计费 / H&R / 自动审种 /
                                                            作弊检测 / 邮件；多实例自动分片）

BT 客户端 ─announce / scrape─▶ tracker :7070 HTTP · :6969 UDP
              （BEP3 二进制安全 · BEP7 IPv6 · BEP15 UDP）peer 事件经 Redis Stream
              └─▶ worker 异步计费 ─▶ PostgreSQL
```

| 服务 | 技术 | 说明 |
| :--- | :--- | :--- |
| `apps/api` | Rust · Actix-web | 业务 API：认证 / 种子 / 促销 / 经济 / 管理 / 兼容层；启动自动执行 sqlx 迁移 |
| `apps/tracker` | Rust · Actix-web | 私有 tracker：HTTP + UDP（BEP15）+ IPv6（BEP7）+ scrape；announce 经 Redis Stream 异步计费，压测 2138 req/s |
| `apps/worker` | Rust · tokio | 45 个定时任务应用内调度（计费 / H&R / 自动审种 / 作弊检测 / 邮件），advisory lock 防抢跑，多实例自动分片 |
| `apps/web` | Next.js 15 | RSC 架构、中/英/日三语、PWA、全路由移动端适配 |
| `packages/domain-types` | TypeScript | 前后端共享契约（信封 / 错误码 / 枚举），`check_type_drift` 门禁防漂移 |

Turborepo + Cargo workspace monorepo，`docker compose` 一键编排。

## 功能一览

- **种子**：发布审核流（三层十五规则自动审种）/ 多维筛选 + 全文搜索（pg_trgm，中文友好）/ 聚合组同组版本 / pieces_hash 跨站辅种指纹 / 促销引擎（档位可配 + 预排 + 到期自动回退）
- **经济**：统一流水记账（唯一事实源）+ 商店 / 银行五档 / 签到 / 券体系 + 反通胀阀门；对账任务每 6h 比对流水与余额
- **用户**：等级四维门槛自动升降 / H&R 全链（快照-惩罚-赦免）/ 考核引擎 / 勋章 / 头像框 / 成就
- **社区**：论坛（视频内嵌 / 投票 / 打赏）/ 短讯 / 求种悬赏 / 字幕工作流 / 组队玩法
- **管理**：45 个后台工具签 / 作弊检测四件套 / 客户端黑白名单 / 审计日志 / 群发
- **生态**：开放 API（Token 制）+ RSS（enclosure 直链）+ Torznab 出口 + NexusPHP 兼容层与别名路径 + 增量新种流 + Token 化发种 + NP 数据导入器

## 文档

| 册 | 读者 | 入口 |
| :--- | :--- | :--- |
| 站长手册 | 装站、开站、升级、备份 | [docs/webmaster/](docs/webmaster/README.md) |
| 用户指南 | 站点终端用户 | [docs/user/](docs/user/README.md) |
| 自定义手册 | 字段 / 页面 / 菜单 / 站型 / 模块 | [docs/customize/](docs/customize/README.md) |
| 运维手册 | 监控 / 任务 / 性能 / 多机部署 | [docs/ops/](docs/ops/README.md) |
| 生态对接 | 工具与脚本开发者 | [docs/integrations/](docs/integrations/README.md) |

对外文档站（与仓库同步发布）：[wiki.ptang.top/ft](https://wiki.ptang.top/ft/)

## 与同类开源项目

| | FluxTorrent | NexusPHP | UNIT3D |
| :--- | :--- | :--- | :--- |
| 定位 | 中立通用建站系统 | 中文 PT 事实标准 | 英文圈通用引擎 |
| 技术栈 | Rust + Next.js | PHP（Laravel 化中） | PHP Laravel |
| 自定义 | 六类型字段引擎 + 站型包 + 模块开关 | 改代码（魔改文化） | 少量字段 |
| 升级 | 迁移自动跑（零魔改前提） | 魔改站升级难 | git:update |
| 移动端 | 全路由适配 + PWA | 桌面优先 | 响应式 |
| 许可证 | GPL-3.0 | GPL-2.0 | AGPL-3.0 |

## 目录结构

```
FluxTorrent/
├── apps/
│   ├── api/          # Rust Actix-web 业务 API（含 sqlx 迁移与适配器 SDK）
│   ├── worker/       # Rust 异步任务（45 个 job + NP 导入器）
│   ├── tracker/      # Rust 私有 Tracker（HTTP + UDP + scrape）
│   └── web/          # Next.js 15 前端（RSC / 三语 i18n / PWA）
├── packages/
│   └── domain-types/ # 前后端共享契约
├── docs/             # 产品文档中心（站长/用户/自定义/运维/生态五册）
├── docker/           # docker-compose.yml + 各服务 Dockerfile + .env.example + 监控栈
└── scripts/          # 部署 / 备份 / CI 守卫 / 油猴转种脚本
```

## 参与贡献

开发约定（质量门禁 / 迁移纪律 / i18n 门禁）见 [CONTRIBUTING.md](CONTRIBUTING.md)。安全问题请走 [SECURITY.md](SECURITY.md) 的私下披露流程，勿开公开 issue。

## 许可证

[GPL-3.0](LICENSE)
