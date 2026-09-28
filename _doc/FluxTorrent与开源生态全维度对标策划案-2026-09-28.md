# FluxTorrent × 开源生态 全维度闭环对标与完善策划案

日期：2026-09-28 ｜ 性质：分析+策划（未实施）｜ 输入：三路并行调研（本仓现状盘点 / NexusPHP·TorrentPier·生态扫描 / UNIT3D·Gazelle·站长口碑）+ 既有闭环审查（0209/0211/0214/0216 四批已修）+ 本次代码抽验

定位基准：**只做建站系统、不偏向任何 PT 类型、站长自由搭建、轻松自定义功能和字段**。所有对标结论都以此为准绳——竞品有而与本定位无关的，不追；竞品没有而本定位需要的，是主攻方向。

---

## 一、结论速览

1. **产品功能闭环已处于第一梯队**：与 NexusPHP v1.10.2 / UNIT3D v9.2.0 逐功能面对比，功能覆盖 ≥95%（促销 6 类×4 域×定时 90 天、考核、H&R、经济、抽卡、成就、2FA/passkey、工单式后台 65+ 工具面均内建），且多处分量更重（安全四件套+Argon2、审计、多机部署、读写分离）。
2. **四处结构性领先，应作为收录与宣传主轴**：① 安装向导（4 步+装机门+demo 自动清除，竞品均无站型概念）；② 原生 Rust tracker（UDP+BEP-7 IPv6+peer 外置+实测 2138 req/s，竞品要靠外挂 Go/Rust 组件才有同等能力）；③ 升级不碰魔改（设置全在库+站型包可回滚，对比 UNIT3D `git:update` 逐文件人工合并）；④ 自定义字段体系（用户侧+内容侧六类型，竞品均无）。
3. **四个真实缺口**：CSP 安全头（唯一硬缺口）；等级后台只读+晋级进度无可视化；运营层"怎么把站办好"的知识文档缺位（对位 NP 的 18 篇教程）；生态工具兼容性（cross-seed/PT-Depiler/pt_mate）未实证。
4. **赛道窗口确认**：TorrentPier 2026-05 官方日落、Gazelle 冻结、sqtracker/SOPT 停滞——现代栈活跃开源整站只剩 UNIT3D（AGPL、Discord 付费墙、官方安装脚本已下线）与 NexusPHP（GPL-2.0、单人维护）；同时冒出同定位的 Rust 新竞品 Arcadia（359 stars，单人、0.0.1 版）。"MIT+开箱即用+中立站型"的通用建站系统位子是空的，但窗口不会一直开着。
5. 行动方案为 **E 批 16 项**（P0×5 / P1×6 / P2×5，见第五章），另附"明确不做"反定位清单。

---

## 二、2025-2026 开源竞品格局（调研事实，截至 2026-09-28）

### 2.1 活跃整站源码

| 项目 | 技术栈 | stars | 状态 | 协议 | 社区形态 |
|---|---|---|---|---|---|
| **UNIT3D**（HDInnovations/UNIT3D） | Laravel 12 + Livewire + Alpine | 2,431 | 活跃（development 分支当日仍有合并，133 贡献者，v9.2.0/2025-12-03） | AGPL-3.0 | Discord **付费入场**；付费服务价目表 |
| **NexusPHP**（xiaomlove/nexusphp） | PHP 8.2+ Laravel + Filament | 1,186 | 活跃（v1.10.2/2026-04-11，2026-05 后提交放缓） | GPL-2.0 | 官网+文档站+TG 群；单人主导（517/贡献） |
| **Gazelle**（OPSnet/Gazelle） | PHP 8.4 + MySQL/PG 双库 + sphinx | 409 | Orpheus 自用维护（2026-06 后无推送） | Unlicense | 无对外社区 |
| **Arcadia** | Rust + TS/Vue | 359 | 2025-03 新起，仅 0.0.1，单人开发公开招人 | AGPL-3.0 | Discord/Matrix |
| TorrentPier | PHP（phpBB2 血统） | 344 | **2026-05 官方日落归档** | MIT | 继任项目 Dexter 2027 才亮相 |
| rocket-pt / sqtracker / SOPT | Java / Node / Rust | 135/284/28 | 均已停滞 | — | — |

tracker 专项（Rust/Go 重写潮流）：NexusPHP 官方 Go Tracker（2025-06）、Roardom/UNIT3D-Announce（Rust，62★，标称支撑 1M+ peers，官方收费 $75 代装）、torrust-tracker（531★）、aquatic（622★）。

生态重心事实：工具已比源码更热——PT-Depiler 浏览器插件 2,634★、cross-seed 1,554★、auto_feed_js 1,250★、pt_mate 移动端 502★。**存量站的效率工具生态，是新站源码的"间接流量入口"。**

### 2.2 竞品模式要点（与本文相关的事实）

- **UNIT3D 商业化**：安装 $100、升级 $50、Rust Announce 代装 $75、调优 $175、主题包 $199（10 套）、上传器 $499、Discord 支持 $200/年；并承接 Gazelle/U-232/XBTIT 迁移。主题是付费 zip+bash 脚本，无插件体系；官方一键安装脚本**已下线**；生产 compose 缺位（仓库内 compose 是 Sail 开发环境）；文档仅 mdBook 13 章且无生产安装章/功能手册。升级靠 `php artisan git:update` 逐文件人工裁决本地魔改。AGPL-3.0。
- **NexusPHP**：安装文档四路（手动 9 步/宝塔/1Panel/Docker+Hub 镜像），升级指南 1.6→1.10 全链，中英双语文档站，i18n 20 语/Crowdin 29 语；插件 composer 安装（大转盘/自定义菜单/角色权限等）；Go Tracker 与 ClickHouse 存做种记录为可选组件；PHP 版 CVE 共 31 条全部集中在 1.7.33 之前的旧代码（最新一组 2022 年，CVSS 9.8 SQL 注入）；Laravel 化后无公开 CVE。
- **Gazelle**：官方安装文档自述"不保证完整性，若感吃力请勿在生产运行 tracker"；需同时养 MySQL+PostgreSQL+sphinx+memcached 四件套；Ocelot 仅 IPv4。音乐站特化，新站基本不选。
- **Arcadia**：与本产品定位最接近的并行探索者（"内容无关+易部署"），但功能面（等级/经济/审核/抽卡等）远小于本产品，单人+0.0.1 版。**需持续观察其社区势能。**
- 站长口碑（可核实证据）：付费安装/升级/迁移是真实需求（HDInnovations 价目表反向证明）；性能要自己折腾（Meilisearch 5 万+结果问题 2026-09-26 才修）；老平台退场潮的根因是"旧架构改不动+维护者青黄不接"。

### 2.3 五条生态趋势（影响产品决策）

1. 老牌裸 PHP 单体进入退场期，活下来的都在框架化/编译型重写——本产品的 Rust+Next.js 路线站在趋势正确侧。
2. announce 层与 Web 层分离成为共识（Go/Rust tracker 外挂化）——本产品**原生一体**是差异化卖点，且已具备外置 peer 存储的多机形态。
3. 部署体验成为竞争点：中文圈在宝塔/1Panel，国际 PaaS（Heroku/Render/Railway）一键模板全生态空白——**空白即机会**。
4. 安全债集中在旧版本魔改站；新架构项目无公开 CVE——"新架构+安全基建"本身可作为收录宣传点，但前提是把自己的头补齐（CSP）。
5. 工具生态比源码更热——兼容 cross-seed/PT-Depiler/pt_mate 等工具的优先级，应高于自建移动端。

---

## 三、十二维度对标详析

> 判定口径：**领先 / 持平 / 落后**，均相对"当前最活跃的两个对手"（UNIT3D + NexusPHP）。每维给出建议项编号（E1-E16，见第五章）。

### D1 仓库与工程门禁 —— 领先

| 面 | FluxTorrent | UNIT3D | NexusPHP |
|---|---|---|---|
| 迁移/版本 | 234 迁移（0237），v0.2.0+arm64 发布流水线 | v9.2.0 | v1.10.2 |
| CI | 5 workflows（含 e2e 装站链/周度 cargo-audit） | 无公开 CI 结论 | 无公开 CI 结论 |
| 质量门禁 | 本地 ci_local.sh + 8 守门脚本 + 300 行/80 列门禁 | — | — |
| 文档 | docs/ 四册 22 篇 + README 重写 | mdBook 13 章（无生产安装/功能手册） | 文档站最全（安装四路+全链升级） |

判定：工程质量与文档结构**领先 UNIT3D、结构与 NexusPHP 持平**。缺：无覆盖率门禁、无浏览器层 E2E（→E14）；仓库仍有开发残留目录尾巴（`.research*`/`_attic` 等，继续按 C3 口径清）。

### D2 安装与首启 —— 领先（保持并加固）

- FluxTorrent：4 步向导（站型→站名+管理员→合规→运营模板）+ setup_gate 装机门封锁一切业务 API + demo 数据自动清除 + 必填环境变量仅 3 项 + launch-checklist 三项开站检查。
- 对比：NexusPHP 手动安装 9 大步+两守护进程+删 install 目录（另有宝塔/1Panel/Docker 三路文档化）；UNIT3D 官方安装脚本已下线、生产 compose 缺位、文档无生产安装章。**竞品向导均无"站型"与"运营模板"概念。**
- 建议：向导已闭环，不重做。补两小件——①低配/离线环境说明（<2GB 内存、无外网拉镜像场景）入 quick-start；②setup 完成页直接链到 E2 手册的"开站第一周"清单（→E2）。

### D3 部署形态 —— 持平（对 UNIT3D 领先，对 NP 中文面板生态落后）

- 已有：单机 compose（含 monitoring profile）+ multi-node compose + nginx LB 样例 + 三机配方实证 + ghcr 双架构镜像 + backup/restore（14 天轮转+演练模式）。
- 对比：NexusPHP 有宝塔/1Panel/Docker Hub 四路；UNIT3D 生产部署基本靠手搓或付费。
- 缺：宝塔/1Panel 适配指南（中文站长实际主流面板）→E11；K8s/Helm **不做**（见反定位清单）。

### D4 使用与易用性（终用户视角） —— 持平

- 已有：移动端 M1-M5 全站四档断点、三语 i18n+术语后台可改、RSS、PWA 无、hover 卡、行卡片、暗色 Aurora 主题。
- 对比：NexusPHP 20 语（Crowdin 29 目标语）、响应式弱（第三方 Flutter 客户端补位）；UNIT3D 响应式重写完成、Weblate 多语、无 PWA、无官方 APP。
- 判定：响应式质量本产品优于 NP、与 UNIT3D 持平；语言广度落后（3 vs 20+）但可辩护（质量优先+后台术语可改）。建议：PWA 轻增量抢先（竞品均无）→E8；语言扩展与贡献流程→E10。

### D5 用户成长全流程（注册→最高等级） —— 持平，体验层两处落后

全流程走查（本产品现状）：

1. **进入**：注册三模式（开放/邀请[邮箱绑定]/申请制人审）+ email_verify 真发信拦截 + 一次性邮箱黑名单 + 验证码四驱动 ✓（超两竞品：NP 1.10 才加 Turnstile，UNIT3D 用 config captcha）
2. **新手期**：onboarding 双模板（考核淘汰制/缓冲宽进制 0226）+ 考核引擎 exams/jixiao + 新手考核内容后台可配 ✓（对位 NP 考核；UNIT3D 仅 hitrun 开关）
3. **日常**：签到/补签/魔力流水/银行五档/商店/券/复活券/抽卡/娱乐屋/农场 ✓（超 UNIT3D BON；与 NP 道具体系持平+）
4. **义务与风险**：H&R 快照/惩罚/预警参数化/赦免 + 作弊四件套 + agent 黑白名单 ✓
5. **晋级**：七级成长（种子→硕果）四维门槛 + class_auto_adjust 自动 + /classes 公开页 ✓（机制与两竞品持平）
6. **职务**：90-99 六级 + 71 权限键 + staff_panel 分档 ✓（超 UNIT3D 用户组粒度）
7. **荣誉**：勋章多佩戴/头像框/装扮/彩虹 ID/成就 ✓（对位 UNIT3D achievements 且更重）
8. **衰退与回归**：不活跃三档 + 自助解封 + 申诉联动 ✓（NP 1.10.2 才加自助解封）
9. **满级之后**：空档——无长线目标设计 ✗

两处真实落后（本次代码抽验）：
- **晋级进度不可视**：/classes 页只有静态等级表（grep 证实全站仅 achievements/exams 两处有 progress 元素）；NP 系用户中心普遍显示"距下一级还差多少"。→E3
- **等级体系后台只读**：class_rules/user_classes 改门槛/改名靠迁移脚本（0172 调档先例）。"站长自由搭建"定位下这是最刺眼的一块——站名能改、模块能关，唯独等级门槛不能。→E3（P0）

### D6 自定义能力（定位核心） —— 领先，且是最大卖点

| 层 | FluxTorrent | UNIT3D | NexusPHP |
|---|---|---|---|
| 站型预设 | 11 站型+custom 克隆，可回滚 | 无（单形态） | 无 |
| 模块开关 | 30 键四端联动+守卫脚本 | 43 个 config 文件散置 | 后台设置树 |
| 自定义字段 | 用户侧+内容侧各六类型 | **无** | **无**（自定义标签仅插件） |
| 自定义页面/菜单/术语 | 全有 | 无 | 插件 |
| 主题 | token 换肤（1 套+变体） | 付费 $199/10 套 | 模板目录（未文档化） |
| 内容包生态 | 生态商店四品类+wasmtime 适配器 | 无 | 插件市场雏形 |

判定：**自定义字段与站型体系是全生态独有**。唯一对手是 WordPress 式"视图层自配"（列表显示哪些列、详情页哪些段落）尚缺→E6（注意：此项先做全仓大盘点再实施，避免与现有 homelayout/tagdict/sections 能力重叠）。

### D7 管理与运营 —— 领先（工具面），落后（知识面）

- 工具面：65+ 后台工具（举报/申诉/批量发放/经济仪表/作弊四件套/IP 检查/群发/审计…）超 UNIT3D staff 面与 NP Filament 面板。
- 缺口①：**运营手册**——NP 用 18 篇教程+FAQ 把"怎么配促销/怎么设考核/怎么做防作弊巡检"讲透了，这是它中文圈统治力的一半；本产品 docs 有"怎么装/怎么配"，缺"**怎么运营**"。→E2（P0）
- 缺口②：admin overview 无数据大盘（趋势/留存/做种健康度四态——后者本就是产品口径遗留）→E16。

### D8 性能与规模 —— 领先

- 本产品：原生 Rust tracker（HTTP+UDP+BEP-7 IPv6 分列 peers6）+ peer 外置 Redis 多副本互见 + announce 2138 req/s 零失败实测 + 读写分离钩子 + 消费者组计费 + SKIP LOCKED 分片 + PG 分区表 + 缓存版本号 bump。
- 对比：NP 需外挂 Go Tracker（2025-06 起）+ClickHouse 才能到大站量级；UNIT3D 默认 PHP announce，追 1M+ peers 需另购/另装 Rust Announce；Gazelle Ocelot 仅 IPv4。
- 判定：**架构领先一代**。待补：带量基准（10 万 peer 灌数）依赖公网→并入 E11；搜索在大数据量下的方案（现 PG 全文）→E9 评估报告，不预立项 meilisearch。

### D9 安全性 —— 持平偏领先，一处硬缺口

- 已有：Argon2、TOTP+WebAuthn passkey、验证码四驱动、IP 封禁、一次性邮箱域、登录失败锁定、双层限流、审计日志、安全头四件（nosniff/DENY/Referrer/HSTS）、JWT≥32 字节强制、SECURITY.md、周度 cargo-audit。
- 对比：NP 历史 CVE 31 条（全旧版）；UNIT3D 0 CVE+2FA 确认。
- **硬缺口：无 CSP（Content-Security-Policy）**——api/web 两侧均未设。对"自托管图片/附件/视频内嵌"的产品这是 XSS 纵深缺失，且是三大件（NP/UNIT3D/Gazelle）中至少 UNIT3D（laravel-security-headers）已覆盖的项。→E1（P0 第一件）
- 次缺：无覆盖率门禁（安全回归靠 e2e）→E14；GeoIP/IP 信誉联动（geo.rs 已有数据面）→P2 观察。

### D10 平台适配 —— 持平

- 服务端：Linux 容器 amd64+arm64 ✓；Web：桌面 4 档断点+三折叠 ✓；客户端协议：HTTP/UDP tracker、BEP-7 v6、RSS、NP 兼容层（compat_http/nexusphp.rs、ptpp.rs）、openapi 60 req/min ✓。
- 对比：NP 覆盖宝塔/1Panel；UNIT3D 仅 Ubuntu 手册；PaaS 一键模板全生态空白。
- 缺：第三方工具（cross-seed 辅种/PT-Depiler/pt_mate）兼容性**从未系统验证**——这是生态入口（趋势 5）→E4（P0）；宝塔/1Panel 指南→E11。

### D11 社区与生态位 —— 起步期（先发产品无社区是常态，但要按"收录即冲刺"准备）

- 商业模式对照：HDInnovations 证明安装/升级/主题/调优有真实付费意愿，但 AGPL+Discord 付费墙也压制了社区；NP 单人维护 bus factor 高。本产品 MIT+全免费+文档开放，是社区策略上的对位优势——**建议坚持开源全量，商业化留"托管/服务"后手，不设知识付费墙**。
- 收录准备件已在（docs/ops/工具生态收录.md）；缺口是 demo 站与带量基准两个"可看可摸"的公网件→E11 承接旧 #14/#15。

### D12 运维（备份/升级/监控/日志） —— 领先

- 已有：backup.sh/restore.sh（演练模式）、Prometheus+Grafana 预置盘+5 告警、runtime_log 落库后台可查、43 job 应用内调度（advisory lock+四档节奏）、ops_webhook（Discord/TG 广播）、upgrade.md（零魔改升级+回滚+NP 迁移）。
- 对比：UNIT3D 升级靠逐文件人工合并+spatie/laravel-backup；NP 自动备份+cron 体系。
- 待补：升级路径只有文档无**滚动升级演练实证**（含 DB 迁移回滚）→E5；自动备份 cron 文档化（脚本头已有示例，提入 launch-checklist 即可，随 E2 带出）。

---

## 四、用户全流程闭环走查结论（定位专项）

沿"安装向导→冷启动→日常运营→用户成长→满级→衰退回归"六段走查，对照 0209/0211/0214/0216 四批修复后的现状：

| 环节 | 闭环判定 | 遗留 |
|---|---|---|
| 安装向导→首站 | ✅（4 步+装机门+demo 清除+双模板） | 低配/离线说明小补 |
| 冷启动→有内容 | ✅（站型包/内容包/生态商店/RSS 转载/辅种兼容层） | **辅种工具兼容未实证（E4）** |
| 日常运营 | ✅ 工具层（65+ 面）；❌ 知识层（运营手册 E2） | 数据大盘（E16） |
| 用户成长 | ✅ 机制层；❌ 体验层（进度可视化 E3） | 满级长线（E15） |
| 站长自定义 | ✅ 字段/模块/页面/菜单；❌ 等级门槛后台化（E3）、视图层（E6） | — |
| 升级与长期 | ✅ 零魔改升级设计；❌ 滚动升级演练实证（E5）、版本支持政策 | — |

**总评：闭环不存在"断链级"缺陷（P0 假配置类问题在 0209 批已清零），剩下的是体验层、知识层、实证层三类"最后一块"。**

---

## 五、行动方案：E 批（16 项）

### E-P0（第一批，建议 10 个工作日内，5 项）

**E1 安全头收尾：CSP**
- 范围：api 侧与 web 侧（next.config.ts headers）补 Content-Security-Policy + Permissions-Policy；评估 COOP/COEP。注意产品有站内图片/附件/视频内嵌（ammonia 白名单+Range 206 已做），CSP 策略需与 embed 白名单对表，勿一刀切禁 frame/media。
- 产出：响应头落地 + docs/security.md（安全姿态清单：哈希/限流/审计/头/披露渠道一页纸）。
- 验收：curl 双侧可见头；e2e 附件/视频内嵌/头像回归全绿。

**E2 站长运营手册（docs/webmaster/playbook）**
- 对位 NP「18 篇教程+FAQ」的生态位，补"怎么运营"知识层。六线各一篇：促销玩法编排（6 kind×4 scope×定时的组合打法与节奏案例）、考核与 H&R 数值标定（对位既有"魔力经济收支平衡表"）、邀请策略选型（开放/邀请/申请三模式+邮箱绑定+一次性邮箱的取舍）、防作弊日常巡检（四件套+agent 规则+IP 检查的周巡检清单）、内容冷启动（站型包+内容包+RSS 转载+辅种）、开站第一周 checklist（含自动备份 cron 配置）。
- 验收：每篇含可抄作业的参数示例；launch-checklist 与 setup 完成页互链。

**E3 等级成长体验包**
- ① /classes 页增加"我的进度"：当前级/下一级四维差距（还差多少上传量/魔力/时间/分享率）——class_rules 单源，登录态下从 me 数据计算；② 晋升/降级触发站内通知（class_auto_adjust 已有 job，补 notice 出口）；③ **后台等级 CRUD**：user_classes 门槛/名称/privileges JSONB 可视化编辑（71 权限键复用 perm 矩阵 UI），废除"改门槛靠迁移"。
- 验收：改门槛→worker 下轮调档生效→用户侧进度即时反映；i18n 三语同步。

**E4 生态工具兼容认证**
- 用 cross-seed / PT-Depiler / pt_mate 三件（生态热度 Top）对本站做真机冒烟：RSS 拉取、种子下载（passkey 链）、NP 兼容 API 关键端点（compat_http/nexusphp.rs、ptpp.rs 已有）、openapi token 流。
- 产出：docs/ops/兼容矩阵.md（工具×功能×状态表）+ 冒烟脚本入 CI 周跑 + openapi 文档加"稳定端点承诺"节（哪些路径带版本承诺、哪些会变）。
- 验收：矩阵全绿或标注已知限制；这是收录宣传的直接素材。

**E5 升级保障实证与版本政策**
- ① 演练：起 0.2.0 全栈（灌测试数据）→ 拉新版镜像滚动升级 → DB 迁移前进 → 验证 → 回滚演练（镜像回退+迁移 down/快照恢复二选一，明确推荐路径）；写成 docs/ops/upgrade-drill.md 并脚本化（scripts/upgrade_drill.py）。
- ② 版本支持政策：SECURITY.md/README 增补"当前支持版本窗口+升级跳板规则"（0.x 阶段：最新 minor+N-1）。
- 验收：演练脚本一键可重复；政策入文档。

### E-P1（第二批，6 项）

**E6 视图层自配**：列表列显隐/排序、详情页字段段落可见性、发布表单段落编排，做成长后台配置（默认沿用各站型预设）。**前置：先跑一次全仓大盘点**（homelayout/tagdict/sections/0195 字段体系已有大量能力，防重叠），以盘点结论裁剪范围。
**E7 捐赠自动回馈档位**：donation tiers（捐额→魔力/上传量/邀请数），支付回调即时发放+捐赠者徽标+webhook 通知；对标 UNIT3D donation 体系，补齐"捐赠→回馈"闭环最后一环。
**E8 PWA**：manifest+SW 缓存壳（静态资源+离线提示页）+移动端安装引导条。竞品均无，M5 之后轻增量高感知。
**E9 搜索体验包**：① 组合条件 UI 收口（现有 tag_ids/sections 多选的视觉统一）；② 同义词/别名检索（terms 表联动，如"星际穿越/Interstellar"）；③ meilisearch 可插拔后端**评估报告**（先跑 perf_baseline 大数据量，PG 够则归档报告备用，不够再立项——对标 NP ES/Meili 双方案）。
**E10 i18n 扩展**：LOCALES 扩至 5 语（+ja/ru 或按收录反馈定）、语言包贡献流程文档（对位 Crowdin/Weblate 生态位，先用 GitHub PR 流程）、RTL 评估结论一页纸。
**E11 面板生态与公网件**：① 1Panel/宝塔部署指南（对位 NP 四路安装；以 Docker 方式装进面板为口径）；② 承接旧 #14 demo 站（只读账号+定时重置）与 #15 带量基准（announce_bench+10 万 peer 灌数出报告）——两项均依赖公网环境，与收录动作同批。

### E-P2（择机，5 项）

**E12 事件订阅矩阵**：在 ops_webhook/notice webhook 基建上，开放用户生命周期事件订阅（注册/晋级/封禁/H&R 触发/捐赠到账→webhook），让站长接自己的自动化（对位"运营工具化"趋势）。
**E13 附件全上云路径**：S3 双回落已有，补"全量迁移上云"工具+CDN 接入指南。
**E14 质量基建**：覆盖率基线（cargo tarpaulin/vitest --coverage，先出报告后设阈值）+ Playwright 浏览器层 E2E（承接 M 批响应式回归，替代部分人工浏览器验收）。
**E15 满级长线**：LV6 之后的长线目标（元老特权/公益展示位/传承邀请权重），联动"健康度四态"产品口径一起定（两条遗留口径合并处理）。
**E16 admin 数据大盘**：overview 升级（注册趋势/留存/做种健康度四态/经济通胀曲线已有经济仪表，补全站视角）。

### 明确不做（反定位清单）

| 不做项 | 理由 |
|---|---|
| 实时聊天/IRC/Chatbox（UNIT3D 方向） | 运维与审核负担大、非建站核心，webhook+论坛+工单已覆盖沟通 |
| 内置网盘/云播/在线影院 | 偏离 tracker 本体，法律与带宽风险失控 |
| 闭源付费插件/知识付费墙 | 与 MIT 开放策略冲突；商业化走"托管/服务"后手 |
| 多租户 SaaS 托管化改造 | 现阶段产品未稳定，先做最好的单站系统 |
| Kubernetes/Helm Chart | compose+multi-node+三机配方已覆盖 90% 场景；触发条件：≥3 个真实部署诉求 |
| 匿名开放注册默认化 | 定位是私有站建站，匿名默认与 H&R/考核体系矛盾 |

---

## 六、落地节奏与验收口径

1. **E-P0 五件**建议按 E1→E3→E2→E4→E5 顺序两周三批 commit（E1 最小最快先清安全缺口；E3 是"自由搭建"定位关键件；E2/E4/E5 是收录前弹药）。
2. **E-P1** 中 E6 必须先出大盘点报告再动代码；E11 与对外收录窗口绑定。
3. 每件沿用现有纪律：迁移号取最新+1（当前 0237，并行撞号高发，提交前再查）；i18n 基线 touch；守门脚本全绿；e2e 回归（31 脚本）+ 21/21。
4. 验收总口径：**E 批完成后，产品可在"仓库→安装→部署→使用→运营→成长→升级"全链每一环，对任一竞品给出不落后且多数领先的实证材料。**

---

## 七、资料来源（调研于 2026-09-28）

- GitHub API/仓库页：xiaomlove/nexusphp（及 releases/contributors/discussions）、HDInnovations/UNIT3D（含 book/src、config/、docker-compose.yml、SECURITY.md）、OPSnet/Gazelle（docs/INSTALL.txt）、WhatCD/Gazelle、torrentpier/torrentpier、Arcadia-Solutions/arcadia、torrust/torrust-tracker、Roardom/UNIT3D-Announce、greatest-ape/aquatic、PlexPt/rocket-pt、tdjsnelling/sqtracker、NJUPT-NYR/SOPT、JustLookAtNow/pt_mate
- 官方站：nexusphp.org（含 About/发版博客）、doc.nexusphp.org（installation/installation_docker/go_tracker/clickhouse）、hdinnovations.github.io/UNIT3D、hdinnovations.github.io/HDInnovations（付费价目）、sunset.torrentpier.com、hosted.weblate.org/engage/unit3d
- 漏洞库：NVD（NexusPHP 31 条 CVE）、GitHub Security Advisories、OSV（UNIT3D 0 条）
- 口碑注记：r/trackers 原帖因网络环境未能直接核实，站长痛点部分采用可核实的间接证据（付费价目表、官方文档自述、issue 时间线）；文中已逐处标注
- 本仓现状：三路 Explore 只读盘点 + 代码抽验（promo.rs 促销 6 类×4 域、tracker BEP-7 peers6、/classes 无进度元素、openapi/compat 层在位）
- 内部输入：_doc/开源生态全方位对比分析与完善策划案-2026-09-27.md（C 批）、通用建站闭环审查-2026-09-26.md、通用建站定位符合度审查-2026-09-25.md、闭环五路方案复核附记、G30/G31 系列配方与实证记录
