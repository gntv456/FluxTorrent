# FluxTorrent 反作弊运营层与注册治理实现拆解

> 对标 PT Studio `anticheat`（SRS-ACH 44 FR）+ `registration-policy`（SRS-REGP 86 FR），
> 基于 FluxTorrent 真实代码盘点（2026-10-14）的落地拆解。**本文是拆解稿，零代码改动。**

---

## 0. 结论先行

**FluxTorrent 的反作弊检测层已经比 PT Studio 规划的还深**（计费四闸 + BT 三阶段实证 +
五类行为画像 + 名单准入，全部阈值 site_settings 化、证据落 `cheat_events`），
**缺的整整齐齐是运营层四件套**：

| 缺口 | PT Studio 对应 | FluxTorrent 现状（证据） |
|---|---|---|
| ① 案件管理 | FR-ACH-014~018 案件/队列/认领/详情 | `cheat_events` 是 `(user, agent, reason)` 扁平台账；resolve 只有二值 `resolved_at` + 一条 modify_log，多 staff 协同不可见 |
| ② 风险评分 | FR-ACH-010~011 评分/分段/观察名单 | hits 是单 agent 计数器且跨 agent 不互通，无法回答「这个用户总体多可疑」 |
| ③ 自动分级处置 | FR-ACH-019~027 处置策略/熔断/四眼 | L2 只发一封管理组信；无自动禁言/封号/剥夺下载权；held 量无补发接口 |
| ④ 申诉闭环 | FR-ACH-031~032 申诉/四眼复核 | ghost 事件文案承诺「可申诉」但代码零入口；staff resolve 是单向勾销 |

**注册治理已有一半**：registration_mode（invite_only/open/email_verify）+ 邮箱后缀
allow/deny 三模式（`email_policy.rs`）+ email_bans + ip_bans + 注册 throttle +
captcha_provider 配置 + application_signup 开关都在；**缺**：全站用户上限、
开放窗口/周期时段、一次性邮箱库、同 IP 注册数限制（隐私合规版）、答题注册。

### 总策略：三不抄、四长在

**不抄**：
1. **JSON DSL 规则集 + 影子 + 回测**（FR-ACH-006~009）——FluxTorrent 检测是代码化画像
   （ghost/speed/reset/corr/xreport_over/nearcap/self_deal/down_under/bitthief 九类），
   阈值已 site_settings 化。评分用**固定公式**（画像权重 × 命中函数 × 时间衰减），
   权重可调但不引入通用 DSL——避免为架构而架构。
2. **WASM 插件形态**——运营层是核心链路，直接进 worker/api 单体，不走插件沙箱。
3. **registration-policy 的申请制全套**（FR-REGP-060~072）——application_signup
   开关已存在且当前够用，申请制增强（结构化字段/证明链接/审核队列）排 M5 以后。

**长在**（不建平行体系）：
1. **案件长在 `reports` 表上**——它已有 `ref_type/ref_id/status/claimed_by/claimed_at/
   handled_by/handled_at` + 未决唯一索引，就是 PT Studio 案件队列的胚子。加 `ref_type='cheat'`
   语义 + 几个列即可，前端复用举报管理台模式。
2. **申诉长在 `appeals` 表上**——已有 `user_id/kind/ref_id/body/status/handled_by/
   result_note` + open 部分索引，加 `kind='cheat'` + 令牌列。
3. **处置长在已有原语上**——`users.warned_until/warned_reason`（警告）、
   `users.status`（封禁）、`ratio_watch_action`（停下载权先例）、
   seeding 黑名单（停做种收益，`seeding.rs:78-85` 已有）、`staffmessages`（站内信）。
4. **证据长在 `cheat_events` 上**——它就是证据表（含 hits/resolved_at/UNIQUE(user,agent,reason)）。

---

## 1. 分阶段计划

### M1 案件管理（cheat_events → 可协同的案件）★ 优先

**目标**：把扁平台账变成 staff 能分派、能协同、有处理记录的调查案件。

**迁移（0367，幂等）**：
- `reports` 加列：`priority smallint NOT NULL DEFAULT 0`（0 普通/1 加急/2 危急）、
  `risk_score int`、`evidence_ref text`（关联 cheat_events 的 agent 或 user_id）、
  `resolution text`（dismiss/no_action/sanction 之外的自由文本，或复用 modify_log）、
  `sla_breached bool`（由 worker 置位）。**不加新表**。
- `cheat_events` 加列：`case_id bigint REFERENCES reports(id)`（证据归入案件，
  可空——未开案的证据暂时游离）。
- 索引：`idx_reports_priority`(status, priority DESC, created_at)、
  `idx_cheat_events_case`(case_id) WHERE case_id IS NOT NULL。

**API（`apps/api/src/http/cheat_case.rs`，新，守 300 行）**：
- `POST /admin/cheat-cases/open`：从 cheat_events（或指定用户）开案——写入 reports
  （ref_type='cheat'）+ 回写 case_id。护栏 `ensure_outranks`（不能给自己/上级开案）。
- `GET /admin/cheat-cases`：队列（status=open/claimed，按 priority DESC, created_at），
  聚合每个案件的关联证据数（cheat_events count）、用户近期画像命中。
- `POST /admin/cheat-cases/{id}/claim`、`/note`（staff 备注时间线——reports 无备注表，
  **M1 用 audit_detail 追加**，不建 notes 表；若不够再 0368 加 case_notes）、
  `/resolve`（复用现有 `/admin/cheat-events/resolve` 的处置语义并扩展：
  处置时可选「同时警告/停收益」）。
- worker `cheat_enforce.rs` 扩展：L2 命中（hits≥5）时**自动开案**（不止发信），
  带 priority=1；同用户进行中案件则追加证据不新开（`uq_reports_open_per_reporter`
  的语义要重新定义——cheat 案件的去重键是 (ref_type='cheat', ref_id=user_id)，
  **迁移里要把该唯一索引的 WHERE 条件排除 ref_type='cheat'**，否则举报去重会误伤案件）。

**前端**：`/cheaterbox` 页（cheater-box.tsx 已有）加「案件队列」面板；
staff-tools-site 的 cheat-events 行加「开案」按钮。

**验收**：两名 staff 同时认领同一案件只有一人成功（reports.claimed_by 已有乐观语义）；
L2 命中自动开案；案件详情能看到该用户全部 cheat_events（按 agent 分组）。

**为什么 M1 优先**：后面所有阶段（评分、处置、申诉）都以「案件」为锚——
PT Studio 的 score→case→sanction→appeal 链路里，case 是中枢。

#### M1 落地记录（2026-10-14，0367 已应用）

与计划的偏差及实际形态：
- **case_notes 表直接进了 0367**（没等 0368）：首版复盘认为 audit_detail 是
  操作审计流（不可改、面向合规），处理过程是协同载体，两者分工不同，混用会
  污染审计语义。表带 ON DELETE CASCADE 随案件归档。
- **`/resolve` 未做**：单条证据处置沿用既有 `/admin/cheat-events/resolve`；
  案件级 resolve（关案）留到 M3 与处置/申诉一并设计（含关案时冻结证据快照）。
- **前端落点**：`/cheaterbox` 页内三块——可疑会话（原样）、作弊事件待办
  （cheat-events-table.tsx，行内加「开案」按钮）、案件队列（cheat-case-panel.tsx
  + cheat-case-detail.tsx，tab 切 open/claimed/done，展开看证据+时间线+备注）。
- **worker**：cheat_enforce 每 tick 先做 SLA 置位（`make_interval(hours => 72)`），
  L2 分支内自动开案（INSERT...SELECT MIN(staff) 署名 + ON CONFLICT 挡重 +
  证据归案 + 首条系统备注）；与 7 天告警去重相互独立——案件 done 后新证据再达
  L2 线会开新案（唯一索引只挡 status IN (0,2)）。
- **API 形态**：`http/cheat_case/` 目录四件（mod 开案+认领+备注 / list 队列 /
  detail 详情 / mount 挂载），300 行门禁所致；路由经 `mount_cheat_cases` 挂进
  v1_scope。开案 duplicate 不报错，返回 `{duplicate: true}` 供前端静默刷新。
- **验证**：api 322 / worker 47 单测全绿；ON CONFLICT 索引推断已用真库实测
  （首插建案、同 ref_id 二插 0 行、举报路径对照组通）；`_smoke_cheat_case.py`
  覆盖开案/去重/队列/详情/备注/双 staff 认领互斥/SLA 置位/L2 自动开案/幂等。

### M2 风险评分与观察名单

**目标**：回答「这个用户总体多可疑」，给队列排序提供 priority 依据。

**迁移（0368）**：新表 `user_risk_scores`：
```sql
user_id bigint PK REFERENCES users(id),
score int NOT NULL DEFAULT 0,        -- 0~100
band text NOT NULL DEFAULT 'none',   -- none/low/watch/high/critical
shadow_breakdown jsonb,              -- 各画像贡献分（可解释性）
ruleset_ver int NOT NULL DEFAULT 1,  -- 权重版本（不用 DSL，用版本号）
updated_at timestamptz NOT NULL DEFAULT now()
```
+ 索引 `idx_risk_band`(band, score DESC) WHERE band != 'none'。

**公式**（固定，权重 site_settings 化——`risk_weight_{agent}` 系列键，
agent = ghost/speed/reset/corr/xreport_over/nearcap/self_deal/down_under/bitthief）：
```
score = min(100, round( Σ_agent w_a × f(hits_a) × decay(days_since_last_a) × 100 / NORM ))
```
- `f(hits) = 1 + log2(hits)`（封顶 hits≤16），`decay = 2^(−days/半衰期)`（默认 14 天，
  `risk_decay_days`）。这是 PT Studio FR-ACH-010 的**简化**：同样的
  「权重×命中×衰减」骨架，砍掉证据规则/聚合规则/乘数规则三层 DSL。
- 分段阈值 `risk_band_{low,watch,high,critical}`（默认 20/40/60/80）。
- watch 及以上 = 观察名单（staff 可见，用户无感）；high/critical 且无进行中案件 →
  自动开案（priority=1/2）——即 M2 给 M1 的自动开案补上「非 L2 路径」的触发源。

**worker**（`jobs/risk_score.rs`，新）：每日 cron 对近 90 天有证据的用户重算 +
新证据增量重算（复用 cheat_events 写入点）；分段变化写时间线（audit 或
risk_score_history，M2 先不建 history 表，分段变化广播 staffmessages 即可）。

**API**：`GET /admin/risk-scores`（观察名单，分 band/排序/过滤）；
`GET /admin/users/{id}/risk`（用户维度分数构成）。

**验收**：构造 ghost×5 + speed×3 的用户分数 > 只有 ghost×1 的用户；
30 天无新证据分数衰减过半；分段跨越阈值自动开案且不重复开案。

#### M2 落地记录（2026-10-14，0368 待随 api 构建应用）

与计划的偏差及实际形态：
- **worker 两个轮次拆成 catalog 两条**（`risk_score` 10m 增量 / `risk_score_daily`
  24h 全量），面板均可手触。增量脏集 = 无分数行 / 新证据(last_seen) /
  新处置(resolved_at) / 分数行超 1h 未重算——最后一条是 decay 的支点：
  **无新证据也必须周期重算**，否则分数冻结（验收项「30 天衰减过半」
  依赖它）。全量轮是权重调参后的安全网。
- **upsert 门禁只挡 score/band 未变**：未变则不写 ⇒ updated_at 停留 ⇒
  1h 后落入脏集。若每轮必写，updated_at 永远新鲜，无新证据的用户再也
  不进脏集——decay 机制会死。shadow_breakdown 随分数变化刷新。
- **只计入 resolved_at IS NULL 的存活证据**（staff 处置即解除）；职员
  （class≥90）不计分；0 分行不删（删了会被增量轮「无分数行」分支插回，
  造成删-插 churn）。
- **档位广播收敛**：仅跨观察名单线（进出 watch+）与升入 critical 才发
  staffmessages（7 天按 (user, band) 去重）——none↔low 太噪且无运营意义。
- **API**：`http/risk.rs` 单文件两端点（`GET /admin/risk-scores` 观察名单
  + `GET /admin/users/{id}/risk` 构成+证据明细），经 `mount_risk` 挂载；
  无分数行回落 0/none 不 404。手动开案同步补 risk_score 快照
  （与 worker auto_open_case 同口径）。
- **run.rs 净增行数归零**：抽 `run_consumer` helper 收敛三个 consume_* 块
  （消费者组语义不变），门禁 322 ≤ 基线 323。
- **前端**：`risk-watchlist.tsx`（watch+/全部切换 + 构成展开）挂进
  cheater-box；案件队列加「风险分」列（开案快照）。
- **公式**：score = min(100, round(Σ w×(1+log2(min(hits,16)))×2^(−days/半衰期)×100/NORM))；
  家族 = agent 冒号前缀（deal:→self_deal）；验收口径 ghost×5+speed×3≈67 分
  （high 档，自动开案 priority 1）vs ghost×1≈13 分（none）。

### M3 处置与申诉闭环

**目标**：处置可执行、可到期、可撤销；用户能申诉、staff 能四眼复核。

**迁移（0369）**：
- `appeals` 加列：`token_hash text`（被封用户令牌申诉，sha3-256）、
  `case_id bigint REFERENCES reports(id)`、`decided_by`（=handled_by 已有，不加）、
  `outcome text`（upheld/mitigated/overturned）。
- `users` 加列：`cheat_banned_until timestamptz`（作弊停权，与 status=2 全封区分：
  停权 = 禁做种收益 + 禁下载，保留登录与申诉能力——**这是比 PT Studio 更合适的档位**，
  私有站「停权观察」比全封常见）。

**处置动作**（POST /admin/cheat-cases/{id}/resolve 的 sanction 选项，全部接线已有原语）：
| 处置 | 落地 | 到期解除 |
|---|---|---|
| 警告 | `users.warned_until/warned_reason` + 站内信（Gazelle 顺延语义：从现有到期时刻起算） | worker 每日扫到期清除 |
| 停做种收益 | 已有：未处置 cheat_events 即停（seeding.rs），**resolve 时对选定证据置 resolved_at 即解除** | 即时 |
| 停权（禁做种+禁下载） | `users.cheat_banned_until` + announce 侧/download 侧查表 | worker 到期清 |
| 封禁 | `users.status=2`（已有，四眼：第二人批准——`self_unban_used` 已有自助解封先例，作弊封禁不走自助） | 手动 |
| 回收虚报流量 | `traffic_ledger` 负向调整（已有 operator_id/reason 列）——补上 M0 盘点发现的「held 量无补发接口」缺口 | 申诉撤销时反向 |

**四眼**：封禁与申诉复核，决定人 ≠ 批准人（`ensure_outranks` 已有框架扩展）。

**申诉**：
- 登录用户：`GET /p/my/appeals` + `POST /p/appeals`（kind='cheat', case_id, body ≤4000 字）。
- 被封用户：封禁站内信带一次性令牌（30 天，只存 hash）→ 公开页 `POST /p/appeals/token`。
- staff：`GET /admin/appeals?kind=cheat` + `/resolve`（outcome + result_note；
  overturned 时解除全部处置 + 流量反向恢复 + 案件标 dismissed + 误判证据标记
  ——cheat_events 加 `false_positive bool` 列，供 M4 统计）。

**验收**：警告顺延叠加正确；停权到期自动恢复；overturned 后收益恢复+流量回滚；
决定人复核自己的申诉被拒；令牌一次性。

### M4 注册治理补强

**目标**：把 registration-policy 的 P0 关卡补到 FluxTorrent（只补 P0，P1 题库/申请制以后）。

**迁移（0370）**：
- 新表 `reg_ip_signups`：`ip_hash text`（sha3-256(ip)，**不存明文**）、
  `user_id bigint`、`created_at timestamptz`；索引 (ip_hash, created_at)。
  ——同 IP 注册数限制的隐私合规版：只存不可逆摘要，30 天清理（worker cron）。
- 新表 `disposable_email_domains`：`domain text PK`、`source text`（builtin/import）、
  `created_at`；内置库随迁移 seed ~2000 常见一次性域名（或首次启动时从常量灌）。
- `site_settings` 新键：`max_users`（0=不限）、`reg_window`（JSON：窗口起止+名额，
  0 字节起不引入窗口表——窗口是低频管理动作，settings JSON 够用）、
  `reg_ip_limit`（同 IP 窗口内最大注册数，默认 2）、`reg_ip_window_hours`（默认 24）。

**register.rs 改动**（现有钩子点已齐：`ip_banned`、`throttle`、`email_policy`）：
- `user_cap` 闸：注册事务内 `SELECT count(*) FROM users WHERE status < 2` + max_users
  比较（**行锁防超发**：`SELECT ... FOR UPDATE` 或 advisory_lock——NexusPHP 的
  检查/插入竞态正是 PT Studio 改进点，必须锁）。
- `window_slots` 闸：reg_window JSON 解析（起止+剩余名额，名额用 Redis 计数
  `reg:window:{id}` + TTL 到窗口结束）。
- `ip_limit` 闸：`reg_ip_signups` 查 ip_hash 窗口计数。
- `disposable` 闸：邮箱域名查 `disposable_email_domains`（在 email_policy 之后，
  与 email_bans 同层——「一次性邮箱」是准入整类门）。

**后台**：settings 页加四键的编辑面（复用 settings_http 分组机制）；
一次性邮箱库导入导出（POST /admin/email-domains/import，≤10 万条幂等）。

**验收**：max_users 达到后 open/invite 通道关闭（invite_only 模式也受限——
Gazelle USER_LIMIT 语义）；窗口外注册被拒且返回下次开放时间；同 IP 第 3 个账号被拒；
disposable 域名被拒；`reg_ip_signups` 无明文 IP。

### M5（原计划排期以后；2026-10-14 已落地答题闸 + PoW + 规则准确率）

答题注册（题库+抽题+判分票据）、PoW 验证码、申请制增强（结构化字段+证明链接+
审核队列+建号）、anticheat 同 IP 对端关联（**注意：announce_ips 表 + self_deal
画像已存在**，这条 PT Studio 列为 P2 的能力 FluxTorrent 检测层已经有了，
运营化只是把 self_deal 证据接进案件链路，随 M1 自动完成）、规则准确率统计
（M3 的 false_positive 列 + 案件结论即可算出）。

---

## 1a. M4 落地记录（2026-10-14，0370/0371/0373 已应用）

| 计划项 | 实际形态 | 偏差说明 |
|---|---|---|
| maxusers 行锁防超发 | `repo/auth.rs` 注册事务内先 `pg_advisory_xact_lock(0x7265_675f_6361_7001)` 再 `count(*) WHERE status < 2` | 用 advisory 事务锁而非 `SELECT FOR UPDATE`——计数查的是聚合，没有单行可锁 |
| 注册窗口名额 | `reg_window` JSON（open/close/slots/label）**纯 settings**，未引 Redis 计数 | 名额按「窗口内已注册数」算；窗口是低频管理动作，JSON 够用，少一张表一个键 |
| 同 IP 限额 | `reg_ip_signups(ip_hash, user_id, created_at)`，ip_hash = sha3-256 hex | 只存摘要无明文；`reg_ip_retention_days` 默认 30 天，worker `reg_cleanup` 清 |
| 一次性邮箱 | `disposable_email_domains(domain PK, source)`，内置 **445** 个域名 | 计划写「~2000」，实际按常见清单收敛到 445（够了，站长可导入扩充）；匹配含**子域后缀**（`$1 LIKE '%.' || domain`） |
| 后台导入导出 | `/admin/email-domains` 四端点（list/export/import/delete），单批 ≤10 万 | 归一化容忍粘贴整邮箱/URL（取 @ 后段、去 scheme/路径/端口） |
| settings 编辑面 | 0371 补 `settings_meta` 两行（注册与邀请组），**只补 hint 不改 group_key** | 四个新键 + M3 的两个键一次补齐 |

前端：`?tool=emaildomains`（staff-tools-security-domains.tsx）+ 0373 导航行；
列表/搜索/导入（纯文本或 JSON）/导出 JSON/删除。同批把安全域面板的
`load()` 改为**按 tab 取数**——原先无条件三连拉（bans/mail/emailbans），
新增第 5 个 tab 会白付 2-3 个请求。

验证：`_smoke_reg_gates.py` **17/17 全过**（四道闸 + 子域匹配 + 摘要无明文
+ 导入导出 + reg_cleanup cron + 403）。

## 1b. M5 落地记录（2026-10-14，0372/0373 已应用）

M5 计划写「排期以后」，本次一并落地**答题闸 + PoW + 规则准确率**；
申请制增强（结构化字段/证明链接/审核队列）按原计划不做。

| 项 | 形态 | 要点 |
|---|---|---|
| 题库 | `reg_quiz_questions`（单选，options JSONB 数组，answer_index 0 基）+ 14 道内置站规题 | 选项 2-6 个；题干按**字符**计长（4-1000），字节判会把中文题自拒 |
| 抽题票据 | `GET /auth/quiz-challenge` → Redis `sha3/quiz:{ticket}` = `{qid}:{answer}`，**只回选项不回答案** | 票据一次性（GETDEL）；TTL `reg_quiz_ttl_minutes` 默认 10 分钟；抽题口自身限流 30/min |
| 判分 | `quiz_verify` → 比对下标 + 落 `reg_quiz_attempts`（IP 只存 sha3） | 双闸：`reg_quiz_enabled`（是否启用）+ `reg_quiz_pass_required`（答错是否拦） |
| PoW | `pow-challenge?scope=register` 绕过 acctlock 触发线 | 登录链路仍要求失败 ≥2 次才发题（无探测价值）；注册链路主动求解 |
| 准确率 | `GET /admin/cheat-accuracy` | 家族级 `1 - fp/resolved`（**下界**：fp 只在申诉 overturned 时回填）+ 处置结论分布；零样本给 null 不给 100% |
| 后台面 | `?tool=regquiz`（题库 CRUD + 批量导入 + 近 N 天通过率）、cheaterbox 内嵌规则准确率面板 | 0373 补 regquiz 导航行 |

**没建新表**：准确率全部由 M1~M3 的 `cheat_events.false_positive` +
`reports.sanction` 现算——这正是 M5 原文「M3 的 false_positive 列 + 案件结论
即可算出」的字面落地。

前端：注册页新增 `RegisterQuiz` / `RegisterPow` / `RegisterCaptcha` 三块
（`components/register-gates.tsx`，把原内联验证码块一并收进来给 300 行门禁
腾位）；`/register-mode` 增补 `reg_quiz_enabled` / `reg_quiz_pass_required`
公开开关。提交失败时用 `gateNonce` 重挂载三块——验证码/答题票据/PoW 解
全是 GETDEL 一次性语义，不重抽第二次提交必然 400。

验证：`_smoke_reg_quiz.py` **19/19 全过**（题库 CRUD/非法拒绝/去重导入/
抽题不含答案/答对通过/答错拦截/票据重放被拒/软闸/缺票被拒/PoW 求解与拒绝/
家族准确率 0.5/处置分布）。

## 1c. 反作弊运营层总览（M1~M5 全闭环）

| 层 | 入口 | 落地 |
|---|---|---|
| 案件 | `/cheaterbox` 案件队列 | reports(ref_type='cheat') + case_notes 时间线 + 双 staff 认领互斥 |
| 评分 | `/cheaterbox` 观察名单 | user_risk_scores（权重×命中×衰减，worker 双轮：10m 增量 / 24h 全量） |
| 处置 | 案件详情 → resolve → approve | sanction 状态机 + 四眼 + 停权 `cheat_banned_until`（禁下载/停收益，到期自恢复） |
| 申诉 | `/appeals` + 封禁站内信令牌 | 登录自助 + 一次性令牌（只存 hash）+ 四眼复核 + overturned 反向解除 |
| 注册治理 | `/admin?tool=emaildomains` 等 | 四道闸 + 答题闸 + PoW + 题库/域名库后台 |
| 可观测 | `/cheaterbox` 规则准确率 | 家族误判率 + 处置结论分布（无新表） |

---

## 2. 迁移与工期一览

| 阶段 | 迁移号 | 新表/加列 | 新模块 | 状态 |
|---|---|---|---|---|
| M1 | 0367 | reports+3列 / cheat_events+case_id / 改 uq 索引 | http/cheat_case.rs、worker cheat_enforce 扩展、前端案件面板 | 已上线 |
| M2 | 0368 | user_risk_scores | worker/jobs/risk_score.rs、http/risk.rs、settings 权重键 | 已上线 |
| M3 | 0369 | appeals+3列 / users+cheat_banned_until / cheat_events+false_positive | appeals 扩展（kind=cheat）、令牌申诉公开端点、处置落地 + `sanction_sweep` worker | 已上线 |
| M4 | 0370 + 0371 + 0373 | reg_ip_signups / disposable_email_domains / settings 四键 / settings_meta / 导航行 | register_gates 四道闸、email_domains 后台、reg_cleanup cron、`?tool=emaildomains` 面板 | 已上线 |
| M5 | 0372 + 0373 | reg_quiz_questions / reg_quiz_attempts / settings 五键 / 导航行 | reg_quiz 抽题判分、pow `scope=register`、cheat_accuracy、`?tool=regquiz` 面板 + 注册页答题/PoW | 已上线 |

> 实际迁移号：**0367~0373**（0371 = settings_meta 编辑面，0373 = 后台导航行）。
> 原表里的 M4「settings 四键」实际落成 0370（键）+ 0371（编辑面）两笔。
> 迁移纪律：建号前 `ls apps/api/migrations/ | awk -F_ '{print $1}' | uniq -d` 查撞号；
> 新 SQL 幂等（会重跑）；改 Rust 必 build api/worker + `up -d --no-deps api`；
> 门禁：300 行/80 字符/check_type_drift/admin_nav_align_guard。
> ⚠️ 干跑迁移要用 `psql -1`（单事务）——裸 `sed s/^COMMIT;/ROLLBACK;/`
> 在 psql 默认的逐句自动提交下**不会回滚**，等于直接落库（本次踩过）。

## 3. 与既有体系的一致性检查

1. **reports 唯一索引**：`uq_reports_open_per_reporter (ref_type, ref_id, reporter_id)
   WHERE status = ANY(ARRAY[0,2])`——cheat 案件的 reporter 是系统（开案 staff），
   需在 0367 里给该索引加 `AND ref_type <> 'cheat'` 或用 case 专用部分索引替代，
   否则「同用户同 staff 重复开案」被误拦、「不同 staff 开同用户」又拦不住。
   正确语义：**同用户至多一个进行中 cheat 案件**（ref_type='cheat', ref_id=user_id）唯一。
2. **权限点**：cheat-events 已有 `notconnectable.view`/`cheaters.view` 先例，
   案件走 `cheater` 权限族扩展（cases.moderate / sanctions.manage 两级——
   复用 user_classes 权限串，不新建 RBAC）。
3. **停权与 announce 热路径**：`cheat_banned_until` 的读取点在 tracker 侧
   （announce.rs）还是 worker 侧（process_event）？**必须在 worker 侧**——
   tracker 不连业务库（架构现实），停权落地为「seeding_gate 加一条件 +
   download 端点查 users」——动手前先确认 download 链路取数方式（publish_http/download.rs）。
4. **与 ratio_watch 的关系**：ratio_watch 是「分享率不足」的柔性观察（警告→停下载），
   cheat 停权是「作弊嫌疑」的处罚，两条链共用 warned_until 会互相覆盖——
   M3 要给 cheat 停权独立列（cheat_banned_until 正是为此），警告类也分开记
   （warned_reason 前缀区分）。
5. **补量发还**：M0 盘点发现 audit_hold.rs:38 注释声称存在但未接线。M3 的流量回收
   要把「held 量查询 + 复核后补发」一起做（traffic_ledger 已有 operator_id/reason）。

## 4. 风险

1. **M1 的唯一索引改造**影响现有举报链路——迁移要在事务里先建新索引再删旧的，
   且回归测试举报去重（uq_reports_open_per_reporter 现有行为不能坏）。
2. **自动开案的误伤面**：L2 自动开案只是「进队列」不自动处罚（PT Studio 的
   结论性规则默认也只冻结），M1/M2 都不做自动处罚，自动处罚留到 M3 且默认关
   （`auto_sanction_enabled` 默认 false）。
3. **证据表增长**：cheat_events 已有 UNIQUE(user,agent,reason) 聚合，量可控；
   user_risk_scores 是用户数行级，无增长问题。
4. **隐私**：reg_ip_signups 只存 hash；appeals 令牌只存 hash——与 PT Studio
   NFR-REGP-006 同标准，动笔前过一遍 _doc 里的隐私 checklist（如有）。
