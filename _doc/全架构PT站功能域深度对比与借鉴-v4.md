# 主流 PT 架构全功能域深度对比与借鉴（v4）

> **定位**：本文是 v3（2026-09-13）的**收敛版**。v3 之后 48 小时内我们连落六批（0072-0078，13 个 commit），v3 借鉴清单 P0×2 / P1×10 / P2 高价值 7 项全部落地。v4 回答三个新问题：
> 1. **落地后的实况**——每项借鉴落地后跑出了什么数据、暴露了什么新问题（v3 只有"该抄什么"，v4 有"抄完的账本"）；
> 2. **剩余真缺口**——v3 清单里哪些还开着，优先级如何重排；
> 3. **新增对照面**——v3 未覆盖的三块：Rust tracker 同代（Torrust/Aquatic 细化）、移动端生态（PT Mate 37 站清单）、TorrentPier 之死（俄语圈第二大引擎 2026-05 归档的验尸报告）。
>
> **证据基础**：v3 的全部一手材料（NP/GZ/U3D/TBSource/XBTIT/M-Team/YemaPT/朱雀 源码级）+ 本轮新增：Torrust 官方仓库（2026-09 快照）、TorrentPier 官方仓库（归档公告原文）、PT Mate（JustLookAtNow/pt_mate README+站点清单）、我方 2026-09-14 快照（HEAD e191c03：**162 表 / 399 路由 / 54 页面 / 78 迁移 / 40 worker 任务函数 / 68 权限点 / 319 site_settings / 17 种火花流水**）。
> 证据分级沿用 v3：`[实证]` 源码或官方文档原文；`[业内]` 多源交叉；`[推断]` 标注的合理推演。

---

## 0. 我方基线更新（v3 → v4，48 小时 delta）

| 指标 | v3（09-13） | v4（09-14） | delta | 来源 |
|---|---|---|---|---|
| 数据表 | 116 | **162** | +46 | information_schema 实测（含 0072-0078 新表） |
| 唯一路由 | 299 | **399** | +100 | grep service() 去重 |
| 页面 | 54 | 54 | 0 | page.tsx 计数（新功能暂走既有页/端点） |
| 迁移文件 | 69 | **78** | +9 | 0070-0078 |
| worker 任务 | 14 | **40**（函数级） | +26 | jobs.rs+bank_jobs+task_jobs |
| 权限点 | 68 | 68 | 0 | authz.rs perm 常量 |
| site_settings | ~200 | **319** | +119 | 含 0078 gift_tax_bp 等 |
| 火花流水种类 | ? | **17** | — | spark_ledger GROUP BY |
| 工具生态位 | 未收录 | 未收录 | 0 | PTPP 收录 PR 仍待人工提交 |

**48 小时新增的功能域**（全部有 v3 对照物）：被拒禁发/免审通道（NP）、自动促销规则表（U3D）、盒子打标（U3D）、peer 超时分档（U3D）、火花产出回收对账（自家审计遗留）、Torznab caps（cross-seed 生态）、做种收益稀有度衰减（NP/GZ）、教材愿望单（U3D 教育化）、FL 券/中性券（Gazelle）、复活任务（U3D）、等级晋升待遇（NP）、聚合组推荐入组+组订阅（GZ）、通知偏好（U3D）、站免池荣誉榜（AB）、定向众筹（HDBits）、泄露检测（U3D）、论坛已读（NP）、工单体系（U3D）、Refundable（U3D）、后台运维三件（U3D）、聊天机器人（U3D NerdBot）、赠送税（Gazelle 奖池税）。

---

# 第一部分（新增）· 同代与终局对照

## 1N. Torrust：Rust tracker 的"组件化"路线 vs 我们的"一体化"路线 `[实证]`

Torrust（AGPL-3.0，Rust+Axum）是开源世界里与我们技术栈最近的 tracker 项目。对照：

| 维度 | Torrust tracker | **我们 tracker** |
|---|---|---|
| 协议面 | UDP 6868/6969 + HTTP 7070/7171 + TLS，BEP 3/7/15/23/27/48 | HTTP announce/scrape（BEP 3/7/23/48 对齐），**无 UDP** |
| 持久化 | SQLite3/MySQL/PG **三选一可插拔** | 零 DB（纯 Redis Stream + 内存 DashMap） |
| 管理面 | 独立 Management API（1212 端口，token 鉴权） | 无独立管理面（settings 走 API 容器） |
| 可观测 | **Prometheus metrics + Grafana 官方仪表盘** | tracing 日志（无 metrics 出口） |
| 定位 | 纯 tracker 组件（配独立 torrust-index 才是站） | 三代计费架构的内嵌件（announce→Stream→worker 落账） |

**两点值得抄**：① **Prometheus/metrics 出口**——我们的 tracker/api 目前只有 tracing 日志，压测（announce_bench.py 待执行）之后没有时序指标就无法谈容量规划；Torrust 的"metrics 是一等公民"应作为我们 observability 的最小目标（actix-prometheus 或 metrics-rs，一个 middleware 的事）；② **UDP tracker（BEP 15）**——Aquatic 的 UDP 225 万 resp/s 说明 HTTP announce 的性能天花板远低于 UDP；但**教育站场景不需要**（客户端以 qB/TR 为主，HTTP 够用），列 P3 观察。

**一点必须不抄**：Torrust tracker 把 PG 作为**可插拔后端之一**（SQLite/MySQL/PG 三选一），意味着它的持久化抽象注定面向"最简公共集"；我们的赌注相反——**PG 特性用到极致**（分区表/jsonb/trgm/物化视图），这是性能与功能密度的来源，不值得为可移植性自缚。

## 2N. TorrentPier 归档验尸（2026-05-18）：论坛中心制的第二次死亡 `[实证]`

俄语圈第二大 PT 引擎（PHP 无框架+MySQL，论坛中心模型）于 2026-05-18 归档：官方公告"3.0 (Ox) 为最终版，不再有补丁/安全修复/功能开发"，后继者 **Dexter 计划 2027 年从零重写**。这是继 Discuz-PT 之后**论坛中心制的第二次确认死亡**，验尸结论：

1. **无框架裸 PHP 撑不过 15 年**——与 TBSource 同因：依赖树老化后每个 CVE 都是手工修；
2. **"论坛+种子附件"的内容模型天花板**——种子详情、促销、H&R 都被塞进论坛帖模型，功能债不可逆；
3. **重写接班（Dexter）而非渐进改造**——再次验证 v3 避坑#3"重写半途而废"的行业样本：宣布重写到发布隔 ≥2 年，期间社区流失。

**对我们的映射**：三条尸体分别对应我们已做的三个选择——Actix+sqlx 类型系统（vs 裸 PHP）、种子独立域+论坛只是社区件（vs 帖附种子）、增量交付永不宣布重写（vs Dexter 模式）。v4 把它写进避坑清单作为 Discuz-PT 的并列证据。

## 3N. PT Mate：移动端生态的第四张门票 `[实证]`

PT Mate（Flutter MD3，MIT）是移动端 PT 客户端里站点覆盖增长最快的一个，当前 37 站配置全部基于 `assets/sites/*.json` 声明式 schema。**关键事实：它的站点类型只有 6 种**——`Gazelle` / `M-Team` / `NexusPHP` / `NexusPHPWeb` / `RousiPro` / `Unit3D`。含义：

1. **移动生态的门票比桌面更集中**——PT-Plugin-Plus 148 站靠每站 config.json，PT Mate 37 站只靠 6 种类型 schema。一个站被收录的前提从"写配置"变成"**长得像六大架构之一**"；
2. 我们的 compat 层（NP 形状）理论上**顺带覆盖 PT Mate 的 NexusPHP 类型**——但 PT Mate 的 NP 类型走 DOM 解析（NexusPHPWeb 是免 DOM 的 API 版），所以"长得像"必须**页面也像**（列表行直链、详情页字段位置），这正是我们 Bearer blob 下载按钮曾经破坏、后来修复的东西。**教训泛化：compat 不只是 JSON 端点，页面 DOM 形状也是生态契约**；
3. Cookie Cloud 同步的封号风险文档（他们 README 用 1200 字警告）侧面印证我们不做"同 IP 多号处罚"（v3 §18）的正确性——移动网络+家庭宽带双 IP 是当代用户常态，处罚模型必须按行为不按 IP。

---

# 第二部分 · 落地实况：v3 借鉴项的运行数据与暴露的新问题

## 4N. 经济系统实测：产出口径变了，问题没变

v3 §27-22 落地后第一份月度对账（2026-09，进行中）：

```
产出（minted） 1,202,799   回收（burned） 291,792   净增 911,007（净增占产出 76%）
```

分 kind 拆（17 种）：**`increment_bulk` 105 万（87%）**、seeding_reward 5.8 万、admin 4.2 万、hr_pardon_refund 2 万、game 1.5 万——真实运营期产出大头是**批量发放**（注册奖励/活动），不是公式结算。回收侧 shop 25.5 万为大头（117 笔）。

**三个新发现**：
1. **回收率 24% 的结构性原因**——消费端只有 shop（25 万）+银行存款（1300）+H&R 免罪（2 万）。对照 NP：魔力消费 13 档商品+H&R 赎回+彩虹 ID+VIP 月卡……我们的 shop_items 目前只挂了少数 SKU。**结论：通胀治理的瓶颈不在"再加税"，在"消费目录深度"**——赠送税（0078，5% 起征）只是第一滴水；
2. **`hr_pardon_refund` 2 万暴露免罪券定价矛盾**——NP 口径 1 万魔力/条对应其收入结构；我们 20000 火花/条的实测效果是"有人用，但 shop 回收 25 万里它只占 8%"。v3 避坑说"免罪券要贵到肉疼"，20000 是否够肉疼要看运营数据（当前样本 1 笔，继续观察）；
3. **对账视图本身就是治理工具**——上线当天就发现 increment_bulk 占 87%，这种"一眼看到通胀源头"的能力是 v3 时代没有的。**建议常驻化：admin 首页挂净增率曲线**（v4 新增缺口 G17）。

## 5N. 通知偏好落地的教训（布尔缺省语义）

0075/0076 落地通知偏好时的逻辑反转事故（存 false 反而发送），已在 v3 附录外记录。v4 补充**结构性教训**：U3D 的 32 字段是**显式列**（migration 里一列一开关），我们选了 JSONB（`notice_prefs->key`）——灵活但把类型安全丢给了运行时。权衡正确（事件类还会涨），代价是**每个新通知类必须过"三态真值表自测"**（缺省/true/false），这条已写进 memory 纪律。对照：Gazelle 的 9 类型×4 通道是枚举表驱动，类型安全但加通知类要迁移——三家三种取舍，没有免费午餐。

## 6N. 定向众筹+赠送税的口径决策（v4 新记录，供后来者）

0078 落地时的两个非显然决策：

1. **税不记正向 spark_ledger**——支出方 `-amount` 流水已把含税全额记为回收，若给"池子"再记一笔 `+tax` 会虚增 minted（v_spark_flow_monthly 双计）。税只进 magic_pool/pool_donations 池账。**退款时按实付全额退（税由池承担）**——池子本来就是回收通道，承担退款税损与它的职能自洽；
2. **众筹 UNIQUE(funding_id, user_id) + 追投走 UPDATE 累计**——而非一人多行。E2E 因此暴露幂等键设计缺陷：idempotency_key 若含 funding_id+随机数，重试会双扣；我们最终用 `用户:项目` 语义键+spend_spark 幂等层双保险。

对照 Gazelle 的 request bounty（事务内直接扣上传量、余额不足回滚）：我们的众筹扣款复用 spend_spark（行锁+幂等先查后插），**口径一致但多了一层"项目状态机"**（status 0/1/2/3）——HDBits 原版没有退款态（凑不满就沉没），我们的"到期退款含税"是对教育站用户更温和的偏离，代价是税损由池承担（首例实测：税 80 火花全部由池吸收）。

---

# 第三部分 · 剩余缺口清单（v4 重排）

v3 §27 的 25 项中 19 项已落地，剩余 6 项 + 新增 3 项缺口，按新证据重排：

### 真缺口（按优先级）

| # | 缺口 | v3 出处 | v4 新证据/判断 | 优先级 |
|---|---|---|---|---|
| G1 | **PT-Plugin-Plus 收录 PR 提交** | §27-1 | 唯一剩余 P0。PT Mate 的发现（§3N）强化其价值：schema 收录有跨工具溢出效应。材料就绪（`.research/ptpp-config-template/`），纯人工动作 | **P0** |
| G2 | **announce 压测基线** | §27-19 | Torrust 对照（§1N）后更必要：无 metrics 无压测=容量规划全靠猜。脚本已备（scripts/announce_bench.py） | **P1** |
| G3 | **Prometheus metrics 出口** | v4 新增（Torrust 对照） | tracker/api 各挂一个 middleware 即可；是 G2 的数据载体 | **P1** |
| G4 | **上传 API（幂等口径）** | §27-23 | YemaPT 口径已写进开放API接入指南；教育站刚需弱于影视站（师资发布者少），降半级 | P2 |
| G5 | **规则页版本化（Wiki 化）** | §27-16 残留 | 教育站规则变更频率低，但"修订可追溯"对社区信任重要 | P2 |
| G6 | **成就四族起步** | §27-20 残留 | seed_milestones 已有骨架（保种量/救种数/发种数/论坛活跃四族），纯增量 | P2 |
| G7 | **POSTPONED 审核第四态** | §27-14 残留 | approval_status=3 被"下架"占用，需先腾挪语义再上，工程量小但动核心状态机 | P3 |
| G8 | **msg/notice App API + Telegram 绑定** | §27-23/24 | M-Team 口径；依赖 App 化决策，暂缓 | P3 |
| G9 | **UDP tracker（BEP 15）** | v4 新增（Torrust/Aquatic 对照） | 教育站客户端以 HTTP 为主，性能上限未成为瓶颈前不投入 | P3 |

### 新增缺口（本轮发现）

| # | 缺口 | 来源 | 说明 |
|---|---|---|---|
| G17 | **admin 首页净增率常驻曲线** | §4N 实测 | v_spark_flow_monthly 已有数据，缺可视化入口；通胀治理从"月度报表"升级为"常驻仪表" |
| G18 | **shop 消费目录扩容** | §4N 实测 | 回收率 24% 的主因是 SKU 太少；对照 NP 13 档商品（VIP 月卡/自定义头衔/彩虹 ID/免广告……）应有尽有。纯运营动作+少量商品效果代码 |
| G19 | **metrics 中间件+时序存储** | §1N | 与 G3 同源；时序库选型先问"PG 能不能做"（timescale 或裸表 5 分钟粒度都够教育站规模）——避坑#10 组件封顶纪律 |

### 已关闭项（v3 → v4 确认）

v3 §27 全部 P0（2/2）、P1（10/10）、P2 高价值 7 项、长尾 8 项（0078）——共 27 项落地，各自落地形态见 git log 0072-0078（`3a5285a`..`e191c03`）。每项的验证口径：单测锁定公式/三态真值表自测/E2E 真库真容器/回归 60/60。

---

# 第四部分 · 架构代际表（v4 终版）

四代范式维持 v3 判断，补两行终局注脚：

| 代 | 代表 | 状态（2026-09） |
|---|---|---|
| 一代·同步写库 | TBSource（2005）、Discuz-PT、**TorrentPier（2026 归档）** | 全灭。TorrentPier 是最后一个活体，Dexter 重写未至 |
| 二代·内存累计批量 flush | xbtt（停滞）、Ocelot（Gazelle 生态维持） | 存活但无新 adopter |
| 二代半·外挂 tracker+队列 | UNIT3D（Rust announce 3s UPSERT）、Torrust（组件化+三库可插拔） | 活跃。Torrust 是"tracker 独立商品化"的代表 |
| 三代·持久流+幂等消费者 | **我们**（Redis Stream→worker→分区流水） | 唯一。账目可审计/宕机可重放是结构性优势 |
| （旁系）纯内存极限 | Aquatic（UDP 225 万/s） | 性能天花板参照物，非站点引擎 |

**代际结论不变但加一条**：三代架构的优势不是性能（Aquatic 证明纯内存更快），是**账目可审计**——这对教育站（家长/学校可能问责的场景）比性能更值钱。守住 tracker 零 DB 边界（v3 §1 避坑）依然是第一纪律。

---

# 第五部分 · 避坑清单 v4（v3 十四条 + 新增四条）

v3 §28 的 14 条全部维持，新增：

| # | 坑 | 尸体/证据 | 我们的防线 |
|---|---|---|---|
| 15 | **论坛中心制内容模型** | TorrentPier 2026-05 归档（Dexter 重写 2027 才来）；Discuz-PT 教育网遗存 | 种子是独立域，论坛只是社区件；任何"把种子功能塞进论坛帖"的需求直接拒 |
| 16 | **宣布重写而非渐进交付** | TorrentPier→Dexter（宣布到落地 ≥2 年空窗）；TBSource BrokenWings | 永不宣布重写；架构演进走迁移+双写过渡 |
| 17 | **可移植性自缚** | Torrust 三库可插拔（功能受最简公共集拖累）；对照我们把 PG 特性用到极致 | 明确"PG-only"是选择不是债务；新需求先问 PG 能不能做 |
| 18 | **builder 镜像浮动 tag** | 自家 0078：rust:1.97-slim 上游换 trixie 底→GLIBC_2.39 运行失败 | 基础镜像一律钉死到发行版代号 tag（bookworm），CI 里加 `docker run --rm img cat /etc/debian_version` 冒烟 |

---

# 第六部分 · 一句话总结（v4）

> v3 说"用户不会因为你的架构先进而原谅你缺一张 FL 券"——48 小时后 FL 券、众筹、免罪券、通知开关、已读、工单、机器人全都有了，但**第一份对账告诉我们：道具的深度（shop 目录）比道具的种类更重要**，而 TorrentPier 的归档提醒我们：架构代差赢下的是生存权，运营深度赢下的才是留存。剩余缺口里只有一张真门票（PTPP 收录 PR）和一块真空白（metrics+压测），其余都是深度问题不再是广度问题。

---

## 附录 A · v4 新增引用坐标

| 对象 | 坐标 | 用途 |
|---|---|---|
| Torrust tracker | github.com/torrust/torrust-tracker（2026-09 快照） | §1N 组件化对照/可观测性/BEP 清单 |
| TorrentPier | github.com/torrentpier/torrentpier（归档公告 2026-05-18） | §2N 验尸/Dexter 后继 |
| PT Mate | github.com/JustLookAtNow/pt_mate README（37 站/6 类型） | §3N 移动生态门票/Cookie Cloud 风险文档 |
| 我方快照 | HEAD e191c03（2026-09-14）：162 表/399 路由/78 迁移/40 worker fn/319 settings/17 spark kind | 全文基线 |
| 我方账本 | v_spark_flow_monthly 2026-09（minted 1.20M/burned 0.29M）+ spark_ledger 17 kind 分布 | §4N 实测 |

*整理：2026-09-14（v4）· v3 的 6 路源码级调研全部有效且不重复；v4 新增 3 路一手材料 + 48 小时落地实况。v5 触发条件：PTPP 收录通过 / 压测出数 / 月度对账满一个自然月。*
