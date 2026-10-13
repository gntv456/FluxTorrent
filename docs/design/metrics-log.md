# 设计系统指标日志（DS-MASTER-01 §6 周节奏落盘）

> 格式：日期 / 指标 / 值 / 来源命令。本文件是 W-35 首屏 JS 基线与
> 后续一切「-15%」对照的唯一数据源；每批构建前后各记一段。

## 2026-10-13 · W-35 基线（阶段四分割前）

| 日期 | 指标 | 值 | 来源命令 |
|---|---|---|---|
| 2026-10-13 | build 成功（分割前） | exit 0，135 路由 | `pnpm -C apps/web exec next build` |
| 2026-10-13 | 共享 First Load JS | 102 kB | 同上（`+ First Load JS shared by all`） |
| 2026-10-13 | 路由总 JS（manifest 求和，含重复计入共享块） | 49,435 kB | `node -e`（app-build-manifest.json 逐路由 stat 求和） |
| 2026-10-13 | design_cov 四指标 | 加载态覆盖 11%→**19%**（14/75）· 原语采用率 4.1%→**10.8%**（58/538）· hex 46（持平）· `<main>`×8 | `node scripts/design_cov_guard.mjs` |
| 2026-10-13 | ui_design_guard | PASS（hardcoded 1180→1180 ratchet） | `node scripts/ui_design_guard.mjs` |

### 分割前路由 First Load JS top10（build 路由表）

| 路由 | Route JS | First Load |
|---|---|---|
| /torrent/[id] | 18.4 kB | 135 kB |
| /my | 15.7 kB | 128 kB |
| /messages | 8.77 kB | 118 kB |
| /games/jgg | 4.81 kB | 118 kB |
| /games/wheel | 4.79 kB | 118 kB |
| /setup | 9.11 kB | 112 kB |
| /invites | 5.55 kB | 114 kB |
| /shop | 6.6 kB | 116 kB |
| /staffbox | 6.1 kB | 109 kB |
| /subtitles | 5.15 kB | 113 kB |

### P0 十页 First Load 明细（分割前）

| 路由 | First Load |
|---|---|
| /torrents | 124 kB |
| /torrent/[id] | 135 kB |
| /upload | 119 kB |
| /messages | 118 kB |
| /forums | 110 kB（[id] 111 kB / topic 120 kB 表内） |
| 搜索（头部对话框，随布局） | —（无独立路由） |
| /me | 103 kB（壳；内容在 /my 128 kB） |
| /top | 110 kB |
| /notifications | 113 kB |
| /wiki/[[...slug]] | 109 kB |

## 2026-10-13 · W-33 分割后复测

| 日期 | 指标 | 值 | 来源命令 |
|---|---|---|---|
| 2026-10-13 | build 成功（分割后） | exit 0 | `pnpm -C apps/web exec next build` |
| 2026-10-13 | 共享 First Load JS | 102 kB（不变） | 同上 |
| 2026-10-13 | 路由总 JS（同口径） | 49,680 kB（+0.5%；口径含跨路由重复计入的共享块，非首屏口径） | manifest 求和 |
| 2026-10-13 | 69 条可比路由 First Load 均值 | 109.8 → 110.4 kB（+0.5%） | build 路由表前后对比 |

### 分割件 chunk 证据（lazy chunk 是否真的移出路由入口）

| 分割件 | lazy chunk | 入口 manifest | 结论 |
|---|---|---|---|
| FishingPond（games/fishing） | `chunks/3569.*.js` 5.6 kB（含 FishingPond 标记）+ 页内引用 | **不在** fishing 路由 entry（异步加载） | 生效 |
| LuckyWheel（games/wheel） | `chunks/8831.*.js` 3.3 kB | 不在 wheel 路由 entry | 生效 |
| AdminUserDetailPage（admin/users/[id]） | 页 chunk 44.1 kB 含标记；路由 entry 12.1 kB | 页 chunk 即路由 chunk（Next 将 client 页面整体视为入口） | **半生效**：首次进入该路由仍拉全量（见下「结论与偏离」） |
| MarkdownRenderer（message-center-table） | `chunks/5461.*.js` 6.4 kB（含标记） | 不在 messages 路由 entry | 生效 |

### First Load JS 前后（目标路由）

| 路由 | 前 | 后 | Δ |
|---|---|---|---|
| /messages | 118 kB | 117 kB | -1 kB |
| /games/fishing | —（前表被 tail 截断） | 119 kB | — |
| /games/wheel | 118 kB | 118 kB | 0 |
| /admin/users/[id] | —（前表未含） | 124 kB | — |

### 结论与偏离（诚实记录）

1. **首屏 JS 未降反微升（+0.5%）**：四件分割的 lazy chunk 确实移出了
   fishing/wheel/messages 的路由入口，但 W-32 二十页四态化把
   ConfirmDialog/toast/StatePanel/Skeleton 原语带进了 requests/offers/
   subtitles/myhr/my-spark/medal-wall 六页（各 +6 kB）——原语采用的
   代价。「-15%」目标在 W-33 单批内不可达，已按偏离处理规则记入
   master-plan §3 仪表盘与 §7（偏离条款）。
2. admin/users/[id] 的 dynamic 因目标本身是 client 页面组件，Next 15
   将其 page chunk 计入路由入口，**首屏无收益**（收益仅在：后续若把
   页面壳改 RSC、或该组件被多路由共享时成立）。保留分割（不回退），
   理由：lazy 边界已建立，W-36+ 令牌清偿时无需再动。
3. `-15%` DoD 对照基线 = 本文件第一段（2026-10-13 分割前表）。后续
   压缩（vendor 分包分析、messages RSC 化等）以此为 0 点。

## 2026-10-13 · 阶段五 W-36..42 清偿批（令牌与 a11y）

| 日期 | 指标 | 前 → 后 | 来源命令 |
|---|---|---|---|
| 2026-10-13 | design_cov 非豁免 hex | 46 → **0** | `node scripts/design_cov_guard.mjs` |
| 2026-10-13 | ui_design 野断点（CSS 口径） | 30 → **0** | `node scripts/ui_design_guard.mjs` |
| 2026-10-13 | ui_design 硬编码色值（CSS 棘轮） | 1180 → 1206 | 同上（+26 = base.css 新增 --content-* 21 个内容令牌双套声明，属令牌层不是野色；baseline 已按约定 `--update` 刷新） |
| 2026-10-13 | a11y 骨架 `<main>` | ×8 → **×12** | design_cov_guard（appeals/ban-log/setup/offline 补 `main id="main-content"`） |
| 2026-10-13 | tsc / eslint（改动文件）/ vitest / next build | 全 0 错 / 117 测试全过 / exit 0 | `pnpm -C apps/web exec tsc --noEmit` 等 |
| 2026-10-13 | i18n / line_limit / ui_design / design_cov 四门禁 | 全 PASS | 各 guard 脚本 |

### W-36 hex 46→0 清偿路径

- **内容语义令牌**（base.css `--content-*`，明暗双套同值——「用户内容」
  不随主题换肤）：BBCode 16 色板（--content-bb-*，upload-form-descr 17
  处→引用令牌，[color=] 参数写 var() 由 bbcode 渲染器 style 直接吃）、
  豆瓣评级星 --content-rating-star（latest-posters）、PTP 画质三色
  --content-tier-uhd/hd/sd（torrent-detail-blocks）、分类兜底色
  --content-cat-fallback（site-profile CAT_FALLBACK_COLOR 改 var 引用）。
- **主题注册表收编**（随 W-42）：theme-toggle THEME_CHROME 与 layout
  no-flash 内联色表统一改序列化 `lib/themes.ts` THEMES 注册表下发。
- **平台字面量豁免 8 处**（W-07 行级豁免约定，全部带原因注释）：
  input[type=color] value 兜底 ×1、取色/标签/头像框输入框 placeholder
  格式样例 ×3、标签字典存库白字 ×1、头像框 CSS 初始样例 ×1、i18n 三语
  cssHint 教学文案 ×3（同一键三语）。admin-tagdict EMPTY.bg_color 改
  `var(--sky)`（新建标签默认底是 UI 缺省值，可令牌化）。

### W-37 断点机制选择与吸附表

纯 CSS `@media` 不能用 `var()`（spec 限制），仓内亦无 SCSS mixin 层；
落地机制 = **语义档数值吸附**：全部野点按 base.css 映射表吸附到
Tailwind 四档字面量（640/768/1024/1280），「低于某档」边界用
`not all and (min-width: Xpx)` 等价写法（ui.css 既有模式，行为零差）。

| 野值 | 吸附至 | 涉及（处） |
|---|---|---|
| 380/400/440/560/620 | 640 | base.css×3、layout.css×1、pages.css×1、arcade.css×1、arcade-sweet×2 |
| 680/700/720/860/900 | 768 | base.css×1、layout.css×2、pages.css×2、arcade-sweet×2 |
| 767（<md 惯用写法） | not-all-and(768) | layout.css×3、pages.css×3 |
| 901/1080/1120 | 1024 | layout.css×3、arcade-sweet×1 |
| 1279.98（<xl 防重叠） | not-all-and(1280) | pages.css×1 |
| 1201/1600→1201 | 1280 | arcade-sweet×2（1600 超宽档并入 1280，五列格在 1280+ 生效） |

### W-40 暗色对比度校准（WCAG AA）

脚本 `scripts/check_dark_contrast.mjs`（node，解析 base.css 双主题
令牌块，resolve var() 链后算 25 组关键配对的相对亮度对比度）。

- **暗色套（校准后全过线）**：正文 10.5-11.9、标题 13.8-15.0、次要
  6.3-7.1、品牌 8.2-9.3、状态徽章（success/warn/danger × soft 底/卡片）
  7.3-11.3、表格正文 8.9、禁用态 5.3。调值两处：--text-faint
  #76849f→**#8b99b3**（4.27→5.60）、--text-fainter #5c6a84→**#76849f**
  （2.95→4.27，该档仅论坛楼层 10px 辅助角标，达 AA large-text ≥3 线
  且留裕量）。只动暗色套。
- **明色套（本批不动，登记阶段六债）**：muted/卡片 4.31、faint/卡片
  3.11、fainter 2.17、状态色/白底 1.9-2.8、brand 3.6——亮色徽章沿袭
  浅底亮色画法，正文/标题类全过（7.7-16.8）。债主：--success/--warning/
  --danger 亮色值与白底组合，需成组调深（影响 soft 底配套色），留待
  W-43 门禁收紧时一并处理，避免本批暗色校准与亮色重调互相牵动。

### W-41/W-42 核验与实现要点

- W-41：game-kit 组件层零 hex（BalanceBar/GameShell/ChipSelect/PlayHint
  全 var()），arcade.css 固有色属美术层豁免条款（文件头有案）——核销。
- W-42：`lib/themes.ts` THEMES 注册表（chrome 平台字面量集中+行级
  豁免注释）；toggle 三选循环 + auto matchMedia 跟随；no-flash 脚本
  由注册表序列化生成（CH/IDS），auto 首绘即解析为具体主题；i18n
  themeLight/themeDark/themeAuto/themeNext 三语新增；localStorage 键
  不变（flux-theme），"auto" 为新增合法值，旧值 brand/brand-night
  向后兼容。

