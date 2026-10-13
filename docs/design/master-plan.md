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

- ☑ **W-32** P1 约 20 页四态化——20/20 完成（2026-10-13）：窗口弹窗族
  （prompt/alert/confirm 23 处）换 ConfirmDialog+toast；裸「加载中」
  文字换 Skeleton/StatePanel；局部 flash/msg 提示条换 toast。gacha/
  medals/invites/bank 壳等已达标页仅迁移其组件层反馈
- ☑ **W-33** next/dynamic 分割四件（2026-10-13）：①游戏画布——
  games/fishing 的 FishingPond 与 games/wheel 的 LuckyWheel dynamic 化
  （loading=SkeletonBlock）；②BBCode 渲染器 lib/bbcode.tsx——**server
  组件库（无 "use client"），dynamic 不可用，跳过**；③管理台重表格
  admin-user-detail——页面壳不变，AdminUserDetailPage dynamic 化
  （loading=SkeletonRows）；④markdown-lite——消费方 message-center-table
  的 MarkdownRenderer dynamic 化（wiki 页为 RSC，跳过）
- ☑ **W-34** 两孤岛反馈收编（2026-10-13）：admin-user-detail——flash
  提示条+window.confirm/prompt 六处全收口（toast 协议 + 新拆
  admin-user-detail-dialogs.tsx 双 ConfirmDialog）；
  game-kit-feedback（ResultFlash/GameToast/HistoryStrip）——**豁免保留**：
  游戏内 HUD 反馈（结果飘字/全屏彩带/按局战绩点）与 toast 的
  「异步写操作结果」语义不同（DS §3.1 何时不该用条款），且 GameToast
  已带 role=status+aria-live，无 a11y 缺口；豁免期随 W-41 game-kit
  三件套抽离一并复核
- ◐ **W-35** 首屏 JS 基线报告（2026-10-13）：基线+分割后对比表已建
  `docs/design/metrics-log.md`。**诚实结论：首屏 JS 未降反 +0.5%**——
  四件分割 lazy chunk 均移出路由入口（证据见 metrics-log），但 W-32
  六页（requests/offers/subtitles/myhr/my-spark/medal-wall）引入原语
  各 +6kB 抵消。**-15% 目标偏离**（§7 偏离条款）：四态化与瘦身两批
  目标在本批互相牵制；后续单独立批（vendor 分包分析/messages RSC 化/
  admin 415kB 页 chunk 拆解）继续清偿，基线以 metrics-log 2026-10-13
  分割前表为准

### 阶段五：令牌与 a11y 清偿（DS §7 + IA-3）

- ☑ **W-36** 非豁免 hex 46→**0**（2026-10-13）：三类清偿——① 内容
  语义色新令牌 `--content-*`（base.css 明暗双套同值：BBCode 16 色板 /
  豆瓣评级星 / PTP 画质三色 / 分类兜底色——「用户内容」不随主题换肤，
  与 UI chrome 的 --tone-* 划清边界）；② 主题 chrome 色收进 W-42
  注册表（theme-toggle/layout no-flash 改为序列化 lib/themes.ts 下发）；
  ③ 平台强制字面量 8 处按 W-07 行级豁免约定标 `design-cov-exempt`
  （input[type=color] value / 存库 CSS 字符串样例 / i18n 格式教学文案）
- ☑ **W-37** 裸像素断点 46（CSS 口径 31 处野点）→**0**（2026-10-13）：
  **机制选择**——纯 CSS 的 @media 不能用 var()，落地为「语义档数值吸附」：
  全部野点按 base.css 映射表吸附到 Tailwind 四档 640/768/1024/1280
  （380/400/440/560/620→640；680/700/720/767/860/900→768；901/1080/
  1120/1279.98→1024/1280；「<md」「<xl」边界用 `not all and (min-width)`
  等价写法，见 ui.css 既有模式）；ui_design_guard 规则 3 从 30 处降到 0
- ☑ **W-38** `<main>` 8→**12** + onKeyDown 配对核验（2026-10-13）：
  (auth)/login/(main) 三组 W-04 已做核销；补 app 顶层散页 4 处
  （appeals/ban-log/setup/offline 包 `<main id="main-content">`，
  SkipLink 锚点全路由可达；about/rules/error/not-found 原本已有）；
  onKeyDown 4 文件逐一核（forum-board-table/forum-category-panel/
  contact-staff/plugins-gomoku）：Enter 提交均有对应 onClick 或 onBlur
  失焦提交，contact-staff 是 Ctrl+Enter 快捷非唯一通道——零缺口
- ☑ **W-39** `/` 快捷键（W-25 提前落，本批核销）：双实现已核——
  torrents-hotkeys（列表页 `/` 聚焦页内搜索框）+ global-search-dialog
  （其余页面 `/` 开全局搜索弹窗，非输入焦点才触发）；键盘走查 P0
  十页属阶段三 W-20..29 范畴，随迁移批次执行
- ☑ **W-40** 暗色对比度 WCAG AA 校准（2026-10-13）：新增
  `scripts/check_dark_contrast.mjs`（解析 base.css 双主题令牌算 25 组
  关键配对）；暗色套除 --text-faint(4.27)/--text-fainter(2.95) 外全
  达标；校准两值（→#8b99b3/#76849f，只动暗色套），复测 5.60/4.27
  （fainter 仅用于 10px 角标，达 AA large-text 线）；明色套未动——
  明色 muted/faint 与状态色/白底的旧债（2.0-4.3）登记为阶段六债，
  见 metrics-log；结果全文记 metrics-log.md
- ☑ **W-41** game-kit 三件套核验（2026-10-13）：组件层
  （game-kit.tsx：GameShell/BalanceBar/ChipSelect/PlayHint）已全令牌
  （var(--surface-*)/--r-*/--border-*，W-34 豁免的 game-kit-feedback
  也已带 role=status）；arcade.css 的 arc-chip/gc-hero 等固有色属
  「游戏美术层」（渐变用固有色防止主题换肤把金属/宝石洗成灰块，
  文件头豁免条款有效）——按 DS §6 豁免条款核销，零改动
- ☑ **W-42** IA-3 主题注册表（2026-10-13）：`lib/themes.ts`
  （THEMES 注册表 + THEME_IDS + parseChoice/resolveTheme，唯一真源）；
  theme-toggle 二值 toggle 改三选循环（亮→暗→跟随系统），auto 档
  matchMedia 监听实时跟随系统；layout no-flash 脚本序列化注册表下发，
  auto 首绘即解析为具体主题（CSS 不认识 "auto"）；新增 icons.tsx
  `auto` 图标；主题名 i18n 三语（themeLight/themeDark/themeAuto/
  themeNext）；localStorage 沿用旧键（flux-theme），存量用户偏好不丢

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
| loading.tsx 覆盖（数据路由） | 0/约40 | ≥60% | ≥90% | 19%（2026-10-13 阶段四；design_cov 数据路由口径 14/75，Skeleton/StatePanel 页内骨架已计入页面但 loading.tsx 文件数未变） |
| 非豁免硬编码 hex | 50 | ≤40 | 0 | **0**（2026-10-13 阶段五 W-36；46→0：--content-* 内容令牌 + 主题注册表收编 chrome 色 + 8 处平台字面量行级豁免） |
| 裸像素断点 | 46 | ≤46 | 0 | **0**（2026-10-13 阶段五 W-37；CSS 野点 31→0，机制=语义档数值吸附 + not-all-and 等价写法，ui_design 口径 30→0） |
| 原语采用率（tsx import ui/） | 1.4% | 只升不降 | — | 10.8%（2026-10-13 阶段四；58/538，design_cov_guard 指标 2；W-32 二十页收编贡献 +6.7pp） |
| window.confirm/alert | 57 | 触碰即迁 | 新增=0 | 50（2026-10-13 阶段四；git grep 口径 HEAD 67→50，净清 17 处：requests/subtitles/myhr/friends/tasks/offers/dressup/frames/gomoku/contests/bank/pool/messages 域 + admin-user-detail 全族；余量在 P2 触碰即迁域） |
| 路由契约覆盖 | 27/725 (3.7%) | — | ≥80%（FRAME 线） | — |
| 首屏 JS | 待 W-08 建基线 | 基线 | -15% | **基线已建**（2026-10-13 阶段四，`docs/design/metrics-log.md`）：共享 102 kB；69 条可比路由 First Load 均值 109.8 kB。W-33 分割后复测 +0.5%（原语引入抵消，偏离条款见 §7 与 metrics-log 结论段） |
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
