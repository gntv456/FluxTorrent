# FluxTorrent 策划方案

## —— 下一代教育类 PT 站点：从 NexusPHP 到 Rust + Next.js 的全面重构

### 文档信息

| 项目 | 内容 |
| :--- | :--- |
| **版本** | v1.2（v1.0 初稿；v1.1 补全等级表/附录 A 覆盖/29 模块验收基准；v1.2 新增 §8 工程规范） |
| **日期** | 2026-09-08 |
| **状态** | 策划评审稿 |
| **变更记录** | v1.2：新增 §8 工程规范（前后端分离/编码规范/CI 门禁/可维护性），原 §8–§10 顺延为 §9–§11 |
| **上游文档** | [下一代 PT 站点重构方案 v2.0](./下一代%20PT%20站点(FluxTorrent)重构方案（最终详细版）.md)（技术架构总纲） |
| **设计基线** | `UI参考/dev-handoff.html`（开发交接 v44.0，2383 画板） |
| **旧站基线** | `UI参考/real_pages/`（好学站 hxpt.org，NexusPHP 内核，42 个抓取页面） |

## 目录

1. [项目概述](#1-项目概述)
2. [关键决策记录](#2-关键决策记录)
3. [产品定位与用户体验](#3-产品定位与用户体验)
4. [功能规划（PRD）](#4-功能规划prd)
5. [总体架构设计](#5-总体架构设计)
6. [数据模型设计](#6-数据模型设计)
7. [UI 落地指南](#7-ui-落地指南)
8. [工程规范：前后端分离与代码标准](#8-工程规范前后端分离与代码标准)
9. [实施路线图](#9-实施路线图)
10. [风险与应对](#10-风险与应对)
11. [附录](#11-附录)

---

## 1. 项目概述

### 1.1 什么是 FluxTorrent

FluxTorrent 是一个面向**教育资源共享场景**的下一代私有种子站（PT 站点）引擎，由三部分组成：

- **一套完整的产品定义**：覆盖种子管理、经济激励、社区、娱乐玩法的 40+ 功能模块（源自旧站「好学」的成熟运营玩法，经产品化梳理与分期裁剪）；
- **一套全新的技术架构**：Rust 后端 + Torrust Tracker + Next.js 前端 + PostgreSQL + Redis，Docker 一键部署（详见《下一代 PT 站点重构方案 v2.0》，本方案整合并落地之）；
- **一套成品级 UI 设计系统**：「好学」品牌明色童趣风格，覆盖桌面 / 移动 / 平板 / 折叠屏全形态的 2383 个设计画板与完整设计 Token 规范。

一句话定位：**让教育资源的分享像种下一颗种子一样简单、温暖、可持续 —— 用对的技术，承载对的产品。**

### 1.2 重构背景：旧站痛点

旧站「好学」（NexusPHP 内核，PHP 5.6/7.x + MySQL + jQuery）长期运营积累了大量特色玩法（保种区、火花经济、绩效考核、农场游戏等），但技术底座已成为瓶颈：

| 维度 | 现状（NexusPHP） | 痛点 |
| :--- | :--- | :--- |
| **语言/架构** | PHP 单体，Web 与 Tracker 耦合 | 性能天花板低，无法独立扩展，单点故障 |
| **数据库** | MySQL 裸查询 | 种子数增长后列表查询极慢，无分区/覆盖索引 |
| **缓存** | 页面脚注显示 "29 reads/writes of Redis" 的零散缓存 | 无体系化多级缓存，高并发下打穿 DB |
| **前端** | jQuery + 古老模板 | 无响应式，移动端体验差，无折叠屏适配 |
| **功能堆叠** | 40+ 模块以插件/散装 PHP 页面存在 | 插件间无统一架构（plugin-bank、magic 系列、jixiao 等），维护困难 |
| **安全** | 历史漏洞多，MD5 密码体系，手动防注入 | SQL 注入 / XSS 风险长期存在 |
| **部署** | 手工配置 Nginx + PHP-FPM + MySQL | 环境依赖复杂，升级易出错 |

### 1.3 成功指标（继承重构方案 v2.0 量化目标）

| 目标类别 | 量化指标 |
| :--- | :--- |
| **API 吞吐量** | ≥ 140,000 req/s（单节点） |
| **Tracker 处理能力** | ≥ 300,000 announce/s |
| **种子规模支持** | ≥ 1,000,000 种子（旧站当前约 2 万级，目标支撑 50 倍增长） |
| **并发在线用户** | ≥ 10,000 人 |
| **服务器需求** | ≤ 8 核 CPU，≤ 16 GB 内存承载百万种子 + 万人在线 |
| **首屏加载时间** | ≤ 1.5 秒（4G 网络） |
| **Lighthouse 性能评分** | ≥ 95 |
| **多端支持** | Web / 手机 / 平板 / 折叠屏（7 种形态）/ PWA |
| **部署时间** | ≤ 5 分钟（从零到上线，仅需 Docker） |
| **功能对齐** | 旧站 40+ 模块全部有去向（实现 / 明确砍掉 / 换形态），详见附录 A 映射总表 |

### 1.4 范围声明

- 本方案是**策划总纲**：定义做什么（产品）、怎么做（架构与数据）、长什么样（UI 落地）、什么时候做完（路线图）。
- 逐行代码实现、接口级 API 文档、详尽测试用例不在本方案内，属于后续各 Phase 的工程文档。
- 技术架构的完整论证（性能基准、编译优化、监控体系等）以《下一代 PT 站点重构方案 v2.0》为准，本方案引用其结论并补充 PT 业务专属设计。

---

## 2. 关键决策记录

> 本章记录策划过程中的关键取舍，是后续开发的「宪法条款」。变更需评审。

### 2.1 决策一：技术路线 —— Rust + Next.js 全栈重写（推翻设计稿假设）

**背景冲突**：`UI参考/dev-handoff.html` 的技术约束（§1）假设「基于 NexusPHP（PHP 模板）改造，只改模板/样式层，不动业务逻辑」；而 `_doc` 重构方案 v2.0 是 Rust + Next.js 全栈重写。两者互斥。

**决策**：采用 **Rust + Next.js 全栈重写**。理由：

1. 项目代号即为新站（FluxTorrent），非旧站换肤；UI 设计稿中「不动业务逻辑」的前提不存在 —— 旧站业务逻辑本身就在重写范围内。
2. 设计稿的**真正资产**是设计 Token、组件规范、布局规则、2383 画板与验收标准 —— 这些全部是「框架无关」的 HTML/CSS 语言，可 100% 平移到 Tailwind + shadcn/ui 体系（映射表见 §7.1）。
3. 旧站 40+ 模块中大量是散装 PHP 插件（plugin-bank、magic_fram、jixiao 等），换肤路线必须继续背负 PHP 插件架构；重写路线反而可以把这些玩法统一收编进整洁架构（详见 §5.6）。
4. 性能/安全/部署目标（§1.3）在 PHP 路线下不可达成。

**后果与约束**：

- 设计稿 dev-handoff 中「原生 HTML5/CSS3/少量 JS、无框架」「图片走 aka.doubaocdn.com CDN」等实现性条款**作废**，替换为 §7 的对应条款。
- 设计稿中的业务数据口径（§6 数据口径不可变条款）**全部保留**：货币叫「火花」、官种 tag_id=3、种子列表字段集合、银行利率、商店价格、签到奖励、勋章 81 枚、课本 29 科、论坛四区等，作为新系统 PRD 的验收基准值。

### 2.2 决策二：功能分期 —— 核心优先，经济与娱乐分阶段

旧站 40+ 模块一次性实现周期过长（预估 12+ 人月）。按 PT 站点存续的依赖关系分三期：

| 期 | 代号 | 内容 | 判据 |
| :--- | :--- | :--- | :--- |
| **P0（MVP）** | 站点能转 | 认证/邀请、种子全流程、Tracker 计费、促销引擎、评论感谢收藏、RSS、基础个人中心、基础管理后台 | 用户可以完整的「注册→浏览→下载→做种→考核上传量」闭环 |
| **P1（经济与社区）** | 站点活起来 | 火花经济五件套（收益/商店/银行/签到/站免池）、勋章、论坛、短讯、求种/候选/字幕、课本中心、保种区、排行榜、绩效考核、任务、邀请管理 | 经济系统形成循环，社区粘性建立 |
| **P2（玩法与生态）** | 站点好玩 | 农场/刮刮乐/九宫格/五子棋/卡牌合成等娱乐玩法、装扮中心、PWA 推送、开放 API、插件系统 | 差异化体验，开发者生态 |

每期验收标准见 §4 各模块「验收基准」与 §9 路线图里程碑，工程红线见 §8。

### 2.3 决策三：单货币化 —— 火花取代「魔力值 + 火花」双轨

**背景**：旧站存在双货币 —— NexusPHP 原生「魔力值」（Bonus，绩效考核工资以此发放）+ 自定义「火花」（做种挖取、商店消费、银行理财）。两套货币并存导致心智负担与数据口径混乱（新 UI 设计稿明确只保留「火花」）。

**决策**：**新系统唯一货币为「火花」（Spark）**。绩效考核工资、商店、银行、签到、任务奖励等一切经济行为统一以火花结算。旧站存量魔力值按官方折算比例一次性转换为火花（折算比例由运营在迁移前确定，建议 1:1 起评）。

### 2.4 决策四：数据迁移 —— 保数据、不保代码

旧站数据是运营资产，必须迁移；旧站代码一行都不迁移。

- **迁移对象**：用户（含密码重置策略）、种子元数据与 .torrent 文件、论坛帖子、短讯、勋章持有记录、火花/银行/签到等经济存量、保种认领记录、课本库。
- **密码迁移**：旧站密码哈希体系（NexusPHP 系列）无法直接用于 Argon2，采用**首次登录强制改密**方案：导入用户标记 `must_reset_password`，凭用户名 + 旧密码验证通过后设置新密码（旧算法验证器在迁移期临时保留，迁移窗口结束后删除）。
- **迁移窗口**：灰度切换双轨运行 1–2 周（详见 §9 Phase 6 与 §10 风险表）。

### 2.5 技术选型总表（继承 v2.0，标注 PT 专属补充）

| 层级 | 选型 | 版本 | 说明 |
| :--- | :--- | :--- | :--- |
| 边缘网关 | Pingora | 最新稳定版 | TLS 终结、限流、WAF、负载均衡 |
| Web 后端 | Actix-web | 4.5+ | API 服务主框架 |
| Tracker | Torrust Tracker | 2.0+ | **PT 专属**：私有模式，announce 鉴权对接本站用户体系 |
| 数据库 | PostgreSQL | 16+ | 分区表 + 覆盖索引 + JSONB |
| 分布式缓存 | Redis / Valkey | 7.2+ | L2 缓存、会话、异步队列 |
| 进程内缓存 | Moka / DashMap | 0.12+ | L1 热点缓存 |
| 前端框架 | Next.js | 15.3+ | App Router、ISR、PPR |
| UI 组件 | shadcn/ui + Tailwind CSS | 最新 / 4.0+ | 承载「好学」设计系统 |
| 动效 | Framer Motion | 最新 | 替代设计稿的原生 JS 动效 |
| Monorepo | Turborepo + pnpm | 2.0+ | 前后端统一管理 |
| 部署 | Docker Compose | 2.0+ | 一键部署 |
| 可观测 | tracing + OpenTelemetry + Prometheus + Grafana | - | 监控告警 |

**Tracker 补充说明**：Torrust 默认（AGPL-3.0）作为独立服务部署；本站需要的是「私有 Tracker + 用户级上传/下载量统计」，通过 Torrust 的认证 token（passkey）机制对接本站用户表，并通过其事件/回调将 announce 统计**异步批量**写入 PostgreSQL 计费流水（链路设计见 §5.4）。若 Torrust 扩展点无法满足 PT 计费需求，备选方案是基于 Aquatic（纯内存极致性能）或自研轻量 announce 服务（核心逻辑仅 ~3k 行），在 Phase 2 技术验证时定案。

---

## 3. 产品定位与用户体验

### 3.1 产品定位：教育类 PT 社区

FluxTorrent 首个落地站点是「好学」—— 面向**基础教育资源**（学前、小学、初中、职高、高中、教育影音、纪录片）的私有种子社区。资源形态覆盖视频、音频、书籍、文档、笔记、课件、软件、图片八种媒介。

典型用户画像：

- **资源 seekers**：家长与教师，找指定教材版本/年级的资源，看重筛选效率（分类→媒介→年级→教材版本四级筛选）；
- **做种贡献者**：长期保种者，看重火花收益、保种荣誉（勋章/考核）与保种区工具；
- **社区活跃者**：论坛/娱乐玩法参与者，是站点氛围与留存的基石。

### 3.2 品牌：种子 ⇄ 成长

设计稿确立的世界观：**「种子 ⇄ 成长」双关** —— BT 种子播下，资源成长，用户也随等级成长。品牌元素四件套：

| 元素 | 规格 | 使用场景 |
| :--- | :--- | :--- |
| **猫头鹰学士吉祥物** | CDN 素材 `aka.doubaocdn.com/s/HOUNKObnzh` | Logo、登录页、空态、加载动画 |
| **彩色贴纸徽章** | 白描边 + 内阴影 + 微旋转 -2° | 免费/2x/官种/新手/H&R/部编版精品 等标签 |
| **彩虹彩带渐变条** | `linear-gradient(90deg,#FFC93C,#FF7A59,#FF8FC7,#5B6BF5,#2FA8FF)` | 进度条、活动横幅、分隔装饰 |
| **蜡笔字标题** | ZCOOL KuaiLe（站酷快乐体） | H1/H2、品牌语、等级名 |

### 3.3 用户等级：成长豆荚体系（LV0–LV6）

旧站 NexusPHP 用户等级（user / power user / elite …）在新站品牌化包装为**成长豆荚等级**：

| 等级 | 名称 | 视觉 | 定位 |
| :--- | :--- | :--- | :--- |
| LV0 | 种子 | 🌰 | 新注册用户 |
| LV1 | 新芽 | 🌱 | 达到基础分享率/在线时长（对应旧站 Power User 档） |
| LV2 | 幼苗 | 🌿 | 中级分享贡献（对应旧站 Elite 档） |
| LV3 | 小树 | 🌳 | 高级分享贡献（对应旧站 Crazy/Crazy User 档，发布资格线） |
| LV4 | 大树 | 🌲 | 资深贡献者（对应旧站 Extreme/Ultimate 档） |
| LV5 | 开花 | 🌸 | 站点核心（对应旧站 Veteran/Insane 档） |
| LV6 | 硕果 | 🍎 | 最高等级 / VIP 通道 |

> 具体升降级阈值（上传量/分享率/注册天数）由运营按旧站 FAQ「用户等级与升降级」规则换算确定，配置化存储于 `user_classes` 表（§6.2），代码不硬编码；上表新旧档位对应关系仅作迁移对照的默认建议（并入 §9.3 待定项 #4 一并定案）。等级仅是展示与权限层（NexusPHP class 的品牌化包装），权限点（发布资格、保种认领、管理面板可见性）与等级解耦配置。旧站管理侧角色（站长/主管/管理员/总版主/论坛版主/维护开发员/发布员/黑猫警长等 staff 梯队）不属于成长豆荚序列，作为独立的管理角色体系配置于 `user_classes.privileges`。

### 3.4 体验原则

1. **内容不变、布局重排**：多端适配不是做阉割版移动站，同一功能在桌面/移动/折叠屏保持完整字段与能力（设计稿硬性要求：种子列表三端三形态字段顺序一致）。
2. **童趣但不幼稚**：视觉活泼（贴纸/彩带/吉祥物），信息密度与操作效率按成人工具标准（tabular-nums 数字对齐、键盘可操作、对比度 ≥4.5:1）。
3. **数据口径即产品**：火花、官种、保种、免费/2x 促销等运营概念在 UI 文案中全站统一，禁止同义混用（如「魔力」「魔力值」字样在新站全部为「火花」）。
4. **空态有温度**：所有列表空态 = 吉祥物 80px + 蜡笔字标题 + 行动按钮，例：「这里还空空的，去种下第一颗种子吧」。

---

## 4. 功能规划（PRD）

> 模块编号规则：`M-` 前缀。每个模块给出：功能点、关键规则、验收基准。规则与基准值凡标注「旧站口径」的，均取自旧站真实页面梳理，迁移时以此验收。
> 优先级：**P0** = MVP 必需；**P1** = 经济与社区期；**P2** = 玩法与生态期。

### 4.0 模块总览与分期

| 期 | 模块 |
| :--- | :--- |
| **P0** | M01 认证与账户 · M02 种子浏览与发现 · M03 种子详情 · M04 发布 · M05 下载与 Tracker · M06 促销引擎 · M07 评论/感谢/收藏 · M08 RSS · M09 基础个人中心 · M10 基础管理后台 |
| **P1** | M11 火花经济 · M12 签到 · M13 站免池 · M14 勋章 · M15 论坛 · M16 短讯与社交 · M17 求种/候选/字幕 · M18 课本中心 · M19 保种区 · M20 排行榜 · M21 绩效考核 · M22 任务中心 · M23 邀请管理 |
| **P2** | M24 娱乐玩法矩阵 · M25 装扮中心 · M26 PWA 与推送 · M27 开放 API · M28 插件系统 · M29 完整管理后台 |

### 4.1 P0 —— 核心 PT 功能（MVP）

#### M01 认证与账户

- **功能点**：邀请码注册（PT 站准入门槛）、用户名/密码登录、密码 Argon2id 存储、JWT（RS256）+ Session 双通道、找回密码（时效 Token 邮件）、登录限流（5 次/分钟/IP）、用户协议确认。
- **关键规则**：
  - 注册仅凭邀请码，一码一用，注册成功绑定邀请人与被邀请人（供 M23 邀请管理统计）；
  - 存量用户迁移：首次登录凭旧密码验证后强制改密（§2.4）；
  - Session Cookie：HttpOnly + Secure + SameSite=Lax。
- **验收基准**：暴力破解 10 万次尝试全部被限流拦截；密码库中不存在明文/MD5。

#### M02 种子浏览与发现（核心页，UI 优先评审对象）

- **功能点**：种子列表页（新站路由 `/torrents`，旧站 `torrents.php`）。
  - **五行筛选 Chip**（移动端横向滚动）：
    1. 分类：全部/学前教育/小学/初中/职高/高中/教育影音/纪录片（旧站 cat401–410）；
    2. 媒介：视频/音频/书籍/文档/笔记/课件/软件/图片（旧站以音轨字段承载媒介筛选，新站独立字段）；
    3. 年级：幼儿园~高三（旧站 audiocodec2–13 复用，新站独立字段 `grade`）；
    4. 版本：人教/部编/统编/苏教/北师大/外研/沪教（`edition`）；
    5. 标签：免费/2x/官方/古籍/禁转/新手/H&R（`tags` 多选）。
  - 结果头：`共 N 个种子 · 断种 M`；排序 Chip：最新发布/大小/做种数。
  - 游标分页（Keyset，禁 OFFSET）；搜索：标题/副标题/发布者，与/准确模式。
- **关键规则**：
  - 列表行卡片字段集合（数据口径不可变）：分类色块、标题、免费剩余时间徽章、副标题（`教材：xx | 章节：xx`）、标签行（官种/古籍/禁转/发布人/时间）、右列大小/做种（绿）/下载（橙）tabular-nums；
  - 官种 = tag_id **3**（旧站口径，保留数值兼容）；
  - 用户视图切换：全部/我发布的/我做种的/我下载的/我完成的/收藏的。
- **验收基准**：百万种子下任意筛选组合 P99 < 50 ms（EXPLAIN ANALYZE 验证走覆盖索引）；三端字段顺序一致（对照设计稿画板 B3 / C2 / 折叠屏系列）。

#### M03 种子详情

- **功能点**：详情页 `/torrent/[id]`。字段：标题+副标题、简介（富文本/BBCode 迁移）、技术信息、NFO 预览、文件列表、海报（IMDb/pt_gen 抓取信息区保留可选项）、大小/添加时间/做种者/下载者/完成数/做种体积、置顶(stick)与官种标记、下载按钮、字幕关联区。
- **关键规则**：权限按钮（编辑/删除）按角色渲染；举报入口常驻；H&R 标记的种子在详情页展示豁免/追责状态。
- **验收基准**：对照设计稿详情页画板逐块验收；旧站 details 页全部信息字段有对应呈现。

#### M04 发布

- **功能点**：发布页 `/upload`。表单：.torrent 文件、标题、副标题、简介（编辑器）、技术信息、NFO、分类、媒介、年级、教材版本、海报 URL、匿名发布选项（uplver）、发布前规则确认。
- **关键规则**：
  - 服务端重新解析 .torrent（Bencode 解析置于 Rust 端，前端 WASM 解析仅作预览）；
  - 发布即进入待审核队列（approval_status=0），管理员审核通过（1）后公开可见，拒绝（2）附理由；
  - 发布奖励（P1 起接经济系统：+1000 火花，旧站口径）。
- **验收基准**：畸形/私种/外链 tracker 的 .torrent 被拒收；三模式（普通/候选/求种）表单复用同一路由组件，参数化差异字段。

#### M05 下载与 Tracker 集成（PT 核心）

- **功能点**：
  - 下载 .torrent：服务端注入本站 announce URL 与用户 passkey，动态生成；
  - Tracker：Torrust 私有模式，UDP + HTTP announce，passkey 鉴权；
  - 用户上传/下载量统计、分享率计算；
  - H&R（Hit & Run）追踪：下载后未达保种时长/比例的记录与追责状态。
- **关键规则**：
  - 计费：announce 事件 → 异步批量入 PG 计费流水（§5.4 链路），不做同步扣减；
  - 促销状态（免费/2x 等）由促销引擎（M06）统一裁决，announce 侧只读快照；
  - passkey 可由用户重置，重置后旧 passkey 立即失效；
  - announce_interval = 300s，peer_timeout = 60s。
- **验收基准**：≥ 300,000 announce/s 基准达标；统计流水与站点报表误差 < 0.1%；断电/重启后流水无丢失（WAL + 批量提交验证）。

#### M06 促销引擎

- **功能点**：种子级促销类型（旧站 spstate 全集）：普通/免费(Free)/2X/2X免费/50%/2X 50%/30%；全局促销开关（如站免池触发的全站双免，M13）；促销定时置位/到期（pos_state_until）。
- **关键规则**：促销裁决为单一事实源（DB 表 + L1 缓存快照，TTL ≤ 60s）；列表页徽章（免费=薄荷绿、2x=珊瑚橙）与下载计费必须同源。
- **验收基准**：促销置位后 ≤ 60s 全站生效；到期自动回收，无残留徽章。

#### M07 评论 / 感谢 / 收藏

- **功能点**：详情页评论（分页、BBCode）、感谢（感谢者列表、一人一次）、收藏/取消收藏（列表页「收藏的」视图联动）。
- **关键规则**：感谢与评论区分事件流（感谢不发帖）；删除种子时级联清理策略：评论保留归档、收藏清除。
- **验收基准**：与旧站数据等量迁移；感谢不可重复、不可撤销。

#### M08 RSS

- **功能点**：自定义 RSS（旧站 getrss.php 对应）：按分类/标签/官种/保种/种子状态/标题关键字组合订阅，生成带用户 token 的订阅链接。
- **验收基准**：主流 RSS 阅读器可订阅；token 泄露可单独重置。

#### M09 基础个人中心

- **功能点**：我的数据（上传/下载/分享率/做种列表/下载列表，旧站 getusertorrentlist + my_data_stats 承载）、passkey 管理、安全设置（改密/2FA 预留）、头像与个人资料。用户公开主页 `/user/[id]`（等级徽章、统计、勋章墙占位）。
- **验收基准**：对照设计稿「我的数据/用户详情」画板；P0 无经济字段时对应卡片隐藏不报错。

#### M10 基础管理后台

- **功能点**（P0 子集，完整版为 M29）：种子审核队列（通过/拒绝+理由）、置顶/禁种、举报处理、用户管理（禁言/封号/等级调整）、审计日志（防篡改 HMAC 链）、站点统计概览（用户数/种子数/做种数/做种体积 —— 旧站首页数据区块口径）。
- **验收基准**：敏感操作（删种/封号）二次验证；所有管理操作留审计日志。

### 4.2 P1 —— 经济与社区

#### M11 火花经济

站点唯一货币「火花」，五条获取渠道 + 一条消费主线，全部经 `spark_ledger` 流水表记账（§6.2），余额 = 流水聚合 + 冗余快照字段。

- **功能点**：我的火花页（余额/收益明细/收益因子说明）、火花商店（商品购买）、做种收益小时结算引擎、宣传链接计费（promotionlink）。

- **获取渠道与规则**（旧站 mybonusmine 口径）：

| 渠道 | 规则 |
| :--- | :--- |
| 做种收益 | 每小时结算：基础火花 + 额外加成（做种数/体积/时间加权）；捐赠者 2 倍收益 |
| 发布种子 | +1000 / 枚 |
| 发字幕 | +5 / 条 |
| 论坛行为 | 发主题 +2、回帖 +1、评论 +1 |
| 投票 | 候选投票 +1、调查投票 +20、趣味盒投票 +1 |
| 宣传链接 | 点击 +1 / IP |

- **消费主线 —— 火花商店**（旧站 mybonus 商品全集）：1/5/10/100 GB 上传量、邀请名额、临时邀请名额、自定义头衔、贵宾待遇、APP 专属 VIP(30天)、赠送火花、15 天去广告、补签卡、彩虹 ID、改名卡、头像框、彩虹用户名样式、动态头像、慈善捐赠。
- **关键规则**：删种收回发布所得火花；所有商品购买走同一交易管线（幂等 + 流水）；商店价格表为验收基准值（取旧站现价，配置化）。
- **验收基准**：任意账户余额 = 流水重放结果；并发购买无超扣；收益因子徽标（1.03x/5x/0.1x）可点击展开说明（设计稿交互要求）。

#### M12 签到（attendance）

- **功能点**：签到日历页（月历视图/连签进度）、每日签到、补签（补签卡）、签到排行榜。
- **规则**（旧站口径）：首次签到 +10 火花；连续签到每日 +5（单日上限 1000）；连签 10/20/30 天额外 +200/+500/+1000；补签卡商店购买。
- **验收基准**：跨时区/跨日边界不重复签到；补签不改变「连签额外奖励」的原始连续计数判定（按运营规则二选一，开发前定案）。

#### M13 站免池（magic_pool → 全局促销池）

- **功能点**：站免池页（当月进度条/目标线/说明文）、捐赠火花、捐赠排行榜（本月）、我的捐赠记录。
- **规则**（旧站口径）：当月累计捐赠达 **200 万火花**，次月 1–3 号自动开启全局双倍免费促销。
- **联动**：触发后写促销引擎（M06）全局促销窗口，全站种子 Free+2X。
- **验收基准**：达标判定与自动开启为原子任务；进度条数字与汇总流水一致。

#### M14 勋章

- **功能点**：勋章图鉴（81 枚基准值，价格/名称排序筛选）、购买、赠送（弹窗流程）、佩戴（用户名旁展示）、勋章墙、限定勋章（开站勋章/开学先锋/节气勋章等运营活动）、卡牌合成联动（P2）。
- **验收基准**：图鉴 81 枚口径一致；佩戴位与用户卡片展示同步。

#### M15 论坛

- **功能点**：版块 → 主题 → 帖子三级；版主管理（置顶/锁定/移动/删帖）；今日新帖统计；BBCode 富文本；发帖/回帖火花奖励（M11）。
- **迁移口径**：旧站 9 个版块全量迁移；新站设计稿「论坛四区」为导航重组，以迁移数据为准保留 9 版块再按新导航分组。
- **验收基准**：旧帖楼层/时间/作者关系不丢失；搜索可用。

#### M16 短讯与社交

- **功能点**：站内短讯（收件箱/发件箱/批量操作）；给管理组发短讯（contactstaff，颜色 BBCode 选择器）；好友/黑名单/待审核三分组（friends）。
- **验收基准**：旧站专用信箱（staffbox/cheaterbox/reportbox）在新后台（M10/M29）中有对应处理队列，短讯页不承载管理职能。

#### M17 求种 / 候选 / 字幕

- **功能点**：
  - **求种区**：发布求种（`/upload?mode=request` 同表单复用）、求种列表、应种交付（交付后自动关联种子、求种者确认、悬赏火花转移）；
  - **候选区**：资源候选投票（1 火花/票，旧站口径），票数达标转正为正式种子（与官种机制联动）；
  - **字幕区**：上传字幕（+5 火花）、字幕与种子关联、下载。
- **验收基准**：三模式与普通发布共用表单组件（M04）；候选转正流程留痕。

#### M18 课本中心

- **功能点**：独立于种子的课本库（textbooks）：出版社/科目/版本/年级/册次五维组织，下载量统计，种子发布时可关联课本条目（列表页副标题「教材：xx | 章节：xx」的数据源），管理端维护课本条目。
- **口径**：29 科、多出版社版本（人教/部编/统编/苏教/北师大/冀教/北京版/五四制等，旧站口径全量迁移）。
- **验收基准**：课本关联的种子列表筛选（版本 Chip）数据来自课本库而非自由文本。

#### M19 保种区

- **功能点**：保种专区列表（免费下载标识）、官方保种/全部保种/保种中统计、保种认领、与绩效考核联动（月保种详情）。
- **规则**（旧站口径）：保种区种子**免费下载**；做种人数 **> 7** 后自动移出保种区；移出后免费状态延续 **3 天**。
- **验收基准**：移出判定为定时任务 + 事件双触发；免费延续到期自动回收（促销引擎实现）。

#### M20 排行榜（topten）

- **功能点**：用户榜（上传者/下载者/种子数/做种体积/最快上传/最快下载/最佳分享率/最差分享率）、社区榜、其它榜；Top 21/100/250；四种展示布局（Table/Progress/Compact/Cards）。
- **验收基准**：缓存聚合（L1 5 分钟 TTL），底层查询不扫描全表。

#### M21 绩效考核（jixiao）

- **功能点**：考核类型（保种员 5T 版/保种员/发布员/转种员/维护开发员/主管等）；考核期指标（上传量/下载量/保种体积/保种种子数/当月时长/操作总数）；最低要求线；基础/奖励工资计算；领取记录；管理端配置（jixiao_admin 对应）。
- **规则**（旧站口径）：工资以火花发放（§2.3 单货币化）；低于最低要求不得基本工资；达标月数累计。
- **验收基准**：指标采集全部来自系统流水（announce 统计、保种表、管理操作日志），零手工填报；发放幂等（重复领取拒绝）。

#### M22 任务中心（task）

- **功能点**：限时任务表（名称/指标/起止时间/目标用户等级/奖励与罚扣火花/认领人数上限/描述）、领取、完成判定、超时罚扣。
- **口径**：旧站「认领人数 23/100 限流」等约束保留为配置。
- **验收基准**：任务到期结算为幂等批任务；罚扣不为负余额（保底 0）。

#### M23 邀请管理

- **功能点**：邀请名额管理（注册赠 + 商店购买 + 临时邀请）、发送邀请（invite.php?id=N 对应）、邀请记录（邀请人/被邀请人/注册状态）、论坛「邀请发放楼」文化承载（论坛板块）。
- **验收基准**：邀请码生命周期（生成/发送/使用/过期/撤销）全链路可查。

### 4.3 P2 —— 玩法与生态

#### M24 娱乐玩法矩阵

- **功能点**：旧站小游戏群统一收编为玩法模块，全部走火花结算，统一接 M11 交易管线（统一赔率/限额/风控配置），逐个独立上线；管理端统一配置（奖品/概率/价格，对应旧站 jgg_admin 等散装后台）。
- **规则要点**（旧站口径）：

| 玩法 | 旧站入口 | 规则要点 |
| :--- | :--- | :--- |
| 好学农场 | magic_fram.php | 农作物 5 天有效期；20% 概率双倍收获；一键收获；市场价格每日 0/4/8/12/16/20 点刷新，波动 ±50%；种植区/仓库/菜市场 |
| 刮刮乐 | magic_scratch.php | 即开型 |
| 九宫格抽奖 | jgg.php | 管理端配置奖品与概率（jgg_admin） |
| 猜大小 | bigsmall.php | 即开型 |
| 五子棋 | wuziqi.php | 对战 |
| 卡牌合成 | medal_synthesis.php | 与勋章系统联动 |
| 水浒传卡牌游戏 | 首页活动 | 收集英雄卡牌、挑战塔楼怪物（活动型，是否复刻由运营定） |
| 趣味盒投票 | funvote / magic.php | 投票 +1 火花 |
| PT Patronus 守护神 | ptlife.php | 外部联动（接入密钥/API 签名），P2 评估是否保留 |

- **验收基准**：每玩法接入统一交易管线（无直改余额的 UPDATE）；全局/单用户损失限额风控生效；赔率与价格配置热更新。

#### M25 装扮中心

- **功能点**：用户装扮购买/佩戴（头像框/用户名样式/动态头像等，与商店商品打通）；管理端配置中心（zhuangshi_admin 对应）。
- **验收基准**：装扮购买复用 M11 商品管线；佩戴状态在论坛/评论区/用户卡片全站即时生效（缓存失效验证）。

#### M26 PWA 与推送

- **功能点**：next-pwa：离线壳、添加到主屏幕、Web Push（促销开启/求种应答/短讯到达，用户可订阅类型）。
- **验收基准**：Lighthouse PWA 检查可安装（installable）；推送订阅-投递-退订闭环；弱网下壳页面可打开。

#### M27 开放 API

- **功能点**：RESTful + utoipa/OpenAPI 文档、版本化（/v1/）、OAuth2 或 API Token 认证、独立限流；优先支撑第三方工具（RSS 增强客户端、移动壳 App、图床/辅种工具）。
- **验收基准**：OpenAPI 文档由代码注解自动生成并与实现同步（CI 校验 drift）；Token 可撤销、限流可配。

#### M28 插件系统

- **功能点**：借鉴 NexusPHP Hook 思想：`Plugin` Trait（on_torrent_upload / on_user_login / register_routes），PluginManager 装配；M24 各玩法是插件化改造的第一批试验田。
- **验收基准**：示例插件（如「发布自动置顶官种」）仅靠注册 Hook 实现，不侵入领域层代码；插件启停不影响核心链路。

#### M29 完整管理后台

- **功能点**：设计稿 66 页管理后台全量落地（桌面优先）：staffpanel、考核后台（考核类型/工资规则）、九宫格配置、装扮管理、课本管理、勋章管理（81 枚 CRUD）、任务配置、保种管理、举报/作弊箱工作流、机密日志分级（普通/机密两档，对应旧站 log.php）。
- **验收基准**：管理后台独立 admin scope 会话 + 敏感操作二次验证（§5.7）；全部写操作进审计日志；设计稿 66 页逐页对照无缺页（可与运营按页裁剪，裁剪需记录）。

---

## 5. 总体架构设计

### 5.1 分层架构总览

继承重构方案 v2.0 的分层架构（客户端层 → 边缘加速与安全层 → Rust 后端核心层 → 数据与缓存层 → 部署与运维层，CI/CD 贯穿），架构图见上游文档 §3，此处不重复绘制。本方案补充 PT 业务专属的三条链路设计。

### 5.2 服务拆分

| 服务 | 进程 | 职责 | 扩展性 |
| :--- | :--- | :--- | :--- |
| `web` | Next.js 15 | SSR/ISR 页面、RSC 数据预取、静态资源 | 无状态，水平扩展 |
| `api` | Actix-web | 全部业务 REST API（认证、种子、经济、社区、管理） | 无状态，水平扩展 |
| `tracker` | Torrust（或备选方案，§2.5） | UDP/HTTP announce、peer 维护 | 独立扩展，内存热数据 |
| `worker` | Tokio 异步任务组 | 计费落库、促销到期、签到结算、保种移出、考核指标聚合、任务结算、通知投递 | 按队列分区扩展 |
| `gateway` | Pingora | TLS、限流、WAF、路由、负载均衡 | 独立扩展 |

Monorepo 目录结构沿用上游文档 §7（`apps/web`、`apps/api`、`apps/tracker`、`packages/*`），**新增** `apps/worker`（异步任务组）与 `packages/domain-types`（前后端共享的业务类型定义：促销状态枚举、火花流水类型、种子标签枚举等，保证计费口径前后端一致）。

### 5.3 多级缓存与数据流

```
请求 → Pingora(边缘/限流) → Next.js(ISR 页面缓存)
                            → Actix API
                                ├─ L1 Moka(纳秒级: 热点种子/会话/权限/促销快照 TTL≤60s)
                                ├─ L2 Redis(毫秒级: 全局状态/API响应/排行聚合)
                                └─ PostgreSQL(分区表+覆盖索引)
```

缓存纪律：

- **促销快照**、**站点统计**、**排行榜**是三大高频读，全部 L1 化；写路径经 worker 异步刷新，禁止请求路径内同步重算；
- 缓存穿透防护：布隆过滤器 + 空值缓存 + 互斥锁回填（上游文档 §13）；
- 经济类数据（火花余额）**只以 DB 流水为准**，缓存仅作展示加速，冲突时以 DB 为准。

### 5.4 announce 计费统计链路（PT 专属，关键链路）

```
BT客户端 ──announce──▶ Torrust Tracker
                         │ ① passkey 鉴权(内存 passkey→user_id 映射, Redis 同步)
                         │ ② peer 状态更新(DashMap 内存)
                         │ ③ 立即返回 compact 响应（不阻塞计费）
                         │ ④ announce 事件 → Redis Stream (fire-and-forget)
                         ▼
                    worker 消费组
                         │ ⑤ 聚合窗口(如 60s/用户/种子)
                         │ ⑥ 批量 upsert: snatches(做种/下载状态) + traffic_ledger(上/下行增量流水)
                         │ ⑦ 促销快照裁决: 免费→downloaded 不增, 2x→uploaded ×2
                         ▼
                    PostgreSQL (分区流水表) ──▶ 站点报表/考核指标/排行榜(定时聚合)
```

设计要点：

1. **announce 路径零 DB 依赖**：鉴权与 peer 维护全内存/Redis，保证 30 万 announce/s 目标；
2. **计费最终一致**：Redis Stream 持久化 + 消费组 ack，断电恢复不丢事件；流水表按月分区，聚合任务滚动物化；
3. **促销裁决单点**：worker 侧从促销引擎取快照，杜绝「列表页显示免费但计费照扣」类不一致；
4. **H&R 判定**：worker 在 snatch 完成事件后按规则（保种时长/比例阈值，运营配置）打标，供详情页与追责队列使用。

### 5.5 促销引擎（PT 专属）

- **数据**：`promotions` 表（作用域：单种子 / 全站；类型：Free/2X/2XFree/50%/2X50%/30%；起止时间；来源：手动/站免池/保种延续/任务奖励）；
- **裁决**：任意时刻种子的有效促销 = 单种子促销覆盖全站促销，取「折扣最大」者（函数式纯计算，可单测穷举）；
- **生效**：写表 → 失效 L1 缓存键 → worker 60s 内刷新快照；到期由 worker 定时扫描回收；
- **审计**：全部促销变更进审计日志（谁/何时/为何 —— 站免池或手动）。

### 5.6 业务模块的架构归位（收编旧站散装插件）

旧站每个「插件」（bank/magic 系列/jixiao/task）都是独立 PHP 入口，新系统统一归位到整洁架构（上游文档 §5.6.1）：

```
apps/api/src/
├── domain/                 # 领域层（纯业务规则，零框架依赖）
│   ├── torrent/            # 种子、促销裁决、H&R
│   ├── accounting/         # 火花账本、银行利息、商店交易（幂等管线）
│   ├── seeding/            # 保种、考核指标、排行榜聚合
│   ├── community/          # 论坛、短讯、求种候选字幕
│   └── gamification/       # 勋章、任务、签到、农场等玩法（P2 插件化）
├── infrastructure/         # db(sqlx) / cache(redis,moka) / http(actix) / tracker(事件对接)
└── presentation/           # handlers / dto / utoipa 注解
```

约束：玩法模块（M24）一律通过 `accounting` 的交易管线动账，禁止自行 UPDATE 余额；领域层事件（`TorrentUploaded`、`UserSeedingMilestone` 等）经内部事件总线发布，经济/勋章/任务模块订阅解耦 —— 这是「发布 +1000 火花」「保种里程碑发勋章」类联动不需要硬编码互相调用的机制保障。

### 5.7 安全架构

完整清单见上游文档 §11（内存安全/参数化查询/Argon2/JWT RS256/TLS1.3/HSTS/安全响应头/cargo-audit/限流/防篡改审计日志/最小权限）。PT 业务补充：

- **passkey 即凭证**：泄露可自助重置；接口层对 passkey 相关操作加二次验证；
- **下载链接防爬**：.torrent 动态生成 + 短时效签名 URL；
- **经济风控**：火花交易管线内置单用户速率限额、异常流水告警（如 1 小时内收益超过历史 P99），对 M24 赌博类玩法设全局 loss-limit 配置；
- **管理端**：独立于前台会话（admin scope），敏感操作二次验证 + 审计。

---

## 6. 数据模型设计

### 6.1 建模原则

1. **PostgreSQL 16**，所有金额/流量用 `BIGINT`（字节、火花均以最小单位整数存储，禁止浮点）；
2. **分区**：大表按时间 RANGE 分区 —— `traffic_ledger`（计费流水）、`spark_ledger`（火花流水）、`announce_log`（如保留原始事件）、`posts`（可选）按月分区；
3. **覆盖索引 + 游标分页**：列表查询一律 `WHERE cursor > $1 ORDER BY id LIMIT n`，列表字段集建覆盖索引；
4. **流水即事实**：火花/流量余额是冗余快照，任何对账以流水重放为准；
5. **枚举配置化**：分类/年级/版本/等级/勋章/商店商品存表，代码只认 ID（旧站把年级塞在 audiocodec 字段的教训）。

### 6.2 核心表清单

#### 身份与权限

```sql
users (
  id BIGINT PK, username CITEXT UNIQUE, email CITEXT UNIQUE,
  pass_hash TEXT,                      -- Argon2id
  passkey CHAR(32) UNIQUE,             -- tracker 凭证
  class_id INT REF user_classes,       -- 成长豆荚等级 LV0-6
  uploaded BIGINT DEFAULT 0, downloaded BIGINT DEFAULT 0,   -- 快照, 权威在 traffic_ledger
  title TEXT, avatar_url TEXT,
  invited_by BIGINT REF users, must_reset_password BOOL DEFAULT false,
  donor BOOL,                          -- 捐赠者(收益 2x)
  status SMALLINT, created_at TIMESTAMPTZ, last_seen_at TIMESTAMPTZ
)
user_classes ( id PK, name, pod_level INT, min_uploaded BIGINT, min_ratio NUMERIC,
               min_age_days INT, privileges JSONB )        -- 权限点配置化
invites ( id PK, inviter_id REF users, code CHAR(32) UNIQUE,
          status SMALLINT, used_by REF users NULL, expires_at TIMESTAMPTZ )
```

#### 种子域

```sql
torrents (
  id BIGINT PK, info_hash CHAR(40) UNIQUE,
  name TEXT, small_descr TEXT,          -- 副标题 "教材：xx | 章节：xx"
  descr TEXT, technical_info TEXT, nfo TEXT,
  category_id INT REF categories,       -- 8 分类(旧站 cat401-410 迁移)
  medium_id INT REF media,              -- 8 媒介(新独立字段)
  grade_id INT REF grades,              -- 年级(旧站 audiocodec 复用→独立)
  edition_id INT REF editions,          -- 教材版本(人教/部编/…)
  textbook_id BIGINT REF textbooks NULL,-- 关联课本条目(M18)
  owner_id REF users, anonymous BOOL,   -- uplver
  size BIGINT, numfiles INT,
  approval_status SMALLINT DEFAULT 0,   -- 0待审 1通过 2拒绝
  sticky BOOL, official_tag BOOL,       -- official_tag 即旧站 tag_id=3
  hr_policy JSONB NULL,                 -- H&R 规则与状态
  created_at TIMESTAMPTZ, mtime TIMESTAMPTZ
) PARTITION BY RANGE (created_at);      -- 按月/季分区

-- 覆盖索引(列表页专用, 上游文档 §6.3.1)
CREATE INDEX idx_torrents_list ON torrents
  (created_at DESC, id, name, small_descr, size, category_id,
   seeders, leechers, times_completed, promotion_cache);

tags ( torrent_id, tag_id )             -- 多标签: 免费/2x/官方/古籍/禁转/新手/H&R
files ( torrent_id, file_index, path TEXT, size BIGINT )   -- PK(torrent_id,file_index)
comments ( id PK, torrent_id, user_id, body TEXT, created_at, edited_at )
thanks   ( torrent_id, user_id, created_at, PRIMARY KEY(torrent_id,user_id) )
bookmarks( user_id, torrent_id, created_at, PRIMARY KEY(user_id,torrent_id) )
snatches ( user_id, torrent_id, uploaded BIGINT, downloaded BIGINT,
           seeded_seconds INT, completed_at TIMESTAMPTZ NULL,
           leeching BOOL, seeding BOOL, hr_flag BOOL,
           PRIMARY KEY(user_id, torrent_id) )
subtitles ( id PK, torrent_id, user_id, title, file_ref, downloads INT, created_at )
```

#### 促销与计费

```sql
promotions ( id PK, scope ENUM('torrent','global'), torrent_id NULL,
             kind ENUM('free','x2','x2free','half','x2half','p30'),
             starts_at, ends_at, source ENUM('manual','magic_pool','preserve_grace','task'),
             created_by, created_at )
traffic_ledger (                        -- announce 计费流水, 按月分区
  id BIGINT, user_id, torrent_id, delta_up BIGINT, delta_down BIGINT,
  promotion_kind SMALLINT, window_start TIMESTAMPTZ, ingested_at TIMESTAMPTZ,
  PRIMARY KEY (user_id, id)
) PARTITION BY RANGE (window_start)
```

#### 经济域（P1）

```sql
spark_ledger (                          -- 火花流水, 按月分区, 唯一事实源
  id BIGINT, user_id, amount BIGINT,    -- 正收入/负支出
  kind TEXT,                            -- seeding_reward/upload/subtitle/forum/vote/
                                        -- shop/bank_deposit/bank_withdraw/attendance/
                                        -- pool_donate/task_reward/task_penalty/game/...
  ref_type TEXT, ref_id BIGINT, idempotency_key TEXT UNIQUE,
  balance_after BIGINT, created_at TIMESTAMPTZ,
  PRIMARY KEY (user_id, id)
) PARTITION BY RANGE (created_at)

bank_deposits ( id PK, user_id, amount BIGINT, term_days INT,   -- 7/30/90/180/365
                rate NUMERIC, interest BIGINT, status SMALLINT,
                start_at, maturity_at, settled_at )
shop_items ( id PK, name, kind, price BIGINT, config JSONB, active BOOL )
shop_orders ( id PK, user_id, item_id, price BIGINT, idempotency_key UNIQUE,
              config_snapshot JSONB, created_at )
magic_pool ( month CHAR(7), donated_total BIGINT, goal BIGINT DEFAULT 2000000,
             promo_started BOOL )
pool_donations ( id PK, user_id, amount, month, created_at )
attendance ( user_id, date DATE, streak INT, reward BIGINT,
             makeup BOOL, PRIMARY KEY(user_id, date) )
```

#### 社区域（P1）

```sql
forums ( id PK, name, descr, group_id INT, min_class INT, moderators BIGINT[] )
topics ( id PK, forum_id, user_id, title, sticky BOOL, locked BOOL,
         views INT, created_at, last_post_at )
posts  ( id BIGINT, topic_id, user_id, body TEXT, created_at, edited_at,
         PRIMARY KEY(topic_id, id) ) PARTITION BY RANGE (created_at)
messages ( id PK, sender_id, receiver_id, subject, body, read_at, created_at )
friendships ( user_id, friend_id, list ENUM('friend','black','pending'),
              PRIMARY KEY(user_id, friend_id) )
requests ( id PK, user_id, title, descr, bounty BIGINT,             -- 求种
           fulfilled_torrent_id NULL, status SMALLINT, created_at )
offers   ( id PK, user_id, torrent_id, votes INT, promoted BOOL,    -- 候选
           created_at )
offer_votes ( offer_id, user_id, cost BIGINT, PRIMARY KEY(offer_id,user_id) )
textbooks ( id PK, subject_id, edition_id, grade_id, volume TEXT,
            publisher TEXT, downloads INT )                         -- 29 科
```

#### 保种 / 考核 / 任务 / 勋章（P1）

```sql
seed_preserve ( torrent_id PK, claimed_by REF users NULL, claimed_at,
                seeders INT, exited_at TIMESTAMPTZ NULL,
                exit_reason ENUM('seeders_gt_7','manual') )          -- >7 移出
jixiao_types ( id PK, name, metrics JSONB, base_pay BIGINT,          -- 考核类型配置
               min_requirements JSONB, bonus_rules JSONB )
jixiao_claims ( id PK, user_id, type_id, period CHAR(7), amount BIGINT,
               metrics_snapshot JSONB, claimed_at,
               UNIQUE(user_id, type_id, period) )                    -- 幂等
tasks ( id PK, name, metric JSONB, starts_at, ends_at,
        target_class INT, reward BIGINT, penalty BIGINT, claim_limit INT )
task_claims ( id PK, task_id, user_id, status SMALLINT, settled_at,
              UNIQUE(task_id, user_id) )
medals ( id PK, name, price BIGINT, rarity, limited BOOL, asset_ref, created_at ) -- 81 枚
user_medals ( user_id, medal_id, source ENUM('buy','gift','event'),
              wearing BOOL, PRIMARY KEY(user_id, medal_id) )
```

#### 管理与审计

```sql
reports ( id PK, reporter_id, ref_type, ref_id, reason, status SMALLINT,
          handled_by, handled_at, created_at )       -- 举报(种子/用户/帖子)
audit_log ( id BIGINT, actor_id, action TEXT, ref JSONB, ip INET,
            prev_hash BYTEA, self_hash BYTEA, created_at )   -- HMAC 链防篡改
```

### 6.3 旧站 MySQL → 新 schema 迁移映射（摘要）

| 旧站实体 | 新表 | 处理要点 |
| :--- | :--- | :--- |
| users（MD5 系哈希） | users | 哈希不迁移，置 `must_reset_password`，首次登录验证旧密码后重置（§2.4） |
| torrents / files | torrents / files | info_hash 重算校验；年级从 audiocodec 反解为 grade_id；媒介从音轨字段归位 medium_id |
| tags / 官种 tag_id=3 | tags / official_tag | 官种双写（bool + tag 行），保证列表筛选与详情标识同源 |
| bonus（魔力+火花混合） | spark_ledger | 存量余额折算为单一火花（§2.3），期初流水 `kind=migration_initial` |
| bank 存款（term_days 7/30/90/180/365） | bank_deposits | 原样迁移，利率快照保留 |
| attendance / farm / 勋章 / 邀请 / 好友 / 短讯 / 论坛 9 版块 | 对应新表 | 全量迁移，ID 映射表留存可回溯 |
| seed_preserve 认领 | seed_preserve | 迁移时重算当时 seeders，立即执行一次「>7 移出」规则对齐 |
| jixiao 配置与领取记录 | jixiao_types / jixiao_claims | 历史领取记录只读归档，新考核期起用新引擎 |
| 课本库 | textbooks | 版本/科目规整为枚举表 |

迁移工具为一次性 Rust CLI（`apps/tools/migrator`）：读取旧 MySQL → 转换 → 写入新 PG → 校验（行数、求和、抽样比对）→ 输出报告。**全量演练 ≥ 2 次**后才允许真实切换。

---

## 7. UI 落地指南

> 本章把设计稿（dev-handoff.html v44.0 + 主册 1754 画板 + 折叠屏扩充册 629 画板）的规范翻译为 Next.js + Tailwind + shadcn/ui 实现约定。画板对照以设计稿 §5「28 模块 × 画板编号映射」为准。

### 7.1 设计 Token → Tailwind 主题映射

设计稿 §2 规定「禁止硬编码颜色」，全部以 CSS 变量承载，与 Tailwind 4 `@theme` 天然契合，**原样移植**：

```css
@theme {
  /* 品牌色 */
  --color-sky:   #2FA8FF;  /* 主色: 主按钮/链接/选中/进度条/导航高亮 */
  --color-sun:   #FFC93C;  /* 强调: 等级/星星/活动横幅 */
  --color-coral: #FF7A59;  /* 行动: 发布/下载/删除/警示 */
  --color-mint:  #2FBF9B;  /* 成长: 做种/免费/成功/已完成 */
  --color-candy: #FF8FC7;  /* 童趣: 新手引导/学前教育 */
  --color-indigo:#5B6BF5;  /* 深辅助: Hero 渐变/管理后台 */
  /* 中性 */
  --color-cloud: #F5FAFF;  /* 页面底色(非纯白) */
  --color-ink:   #1F2A44;  /* 正文(非纯黑)/深色顶栏 */
  --color-sub:   #93A1BC;  /* 辅助文字 */
  --color-line:  #E3EDF7;  /* 分隔线 */
  /* 扩展(主册画板) */
  --color-sky-deep: #1E8AE8;
  --color-sky-soft: #E8F4FF;  /* 各色 -soft 浅底系列 */
  --color-cream:    #FFFDF6;
  /* 状态 */
  --color-success: #2FBF9B; --color-warning: #FFC93C;
  --color-danger:  #FF6B6B; --color-info:    #2FA8FF;
  /* 圆角 */
  --radius-xl: 24px; --radius-lg: 20px; --radius-md: 14px; --radius-sm: 10px;
  /* 字体 */
  --font-display: "ZCOOL KuaiLe", "Noto Sans SC", sans-serif;  /* H1/H2/品牌/等级名 */
  --font-body:    "Noto Sans SC", "PingFang SC", "Microsoft YaHei", sans-serif;
  --font-num:     "Noto Sans SC", sans-serif; /* 数字加 900 + tabular-nums 工具类 */
}
```

- 阴影：卡片 `0 8px 24px rgba(31,42,68,.08)`；悬浮 `0 4px 18px rgba(47,168,255,.08)`（注册为 `--shadow-card` / `--shadow-hover`）；
- 字号阶梯：H1 40–46 / H2 24–30 / H3 15–17 / 正文 13–14 / 辅助 11–12.5 px，移动端整体降一级；
- 字体自托管（`next/font/local`），不依赖设计稿的飞书镜像外链；
- 明色主题为唯一主题（设计稿无暗色模式），`next-themes` 仅预留切换能力不开放 UI。

### 7.2 技术栈替换对照表（设计稿条款 → 新实现）

| 设计稿条款（NexusPHP 假设） | FluxTorrent 实现 |
| :--- | :--- |
| 原生 HTML/CSS + 少量 JS，无框架 | Next.js 15 App Router + RSC，动效 Framer Motion |
| 内联 SVG 图标（viewBox 0 0 24 24） | lucide-react 统一图标库（缺失图标才手绘内联 SVG） |
| 图片走 aka.doubaocdn.com CDN 直引 | 吉祥物/封面等品牌素材**下载入库**随版本发布（消灭外部 CDN 单点），用户上传图走本站图床/对象存储 + Next Image 优化（AVIF/WebP） |
| 纯 CSS 动画类（tk-float/tk-sway/tk-pop 等） | 简单循环动画保留纯 CSS（性能更好）；交互动效（弹窗/列表入场/数字滚动）用 Framer Motion；全部尊重 `prefers-reduced-motion` |
| 基于模板的页面组装 | RSC 服务端组件 + shadcn/ui 客户端交互组件 |
| 样式即最终产物 | Tailwind Token 化，同一套 Token 输出设计规范站（Storybook 可选） |

### 7.3 组件清单（设计稿 §3.3 → shadcn/ui 定制）

| 组件 | 规格要点 | 实现载体 |
| :--- | :--- | :--- |
| 按钮 | 主/次/幽灵；胶囊 999px；高 40px / 移动 44px；按压 scale 0.97 | shadcn Button 变体 `primary/secondary/ghost` |
| 贴纸徽章 | 白描边+内阴影+微旋转 -2°；免费=薄荷绿 / 2x=珊瑚橙 / 新手=糖果粉 / 官种=靛蓝 / H&R=灰 / 部编版精品=明黄 | 自定义 `StickerBadge` |
| 学科标签 | 六色系 | `SubjectTag` |
| 筛选 Chip | 横向滚动容器；选中态 sky 底色 | shadcn ToggleGroup + 滚动容器封装 |
| 种子行卡片 `.tor-card` | 左分类色块(34px/圆角9px)+主区+右数据列(tabular-nums) | `TorrentRow`（列表核心组件，三端复用） |
| 彩带进度条 | 五色渐变 + 3s 位移动画 | `RainbowProgress` |
| 表格 | 表头 #EAF4FF + 斑马纹 | shadcn Table 主题覆写 |
| 表单 | 输入框 44px、聚焦光晕 | shadcn Input/Form 覆写 |
| 卡片 / 空态 / 弹窗 / 抽屉 | 空态=吉祥物 80px+蜡笔字标题+行动按钮；弹窗 0.2s 缩放 | shadcn Card/Dialog/Sheet + `EmptyState` |
| 底部 Tab（移动） | 5 Tab：首页/搜索/发布/消息/个人中心；图标 24px+标签 11px | `MobileTabBar` |
| 数字滚动 / 签到粒子 | rAF 1s / 0.6s 粒子 | Framer Motion + canvas |

### 7.4 响应式与折叠屏（硬约束）

**断点**（与 Tailwind 对齐）：

| 端 | 区间 | 布局要点 |
| :--- | :--- | :--- |
| 桌面 | ≥1280 | 顶栏 64px 吸顶；12 列栅格（间距 16px）；内容最大宽 1280px；卡片 3–4 列 |
| 平板 | 768–1280 | 卡片 2–3 列；导航收纳 |
| 移动 | ≤768 | 单列卡片；底部 5 Tab；筛选 Chip 横向滚动 |
| 折叠屏展开 | ~673px（双折）/ ~900px（三折） | 单栏居中 max-width 640px / 三栏（导航/内容/辅助） |
| 折叠屏外屏 | ~340px | 超窄单栏：隐藏次级列、标题 2 行截断、触控目标仍 ≥44px |
| 折叠屏分屏 | 720×900（双折）/ 1200×520（三折） | 左 65% 内容 + 右 35% 快捷面板 / 内容-数据-操作三栏分层 |

**硬性规则**（来自设计稿 §4，验收必查）：

1. 铰链中缝预留 **≥48px 不可交互区**（用 `env(viewport-segment-*)` / 厂商折叠 API 检测）；
2. `env(safe-area-inset-bottom)` 适配全面屏与底部 Tab；
3. 触控目标 ≥44px；文字对比度 ≥4.5:1；键盘全可操作；
4. `prefers-reduced-motion` 降级必须生效；
5. 「内容不变、布局重排」：任何端不砍功能字段（种子列表三端三形态字段顺序一致）；
6. 表格在窄屏（折叠态 ~600×400）转卡片布局。

**技术手段**：容器查询（`@container`）优先于媒体查询做组件级自适应（折叠屏分屏的右面板 ≤280px 即容器查询场景）；折叠形态检测用 `viewport-segment` CSS + usehooks-ts 媒体查询兜底。

### 7.5 页面实现优先序与映射（对照设计稿 §0 建议顺序）

| 序 | 页面组 | 模块 | 设计稿画板 |
| :--- | :--- | :--- | :--- |
| ① | Token 与全局样式基线 | — | A1–A3 |
| ② | 登录/注册 | M01 | 登录注册组 |
| ③ | 首页工作台 + 种子列表（核心页优先评审） | M02 | B3/C2/折叠系列 |
| ④ | 种子详情 / 发布 | M03 M04 | 详情/发布组 |
| ⑤ | 课本/官种/保种/排行/勋章/站免池 | M18 M19 M20 M14 M13 | 内容区各组 |
| ⑥ | 经济系统 | M11 M12 | 火花/签到组 |
| ⑦ | 社区 | M15 M16 M17 M23 | 论坛/短讯组 |
| ⑧ | 娱乐 | M24 | 农场/游戏组 |
| ⑨ | 个人中心 | M09 | 我的数据组 |
| ⑩ | 管理后台（可排二期） | M29 | 66 页后台组 |
| ⑪ | 三端 + 折叠屏全形态适配收尾 | 全部 | 扩充册 629 画板 |
| ⑫ | 验收 | — | §7.6 checklist |

### 7.6 UI 验收标准（设计稿 §9 十条，全文保留）

1. 全站无硬编码色值（lint 检查 Tailwind 类与 CSS 变量）；
2. 三端 + 折叠屏逐页对照画板无缺页；
3. 种子列表三端三形态字段顺序一致；
4. 桌面/移动/平板/折叠展开/折叠折叠态五类真机实测；
5. 铰链区无交互元素；
6. 触控 ≥44px、对比度 ≥4.5:1、键盘可操作、reduced-motion 生效；
7. CDN/本地素材可用性检查（品牌素材已入库，构建期校验资源存在）；
8. 数据口径文案检查（火花/官种/保种等术语全站一致，INF 分享率特殊样式，收益因子徽标可点击）；
9. 交互规范符合 §7.3 组件规格（按钮尺寸/动效时长）；
10. Lighthouse 性能 ≥95 + 首屏 ≤1.5s（4G 节流）。

---

## 8. 工程规范：前后端分离与代码标准

> 本章是团队协作的「工程宪法」：前后端边界、编码规范、可维护性红线。CI 强制执行（§8.4），违反即构建失败，不依赖人工自觉。

### 8.1 前后端分离架构约定

**总体形态**：前端（Next.js）与后端（Actix-web API）是两个独立部署的进程，**只通过 HTTP API 通信**，无共享数据库、无服务端模板、无 BFF 之外的进程内调用。

```
浏览器 ──HTTPS──▶ Pingora 网关
                   ├── /            → Next.js (web)     页面 + RSC
                   ├── /api/v1/*    → Actix-web (api)   业务 API
                   ├── /announce/*  → Tracker
                   └── /static|/_next/* → CDN 缓存
```

**职责边界**：

| 维度 | 前端（apps/web） | 后端（apps/api） |
| :--- | :--- | :--- |
| 业务规则 | **零业务规则**。只做展示、交互、状态编排 | 唯一事实源。促销裁决、余额计算、权限判断全部在此 |
| 数据校验 | 表单体验级校验（即时反馈） | **权威校验**。前端校验仅为体验，后端必须全量重验 |
| 鉴权 | 存储/携带凭证（HttpOnly Cookie 由后端 Set-Cookie） | 签发/校验 JWT 与 Session、RBAC 鉴权 |
| 组装数据 | RSC 内调用 API，禁止浏览器端拼业务 | 返回完整 DTO，不做展示格式化（颜色/文案映射在前端） |
| 环境变量 | 仅 `NEXT_PUBLIC_*`（公开配置） | 全部私密配置（DB/Redis/JWT 私钥） |

**硬性规则**：

1. 前端代码禁止出现 SQL、禁止直连 PostgreSQL/Redis；
2. 后端禁止返回 HTML 片段（富文本内容除外，须经 sanitizer 白名单过滤）；
3. API URL 版本化 `/api/v1/*`，破坏性变更必须升版本，旧版本至少保留一个大版本周期；
4. Next.js 的 `app/api/*` 路由**仅允许**做代理转发与会话引导（拿 HttpOnly Cookie 转调后端），禁止写业务逻辑；
5. 前后端唯一共享物是 `packages/domain-types`（TypeScript 类型 + Rust 侧 utoipa schema 生成），契约变更走 OpenAPI diff 流程（§8.2）。

### 8.2 API 契约规范

- **规范优先**：后端用 utoipa 注解生成 OpenAPI 3.1 文档（`/api/v1/openapi.json`），CI 校验「注解与实现 drift = 0」；前端用 `openapi-typescript` 从该文档生成类型，**手写 fetch 类型视为错误**；
- **统一响应信封**：

```json
{ "code": 0, "message": "ok", "data": { }, "request_id": "01J..." }
// 错误： code != 0，message 为用户可读文案，data 可选携带错误详情
```

- **错误码表集中管理**：`packages/domain-types` 定义错误码枚举（分段：1xxx 通用 / 2xxx 认证 / 3xxx 种子 / 4xxx 经济 / 5xxx 社区），前端按 code 映射文案，禁止按 message 字符串判断；
- **分页统一**：列表接口一律游标分页，请求 `cursor?&limit(≤50)`，响应 `{ items, next_cursor, total_estimate }`；
- **时间统一 UTC ISO-8601（RFC 3339）**，前端本地化渲染；金额/流量统一整数最小单位（字节、火花枚），由前端负责人性化格式化；
- **幂等**：所有写操作支持 `Idempotency-Key` 头（购买/领取/转账类必须）。

### 8.3 编码规范

#### 8.3.1 Rust（apps/api、apps/worker、packages/wasm）

| 项 | 规范 |
| :--- | :--- |
| 格式化 | `cargo fmt --check`（CI 强制，零 diff 容忍） |
| Lint | `cargo clippy -- -D warnings`（CI 强制，警告即失败） |
| 命名 | 类型/Trait 大驼峰；函数/变量 snake_case；常量 SCREAMING_SNAKE；模块名单数小写 |
| 错误处理 | **禁止裸 `unwrap()/expect()`** 于非测试代码（clippy lint 限制）；统一 `thiserror` 定义领域错误 → presentation 层映射 HTTP 状态与错误码；`anyhow` 仅允许出现在 apps 层入口 |
| 异步 | Tokio 运行时；阻塞调用（密码哈希、Bencode 大文件解析）必须 `web::block`/`spawn_blocking`；禁止在 async 上下文持锁跨越 `.await` |
| SQL | 一律 sqlx 参数化查询；新增查询必须过 `sqlx::query!` 编译期检查（CI 带 `DATABASE_URL` 跑 `cargo sqlx prepare --check`） |
| 日志 | tracing 结构化日志；禁止 `println!`；span 命名与 handler 一致 |
| 测试 | 领域层单测覆盖率 ≥ 80%（cargo-llvm-cov 度量）；handler 层集成测试走测试容器 PostgreSQL |
| 模块边界 | domain 层 **禁止** import actix/sqlx/redis（架构测试用 `cargo-deny` + 自定义 lint 守护）；repository 只暴露 trait，实现在 infrastructure |

#### 8.3.2 TypeScript / React（apps/web、packages/ui）

| 项 | 规范 |
| :--- | :--- |
| 格式化 + Lint | Prettier + ESLint（`typescript-strict` 预设）+ `eslint-plugin-react-hooks`，CI 强制 |
| TypeScript | **strict 全开**（含 `noUncheckedIndexedAccess`）；禁止 `any`（用 `unknown` + 收窄）；禁止 `!` 非空断言（lint 关闭该语法）；导出函数显式返回类型 |
| 组件 | 函数组件 + Hooks，禁止 class 组件；文件名与组件名一致（PascalCase）；单组件 ≤ 300 行，超出必须拆分 |
| 服务端/客户端 | 页面默认 RSC；`'use client'` 仅用于交互叶子组件，且必须出现在文件首行；数据获取只在 RSC/Server Action 中，客户端组件禁止直接 fetch 业务 API（表单提交走 Server Action） |
| 状态 | 服务端状态用 RSC + revalidate；客户端状态最小化（useState/context），仅表单/UI 状态；禁止引入全局状态库（确需时评审后引入） |
| 样式 | **只允许 Tailwind 工具类 + 设计 Token 变量**（§7.1）；禁止内联 style（动态值除外）、禁止硬编码色值/字号（§7.6-1 lint 检查）；组件变体用 `cva`（class-variance-authority） |
| 目录 | `app/`（路由，薄）、`components/`（按域分组 torrent/user/…）、`lib/`（纯函数）、`hooks/`；`components/ui/` 仅放 shadcn 基件，业务组件禁止放此 |
| 请求 | 统一 `lib/api-client.ts` 封装（信封解包、错误码映射、重试策略）；组件内禁止裸 fetch |
| 可访问性 | 交互元素必须有可聚焦态与 aria 标注（jsx-a11y lint 全开）—— 承接设计稿 §9 验收 |
| 测试 | 关键交互组件 Storybook + Vitest 组件测试；E2E（Playwright）覆盖 P0 核心路径（登录/列表/详情/发布/下载） |

#### 8.3.3 通用工程纪律（全栈）

- **提交规范**：Conventional Commits（`feat:|fix:|refactor:|docs:|chore:` + 模块 scope，如 `feat(api): ...`），CI 校验格式；
- **分支模型**：trunk-based —— `main` 始终可发布，短生命周期分支（≤3 天）+ PR 必审（至少 1 人，CI 绿灯是合并前置）；
- **PR 体量**：单 PR ≤ 400 行变更（不含生成物），超限拆分；
- **提交前钩子**：pre-commit 跑 fmt/lint 增量检查（前端 husky + lint-staged；Rust 用 cargo-husky 等效），CI 再全量兜底；
- **禁止提交物**：密钥（gitleaks 扫描）、生成代码（OpenAPI 生成物除外，单独目录）、二进制大文件（品牌素材走 LFS）。

### 8.4 CI/CD 质量门禁

所有规范落地为流水线硬门禁（`.github/workflows/ci.yml`），任一红灯构建失败：

```
PR 提交 → [lint 层] cargo fmt/clippy · eslint/prettier · gitleaks · conventional commits
        → [类型层] tsc --noEmit · sqlx prepare check · openapi drift check
        → [测试层] cargo test(含架构守护测试) · vitest · playwright(P0 路径)
        → [安全层] cargo-audit · cargo-deny(license) · npm audit
        → [度量层] 覆盖率不低于基线(Rust domain ≥80% / 前端核心组件 ≥60%)，只升不降(ratchet)
合并 → main → 构建 Docker 镜像(多阶段) → 推送 registry → 部署 staging 自动化
```

### 8.5 可维护性规范

1. **文档即代码**：每个 `apps/*` 顶层 README 说明职责/启动/测试；跨模块约定只写在 `_doc/`，代码注释只解释「为什么」，不解释「是什么」；
2. **ADR 决策记录**：所有架构级决策（如 Tracker 终选、状态库引入）写成 ADR（`_doc/adr/NNN-标题.md`：背景/选项/决策/后果），本文档 §2 是 ADR-000；
3. **依赖准入**：新增依赖需在 PR 描述说明用途与替代方案；Rust 侧 cargo-deny 维护许可与来源白名单；npm 依赖周下载量 < 1k 或半年无维护的默认拒绝；
4. **技术债显式管理**：`TODO` 必须带 issue 号（`// TODO(#123): ...`），无 issue 的 TODO CI 报警；每 Phase 结束留 10% 时间清偿本期技术债；
5. **数据库迁移纪律**：schema 变更只增不改（新增列可空/带默认值），破坏性变更走「双写过渡 → 迁移数据 → 删旧列」三步，迁移文件一经合并禁止修改；
6. **观测性内建**：新接口必须同时交付 tracing span + Prometheus 指标 + 错误码登记，缺一不合入；
7. **公共组件准入**：进入 `packages/ui` 的组件必须有 Storybook 用例 + props 类型注释；一次性业务组件留在各 app 内；
8. **注释与命名底线**：自文档化优先；任何「魔法数字」（如促销倍率、缓存 TTL）必须提为 `packages/domain-types` 常量并注释口径来源。

---

## 9. 实施路线图

### 9.1 阶段总览

| 阶段 | 周期 | 核心任务 | 里程碑 / 交付物 | 验收口径 |
| :--- | :--- | :--- | :--- | :--- |
| **Phase 0 立项与脚手架** | 2 周 | Monorepo 搭建（Turborepo + pnpm + Rust workspace）；§8 全套 CI 门禁落地（fmt/clippy/eslint/tsc/sqlx check/openapi drift/gitleaks/提交规范）；Docker Compose 开发环境；Token 基线评审 | `make dev` 一键起全栈；Token 规范站可浏览；故意提交违规代码时 CI 全部红灯（门禁自证） | 新人 clone 后 30 分钟内跑通本地环境 |
| **Phase 1 基础架构与认证** | 4 周 | PG schema 与迁移框架；Actix 骨架（整洁架构分层）；M01 认证/邀请；JWT/Session；审计日志；Pingora 基础路由 | 开发环境全链路（网关→web→api→db）跑通，注册登录可用 | M01 验收基准；安全头全配 |
| **Phase 2 核心 PT 功能** | 8 周 | M02–M10：种子全流程、Torrust 集成与 announce 计费链路（§5.4 技术验证在前 2 周定案）、促销引擎、评论感谢收藏、RSS、基础个人中心与后台 | **可运行 Beta**：注册→浏览→下载→做种→统计闭环 | announce 30万/s 基准；百万种子查询 P99<50ms |
| **Phase 3 全站 UI 与多端** | 8 周（与 Phase 2 后半并行 4 周） | 按 §7.5 顺序落地全部前台页面；折叠屏 7 形态；PWA 基础；性能优化（ISR/虚拟滚动/图片） | **UI 完整可演示版** | §7.6 十条 UI 验收；Lighthouse ≥95 |
| **Phase 4 经济与社区** | 6 周 | M11–M17 M18 M19 M20 M23：火花经济、签到、站免池、勋章、论坛、短讯、求种候选字幕、课本中心（M18）、保种区、排行榜、邀请 | 经济与社区功能全量上线测试服 | 余额=流水重放；保种/站免池自动规则验证 |
| **Phase 5 考核任务与玩法** | 4 周 | M21 M22 M24 M25：绩效考核、任务中心、娱乐玩法矩阵（插件化改造）、装扮中心、管理后台补全（M29 完整版）；收尾 M26 PWA 推送、M27 开放 API、M28 插件系统 | 全功能对齐旧站（除明示砍除项） | 考核零手工填报；玩法接统一交易管线；M26–M28 验收基准 |
| **Phase 6 安全加固与上线** | 4 周 | 压测调优（wrk/udp-bench）；渗透自测；旧数据迁移演练 ×2；灰度切换双轨运行 1–2 周；监控告警（Prometheus/Grafana）齐备 | **生产上线** | §1.3 性能指标全达标；迁移校验报告零差异 |

总周期约 **36 周**（Phase 2/3 并行后有效压缩至 ~32 周），与上游文档 v2.0「Phase 1–5」对应关系：本方案 Phase 0–1 ≈ 上游 Phase 1；Phase 2–3 ≈ 上游 Phase 2–3；Phase 4–6 ≈ 上游 Phase 4–5 并细化到模块级。

### 9.2 团队配置建议

| 角色 | 人数 | 覆盖 |
| :--- | :--- | :--- |
| Rust 后端工程师 | 2–3 | api / tracker 对接 / worker / 迁移工具 |
| Next.js 前端工程师 | 2 | 全站 UI、多端与折叠屏 |
| UI 设计师（兼职） | 1 | 画板对照评审、素材切图、验收 |
| 运维工程师 | 1 | Docker/Pingora/监控/灰度（可兼职） |
| 安全顾问（阶段性） | 1 | Phase 6 渗透与审计 |
| 产品/运营接口 | 1（兼任） | 规则定案（签到补签口径、魔力折算比例、等级阈值等开发前待定项） |

### 9.3 开发前待定项清单（阻塞项，Phase 0 内定案）

| # | 待定项 | 决策人 | 默认建议 |
| :--- | :--- | :--- | :--- |
| 1 | Tracker 终选：Torrust 扩展 vs Aquatic vs 自研 | 后端负责人 | 先 Torrust 验证，2 周内出结论 |
| 2 | 签到补签是否影响连签额外奖励计数 | 运营 | 不影响（保留原始连续计数） |
| 3 | 旧魔力值→火花折算比例 | 运营 | 1:1 起评 |
| 4 | 成长豆荚 LV1–LV6 具体阈值 | 运营 | 按旧站 FAQ 升降级规则换算；新旧档位默认对照见 §3.3 表 |
| 5 | 水浒卡牌等旧活动是否复刻 | 运营 | 默认不进 P2 首批 |
| 6 | IMDb/pt_gen 抓取是否保留 | 产品 | 教育资源弱相关，默认降级为可选海报字段 |

---

---

## 10. 风险与应对

继承上游文档 §13 风险表（Rust 学习曲线、WASM 调试、缓存击穿、连接池、依赖漏洞、容器故障、带宽成本、人员流失等，应对策略不变），此处补充本项目特有的新增风险：

| 风险 | 概率 | 影响 | 应对策略 |
| :--- | :--- | :--- | :--- |
| **Tracker 扩展点不足**：Torrust 无法满足 PT 用户级计费/H&R 需求 | 中 | 高 | Phase 2 前两周技术验证（§9.3 #1）；备选 Aquatic/自研，announce 核心逻辑量级小（~3k 行），自研兜底可行 |
| **计费数据不一致**：异步链路丢事件导致上传量争议 | 低 | 高 | Redis Stream 持久化 + 消费 ack + 对账任务（快照 vs 流水重放，每小时校验，差异告警） |
| **旧数据迁移失败/丢失** | 中 | 高 | 迁移 CLI 全量演练 ≥2 次；行数/求和/抽样三级校验；灰度双轨 1–2 周；回滚预案（旧站只读封存 3 个月） |
| **旧密码体系无法平滑过渡** | 高 | 中 | 强制改密流程（§2.4）+ 站内公告 + 客服通道；监控首周改密完成率 |
| **设计稿 2383 画板验收工作量失控** | 中 | 中 | 按 §7.5 模块分组验收（非逐画板）；每模块「核心页对照 + 抽查」；折叠屏扩充册以「形态规则 + 抽样」验收 |
| **折叠屏真机碎片化**（各厂商铰链/分屏 API 差异） | 高 | 中 | CSS 标准优先（viewport-segments），厂商 API 兜底；Phase 3 建立最低真机矩阵（双折/三折各 ≥2 台） |
| **旧站特色玩法口径失真**（补签/农场刷新等细节与用户预期不符） | 中 | 中 | §9.3 待定项清单在 Phase 0 集中定案；P1 玩法灰度期收集反馈快速调参 |
| **单货币化引发老用户抵触**（魔力→火花折算） | 中 | 中 | 折算比例提前公示；期初流水可追溯；上线首月争议补偿通道 |
| **性能目标过度承诺**（14 万 req/s 依赖压测条件） | 中 | 中 | Phase 6 压测报告公开口径（数据集/并发模型）；关键在业务量级下的真实 P99，而非裸峰值 |

---

## 11. 附录

### 附录 A：旧站模块 → FluxTorrent 映射总表

> 覆盖旧站全部 40+ 入口页面，无遗漏。「优先级」对应 §4 分期。

| 旧站页面/入口 | 旧站功能 | 新模块 | 新路由（约定） | 优先级 |
| :--- | :--- | :--- | :--- | :--- |
| index.php | 首页工作台（统计/消息/投票） | M02 | `/` | P0 |
| login.php | 登录 | M01 | `/login` | P0 |
| usercp.php | 控制面板（个人/网站/论坛/安全） | M09 | `/settings` | P0 |
| userdetails.php | 用户详情 | M09 | `/user/[id]` | P0 |
| torrents.php | 种子列表（筛选/搜索/用户视图） | M02 | `/torrents` | P0 |
| details.php | 种子详情（感谢/评论/收藏/举报） | M03 M07 | `/torrent/[id]` | P0 |
| upload.php（3 模式） | 发布/候选/求种 | M04 M17 | `/upload` | P0 |
| download / announce | 下载与 Tracker | M05 | `/download/[id]` + tracker 端点 | P0 |
| getrss.php | 自定义 RSS | M08 | `/rss` | P0 |
| my_data_stats.php / getusertorrentlist.php | 我的数据 | M09 | `/my` | P0 |
| staffpanel.php + 待审/禁种队列 | 管理面板 | M10 M29 | `/admin` | P0 |
| reports.php / cheaterbox.php / log.php | 举报/作弊箱/日志 | M10 | `/admin/reports` 等 | P0 |
| faq.php / rules.php / aboutnexus.php | 帮助/规则/关于 | M10（内容页） | `/help` `/rules` `/about` | P0 |
| topten.php | 排行榜 | M20 | `/top` | P1 |
| mybonus.php | 火花商店 | M11 | `/shop` | P1 |
| mybonusmine.php | 我的火花（收益明细） | M11 | `/my/spark` | P1 |
| plugin-bank.php | 火花银行 | M11 | `/bank` | P1 |
| attendance.php | 签到 | M12 | `/attendance` | P1 |
| magic_pool.php | 站免池 | M13 | `/magic-pool` | P1 |
| medal.php / medal_wall.php | 勋章图鉴/勋章墙 | M14 | `/medals` | P1 |
| forums.php | 论坛（9 版块） | M15 | `/forums` | P1 |
| messages.php | 站内短讯 | M16 | `/messages` | P1 |
| contactstaff.php | 联系管理组 | M16 | `/messages/staff` | P1 |
| friends.php | 好友/黑名单 | M16 | `/friends` | P1 |
| invite.php | 邀请 | M23 | `/invite` | P1 |
| viewrequests.php | 求种区 | M17 | `/requests` | P1 |
| offers.php / candidate.php | 候选区 | M17 | `/offers` | P1 |
| subtitles.php | 字幕区 | M17 | `/subtitles` | P1 |
| textbooks.php | 课本中心 | M18 | `/textbooks` | P1 |
| seed_preserve.php | 保种区 | M19 | `/preserve` | P1 |
| jixiao.php / jixiao_admin.php | 绩效考核（前台/后台） | M21 | `/jixiao` | P1 |
| task.php | 任务中心 | M22 | `/tasks` | P1 |
| magic_fram.php | 好学农场 | M24 | `/farm` | P2 |
| magic_scratch.php | 刮刮乐 | M24 | `/games/scratch` | P2 |
| jgg.php / jgg_admin.php | 九宫格抽奖 | M24 | `/games/jgg` | P2 |
| bigsmall.php | 猜大小 | M24 | `/games/bigsmall` | P2 |
| wuziqi.php | 五子棋 | M24 | `/games/wuziqi` | P2 |
| medal_synthesis.php | 卡牌合成 | M24 | `/games/synthesis` | P2 |
| 水浒传卡牌活动 / funvote | 限时活动 | M24 | `/events` | P2（待运营定） |
| ptlife.php | PT Patronus 守护神 | M24 | 外部联动 | P2（评估） |
| zhuangshi.php / zhuangshi_admin.php | 装扮中心 | M25 | `/decoration` | P2 |
| promotionlink.php | 宣传链接 | M11 | `/promote` | P1 |
| tools.php / 工具集（图床 Token/压缩/auto-feed/IYUU） | 外部工具聚合页 | M27 | `/tools` + 开放 API 支撑 | P2 |
| magic.php | 趣味盒（magic 主入口） | M24 | `/games/magic-box` | P2 |
| staff.php | 管理组名单 | M10 | `/staff` | P0 |
| poll / shoutbox | 投票/群聊 | M15 | 论坛投票 + `/shoutbox` | P1 |

**新增能力模块**（无旧站页面对应，为 FluxTorrent 新建的基础能力，供对照）：

| 模块 | 功能 | 优先级 |
| :--- | :--- | :--- |
| M06 促销引擎 | 促销状态单一事实源（§5.5），无独立页面，渗透于列表/详情/计费 | P0 |
| M07 评论/感谢/收藏 | 内嵌于种子详情页（旧站同页承载，故无独立入口行） | P0 |
| M26 PWA 与推送 | 全站能力（manifest/Service Worker/推送订阅），无独立路由 | P2 |
| M28 插件系统 | 后端 Plugin Trait 与装配机制（§4.3），无用户可见页面 | P2 |
| M29 完整管理后台 | 即上表 staffpanel 行的完整版（66 页，§4.3） | P2 |

### 附录 B：名词表

| 名词 | 释义 |
| :--- | :--- |
| **PT / 私种子站** | Private Tracker：需注册邀请制、统计用户上传下载量的种子社区 |
| **火花（Spark）** | 站点唯一虚拟货币：做种/发布/签到等获取，商店/银行/游戏消费（§2.3 决策） |
| **官种** | 官方发布种子，旧站 tag_id=3，新站 `official_tag` 标识 |
| **保种** | 承诺长期做种指定种子的机制；保种区种子免费下载，做种>7 自动移出，免费延续 3 天 |
| **H&R（Hit & Run）** | 下载后未完成保种义务的行为，触发追责标记 |
| **促销（Promotion）** | 种子/全站的上传下载计费倍率状态：免费/2X/2X免费/50%/2X50%/30% |
| **passkey** | 用户级 Tracker 鉴权令牌，拼接在 announce URL 中 |
| **站免池（Magic Pool）** | 全站捐赠池：月累计 200 万火花触发次月全局双免（M13） |
| **绩效考核（Jixiao）** | 按保种/发布等指标周期性发放火花工资的系统（M21） |
| **成长豆荚** | 用户等级 LV0–6 的品牌化体系：种子→新芽→幼苗→小树→大树→开花→硕果（§3.3） |
| **断种** | 做种数为 0 的种子 |
| **_INF 分享率** | 上传量远大于下载量时分享率显示为无穷大，UI 需特殊样式 |

### 附录 C：参考资料索引

| 资料 | 位置 | 用途 |
| :--- | :--- | :--- |
| 下一代 PT 站点重构方案 v2.0（最终详细版） | `_doc/` | 技术架构、性能/安全基准、Docker/监控配置 |
| 可参考的开源项目清单 | `_doc/可参考的项目.md` | Torrust/Aquatic/Actix/Next.js 等选型依据 |
| 设计交接文档 v44.0 | `UI参考/dev-handoff.html` | 设计 Token、组件、动效、模块清单、验收标准 |
| 主题归纳与版本史 | `UI参考/theme-summary.html` | 品牌概念、数据口径、v1→v44 演变 |
| 设计画板主册（1754 块） | `UI参考/index.html` | 桌面/移动/平板/折叠屏页面细节 |
| 折叠屏扩充册（629 块） | `UI参考/index-fold-extra.html` | 外屏/半展开/折叠态/分屏形态 |
| 设计数据源（真实站点样本） | `UI参考/gen/data.py` | mock 数据与口径基准（种子/课本/商店/勋章等） |
| 旧站真实页面（42 个） | `UI参考/real_pages/` | 功能规则与业务口径的事实来源 |

---

*本文档由 NexusPHP 旧站调研（42 页面）、UI 设计稿 v44.0（2383 画板）与技术重构方案 v2.0 三份材料整合而成。业务规则与数据口径以旧站事实为准，架构决策以 v2.0 为准，UI 规范以设计稿为准 —— 三者冲突处已在 §2 决策记录中显式裁决。*

