# 设计系统推进总控框架（DS-MASTER-01）

> 成文：2026-10-13 · 文档编号 DS-MASTER-01 · 上游：DS-PLAN-01（v4）+ FRAME-PLAN-01
> 定位：**策划案与日常工作之间的"防偏航仪表盘"**。策划案回答"为什么与做什么"，本框架回答"现在在哪、下一步是哪、怎么证明没偏"。任何一次 UI/框架改动，先在本框架找到自己的坐标再动手；找不到坐标 = 该先改策划案。
>
> 使用方式：每完成一个工作包，勾选对应条目并填 PR 号；§3 仪表盘数字随阶段复算；偏离处理走 §7。

---

## 1. 目标锚（一切工作的判定基准，从两份策划案浓缩，改动需回到策划案）

| # | 锚点 | 出处 | 判定问题（每个 PR 自问） |
|---|---|---|---|
| A1 | 反馈有协议：写操作反馈只有 toast/ActionFeedback 两形态 | DS §3.1 | 我加的反馈是协议的还是手写的？ |
| A2 | 令牌是单一真源：非豁免 hex=0、裸断点=0 | DS §0.2 意图二 | 我写的颜色/断点从哪来？ |
| A3 | 四态是默认配置：加载/空/错/成功缺一即设计未完成 | DS §10.1 宪章 4 | 我这个数据 UI 的四态各长什么样？ |
| A4 | 高频直显低频收拢（IA 五项） | DS §11.3 | 我的信息放层级是数据决定的吗？ |
| A5 | 性能预算不可破 | DS §12.1 | 我的改动让首屏 JS 变大了吗？ |
| A6 | 契约先行：实现必须是契约，新端点先登记 | FRAME §3 | 我的路由在清单里吗？错误码在注册表吗？ |
| A7 | 分层宪法：H 禁 sqlx、S 禁 actix | FRAME §2 | 我的代码住对层了吗？ |
| A8 | 用例唯一：一个用例一个 services 函数 | FRAME §4 | 我是在复制还是在调用？ |

---

## 2. 阶段-工作包分解（WBS，两案全部任务的可勾选清单）

> 每个工作包=一个 PR 线；状态：☐ 待做 / ◐ 进行 / ☑ 完成+PR 号。**W-编号是唯一跟踪键**，PR 描述与 commit 必须引用。

### 阶段一：地基（DS §7）＋ FRAME F1 前置

- ☑ **W-01** Toast 系统（Provider+编程式调用+role/aria-live+移动端顶滑）——`components/ui/toast.tsx`；双 live region（polite/assertive）、悬停暂停、request_id 芯片、desc 二行、optimistic 变体（W-17 并入）
- ☑ **W-02** ActionFeedback + BusyLabel（行内三态）——`components/ui/action-feedback.tsx`
- ☑ **W-03** StatePanel + ErrorPanel（四态路由器 + request_id 一键复制）——`components/ui/state-panel.tsx` + `error-panel.tsx`；ApiError 增补 requestId 字段（api-client.ts）
- ☑ **W-04** SkipLink 全站 `<main id>` 锚点——`components/ui/skip-link.tsx`；根布局挂载，(main)/(auth)/login 三处 main 补 id
- ☑ **W-05** ui.css tone 令牌 + `--motion-*` 动效令牌 + reduced-motion 短路——base.css 词汇层 `--tone-*`（16 个）+ `--motion-fast/normal/slow/ease` + 全局 !important 短路
- ☑ **W-06** `components/ui/README.md` API 约定一页纸
- ☑ **W-07** design_cov_guard 脚本上线（四指标基线落盘）——`scripts/design_cov_guard.mjs` + `design_cov_baseline.json`；行级豁免约定 `design-cov-exempt`
- ☑ **W-08** **[基建]** Lighthouse CI 接入（P0 十页轮换，产出基线报告）——`lighthouserc.js` + `e2e/lhci-login-token.cjs`；登录态走 extraHeaders（两个失败方案的教训记录在文件头）；**十页 LCP/CLS/TBT 基线已建，全超预算**（见 §3）
- ☑ **W-09** **[基建]** Playwright 视觉回归基建（明暗双主题截图对比管线，先人工审）——`e2e/visual-regression.spec.ts`（reducedMotion + mask + maxDiffPixelRatio 0.01）；基线待 web 重建后生成
- ☑ **W-10** 试点：消息发送/设置保存两页换协议反馈（含 5003 冲突 toast 路径）——message-center 全链路（send/bulk/upsertBox）+ settings 保存/导入/导出/包/适配器域（SettingsToast 孤岛整体收编，W-34 提前）；消息列表三态手写段落换 StatePanel
- ☑ **W-11** IA-5 页脚版本号（/api/v1/about → footer 版权条）——footer.tsx，失败静默省略
- ☐ **W-12** **[FRAME F1]** route_manifest.mjs 路由清单生成器（725 条防漂移门禁）——FRAME 线，未动
- ☐ **W-13** **[FRAME F1]** 错误码注册表（errors.rs 导出文档+TS 常量生成，drift 全量对拍）——FRAME 线，未动
- **阶段一 DoD 复核**：W-01..11 完成；W-08 基线产出（超预算，转阶段三/四清偿）；W-09 管线就绪（基线待重建）；两试点页走查过；tone×size 矩阵随原语落地（评审点：无第三主题前以文档评审代替）

### 阶段二：表单与确认协议（DS §7）

- ☑ **W-14** FormField + FieldErrors（与后端 fields map 结构化对齐）——`components/ui/form-field.tsx`；fieldErrorsFrom(ApiError) 直出 map
- ☑ **W-15** SubmitButton（生命周期封装+防双击）——`components/ui/submit-button.tsx`
- ☑ **W-16** ConfirmDialog（焦点陷阱+Esc+危险二次输入）——`components/ui/confirm-dialog.tsx`；复用 use-focus-trap；danger 不响应遮罩误触
- ☑ **W-17** 乐观更新变体（收藏/点赞即时反馈+失败回滚）——toast.optimistic（run 失败翻转错误态 + requestId）
- ☐ **W-18** 试点三表单：注册/发种/商店购买全错误路径走查——待 web 重建后人工走查
- ☐ **W-19** ConfirmDialog 落地两处：删除种子/撤销邀请——待迁

### 阶段三：P0 十页迁移 + loading 铺设（DS §7 + IA-1/2/4）

P0 十页清单（固定，勿增删）：
- ☐ **W-20** /torrents（列表页：StatePanel+已选筛选计数徽标+一键收起）
- ☐ **W-21** /torrent/[id]（详情页四态）
- ☐ **W-22** /upload（发种页：表单协议回填+错误路径）
- ☐ **W-23** /messages（收件箱：SSE 联动+四态）
- ☐ **W-24** /forums + /forums/[id]（列表+主题两页一组）
- ☐ **W-25** 搜索结果页（全局搜索对话框落地页）
- ☐ **W-26** /me（用户中心概览）
- ☐ **W-27** /top（排行榜）
- ☐ **W-28** /notifications（通知中心）
- ☐ **W-29** loading.tsx ×10（Skeleton 四预设组合，≤30 行/文件）+「正在获取」i18n 术语族
- ☐ **W-30** IA-1 导航重组（发现组拆两层、排行并组、搜索直显框）
- ☐ **W-31** IA-2 魔力 chip（后台开关）+ 消费页余额直显
- **DoD**：路由加载覆盖 ≥60%；十页 axe 零 critical；Lighthouse 移动档 LCP≤2.5s 基线达成

### 阶段四：P1 页迁移 + 代码分割（DS §7）

- ☐ **W-32** P1 约 20 页四态化（bank/games/gacha/medal-wall/offers/requests/subtitles/collections/friends/invites/myhr/tasks/…按此序）
- ☐ **W-33** next/dynamic 分割四件：游戏画布/BBCode 渲染器/管理台重表格/markdown-lite（各带 loading 骨架）
- ☐ **W-34** 两孤岛反馈收编（admin-user-detail/game-kit-feedback → toast 协议）
- ☐ **W-35** 首屏 JS 基线报告（build 产物前后对比表，目标 -15%）

### 阶段五：令牌与 a11y 清偿（DS §7 + IA-3）

- ☐ **W-36** 非豁免 hex 50 处清偿（v4 修正后目标；games 82 处进白名单）
- ☐ **W-37** 46 处裸像素断点收敛
- ☐ **W-38** `<main>` 6→全站、onKeyDown 配对补齐（4 文件基数）
- ☐ **W-39** `/` 快捷键聚焦搜索 + 键盘走查 P0 十页
- ☐ **W-40** 暗色对比度单独校准（WCAG AA，暗色独立过尺）
- ☐ **W-41** game-kit 三件套抽离（按钮/面板/分数板走令牌）
- ☐ **W-42** IA-3 主题注册表扩展（Theme 类型注册表制+「跟随系统」档）

### 阶段六：护城与移交（DS §7）+ FRAME F2-F5

- ☐ **W-43** design_cov_guard 进 CI（四指标只许向好）
- ☐ **W-44** ui_design_guard 扩 L4→L1 禁令（页面直用 hex/var(--) 即挂）
- ☐ **W-45** `docs/design-system.md`（原语目录/API 速查/新增页面 checklist）
- ☐ **W-46** CONTRIBUTING 前端节改写（宪章+六步流并入）
- ☐ **W-47** 门禁红测（故意提交 hex 新页面被拦的演示记录）
- ☐ **W-48** **[FRAME F2]** 分层宪法门禁三件 + review_decide/hr_pardon 用例抽离示范
- ☐ **W-49** **[FRAME F3]** compat 层 utoipa 化（手写 spec.rs 22 条归零）
- ☐ **W-50** **[FRAME F4 起步]** 开放 API 域 utoipa 化（tokens/download_keys/rss_tokens ~15 条）
- **达标判定日**：§9 验收清单 12 条 + FRAME §7 十条逐项过堂

---

## 3. 推进仪表盘（每阶段结束复算，数字只许向好）

| 指标 | 基线（2026-10-13） | 阶段三目标 | 终态目标 | 最近值（阶段/日期） |
|---|---|---|---|---|
| 组件型原语数 | 3 | ≥8 | ≥12 | 11（2026-10-13 阶段一/二：toast/action-feedback/busylabel/state-panel/error-panel/skip-link/form-field/field-errors/submit-button/confirm-dialog + 既有3） |
| loading.tsx 覆盖（数据路由） | 0/约40 | ≥60% | ≥90% | 11%（2026-10-13；design_cov_guard 数据路由口径 8/74——基线列 0 为门禁落盘前手数，门禁基线文件见 `scripts/design_cov_baseline.json`） |
| 非豁免硬编码 hex | 50 | ≤40 | 0 | 46（2026-10-13；design_cov_guard 实测 46，基线列 50 是「games 豁免」口径重估前的手数；`node scripts/design_cov_guard.mjs` 只认 46，差异 4 为平台字面量转 `design-cov-exempt` 行级豁免） |
| 裸像素断点 | 46 | ≤46 | 0 | 46（2026-10-13 未动） |
| 原语采用率（tsx import ui/） | 1.4% | 只升不降 | — | 3.9%（2026-10-13；21/533，design_cov_guard 指标 2） |
| window.confirm/alert | 57 | 触碰即迁 | 新增=0 | 58（2026-10-13 实测；含 prompt） |
| 路由契约覆盖 | 27/725 (3.7%) | — | ≥80%（FRAME 线） | — |
| 首屏 JS | 待 W-08 建基线 | 基线 | -15% | — |
| LCP P0 十页 | 待建 | ≤2.5s | 全绿 | **基线已建（2026-10-13 登录态）**：forums 6870 / forums-topic 6926 / messages 6593 / notifications 4740 / top 7995 / torrent-detail 7545 / torrents 7245 / search 7245 / upload 6840 / users-1 ≈6500ms——全数超预算，阶段三/四清偿 |
| CLS P0 十页 | 待建 | ≤0.1 | 全绿 | **基线已建**：0.27–0.34（内容页）/ 0.9（messages）——同超预算 |
| TBT P0 十页 | 待建 | ≤300ms | 全绿 | **基线已建**：0–335ms（登录重定向页 182–335；内容页 0） |
| axe critical（P0） | 未测 | 0 | 0 | — |

> 复算命令见 DS-PLAN §14.2；仪表盘更新与 WBS 勾选同批提交。
> design_cov_guard 四指标（W-07 基线已归档 `scripts/design_cov_baseline.json`，只许向好）：
> `node scripts/design_cov_guard.mjs`（比对基线）/ `--update` 落盘。
> LCP/CLS/TBT 复跑：`cd apps/web && LHC_LOGIN=1 LHC_USER=root LHC_PASS=*** pnpm lhci`
> （extraHeaders 登录态方案，2026-10-13 实证；报告落 apps/.lighthouseci/，为 LHR JSON）。

---

## 4. 页面-阶段映射表（防漏防重：每页只属于一个迁移批次）

| 批次 | 页面 | 数量 |
|---|---|---|
| P0（阶段三 W-20..29） | torrents、torrent/[id]、upload、messages、forums×2、搜索、me、top、notifications | 10 |
| P1（阶段四 W-32） | bank、games、gacha、medal-wall、medals、avatar-frames、dressup、offers、requests、subtitles、collections、friends、invites、myhr、tasks、contests、farm、gomoku、magic-pool、my-spark | 20 |
| P2（触碰即迁，不立项） | admin 全族、about、faq、help、classes、clients、links、users、artists、networks、seasonal、subtitle-groups、teams、textbooks、endangered、preserve、resurrections、reports、jixiao、cheaterbox、contactstaff、staffbox、donate、wiki、getrss、api-docs、appeals、ban-log、rules、my、p/* | 余量 |

> P2 纪律：任何 P2 页因功能改动被触碰时，四态与原语迁移**随改顺带完成**，PR 引用 A3 锚点；仪表盘的"触碰即迁"记录在本表加注日期。

---

## 5. 坐标定位法（动手前 30 秒自检）

```
我要改的页面/模块 → §4 属哪批？
  ├─ P0/P1 未到期 → 只修 bug 不做迁移（防提前铺开）
  ├─ 本批 → 找对应 W-xx，PR 引用锚点 A-x
  └─ P2 被触碰 → 顺带迁移 + 本表注日期
我加的组件 → components/ui/ 有吗？
  ├─ 有 → 用它
  └─ 无 → 值得原语化吗（≥3 处复用前景）？是→先立原语工单；否→页面内实现但禁违反 A2
我写的路由/错误码 → 清单/注册表登记了吗？（A6）
```

---

## 6. 周节奏（轻量，防流程变负担）

- 每完成 1-2 个 W 包：更新 §2 勾选 + §3 仪表盘该行 + §4 若有触碰记录；
- 每周五：仪表盘快照一行进 §3 历史（或 metrics-log.md），对照 A1-A8 抽一个 PR 复核锚点；
- 阶段边界：DoD 逐条过堂，未达不进下阶段（阶段一 tone 矩阵评审是硬门）。

---

## 7. 偏离处理（什么时候改策划案、什么时候改代码）

| 情形 | 处置 |
|---|---|
| 新需求落在 P2 页且量大（如 admin 大改版） | 先在 §4 该页标注"提前迁移"与日期，按 P0 标准执行——不改策划案 |
| 发现原语 API 设计缺陷 | 阶段内小改直接改 + README 同步；破坏性改（改 props 语义）→ 停下，先改 DS-PLAN §3 与 README，列全调用方同批迁移 |
| 锚点 A1-A8 与现实冲突（如某场景协议反馈确实不适用） | 在 DS-PLAN §0.2 对应意图下记"例外条款"，注明场景与期限——**不允许静默绕过** |
| 门禁拦下但确需紧急上线 | 走 baseline 登记制（同 line_limit 先例）：登记文件+issue+清偿期限，仪表盘记债 |

---

## 8. 与上游文档的关系

```
DS-PLAN-01（为什么/做什么/验收标准）
FRAME-PLAN-01（契约/分层/用例治理）
        │
        ▼
DS-MASTER-01（本框架：WBS + 仪表盘 + 页面映射 + 防偏航规则）← 日常唯一操作面
        │
        ▼
W-xx 工作包 → PR（引用锚点）→ 门禁 → 勾选回填
```
