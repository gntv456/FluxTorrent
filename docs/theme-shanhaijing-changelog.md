# 山海经「岩彩长卷」主题全量落地 · 变更摘要

> 状态：**改动仍在工作树、未提交**（本摘要基于 2026-11-03 `git status` / `git diff --stat` 取证）。
> 工作树里同时存在本轮之外的其它在制工作（upload-form / message-center / cheat / invite / hxpt-admin-design / vibe_images / Cargo.lock 等），不在本文范围内。
> 图标线（分类位图图标）由并行会话开发，属同一轮交付，清单中以〔图标线〕标注归属；其余为〔主题线〕。

## 0. 一句话结论

四套主题体系（brand / brand-night / tide 换肤层 / shanhaijing）就位：山海经主题「甲 · 令牌层 + 乙 · 结构层」**全量落地**，配套两套分类位图图标（aurora / 山海经）与自托管字体，六条门禁全绿，三套既有主题逐像素零回归。

交付范围：主题注册表第四档 `shanhaijing`（zh 山海经 / zh-TW 山海經 / en Mythica）+ 约 140 个令牌槽位 + 图鉴侧栏三态 / Hero 山海图卷 / 竹简表 / 印章角标 / 游戏大厅岩彩化 + 2×96 枚分类图标 PNG + 184 个字体分片。

## 1. 令牌层（新增与重命名）

| 项 | 内容 |
| :--- | :--- |
| 主题注册 | `apps/web/lib/themes.ts` 追加 `shanhaijing`（scheme: dark，chrome `#0a0f0d`；排在 brand-night 后，auto 深色档行为不变） |
| 令牌文件 | 新建 `apps/web/app/styles/theme-shanhaijing.css`（约 140 槽位，全部 `[data-theme="shanhaijing"]` 作用域），挂到 `globals.css` 链末 |
| **兼容层重命名** | `--brand-*` → `--compat-*` 全库机械替换：**70 个文件、583 处引用**；旧昵称字样改前 1 处（设计文档）→ 改后 **全库 0 命中**；定义/引用交叉核对 15 键无悬空 |
| `--focus-ring` 归口 | 金环 `#d3a13f` + 软晕（沿用 base.css P2-6 归口槽位，实测 43 组场景 outlineColor 全对） |
| `--mood-*` 画布色 | 44 个（钓场/转盘/扭蛋 SVG 场景）暗底岩彩等效色 |
| `--fhue-*` / `--fbadge-*` | 论坛色板 6 + 徽章 12 个暗底等效色（fg 实测 5.2–12.5:1） |
| 偏差声明 | `--sky-deep`/`--danger`/`--coral`/`--indigo` 四处按暗底 AA 取亮档（沿 brand-night W-40 校准惯例，文件头注有对照表） |

## 2. 结构层（新增组件与关键文件）

| 文件 | 职责（一句话） | 归属 |
| :--- | :--- | :--- |
| `apps/web/components/shj-atlas.tsx` | 「山海图鉴」竖排侧栏：≥1180 展开 / 768–1179 图标轨（分组浮层）/ <768 交还抽屉+TabBar；与顶栏同源 6 组 37 入口 | 主题线 |
| `apps/web/components/shj-hero.tsx` | 首页 Hero「山海图卷」：金箔题字 + 竖排卷签 + 4 块站点数据玉牌 | 主题线 |
| `apps/web/components/cat-icon.tsx` | 多色位图分类图标组件（解析链：categoryKey → iconKey → cat-default → cat-other → 首字色块；MutationObserver 跟随换主题换套） | 图标线 |
| `apps/web/lib/cat-icon-sets.ts` | 图标集注册表与文件解析（labelKey/noteKey 走 i18n 字典） | 图标线 |
| `apps/web/components/theme-toggle.tsx` | 追加悬停/聚焦预取「下一档」图标集当前页可见图标（不无条件预取两套） | 主题线 |
| `apps/web/components/game/game-stage.tsx` | 新增 `shjBg` 属性（内联 `--gs-shj-bg` 令牌引用，仅山海经消费） | 主题线 |
| `apps/web/app/(main)/layout.tsx` | 挂载 `<ShjAtlas>`（恒渲染、CSS 控显隐，数据与 Header 同源零增请求） | 主题线 |
| `apps/web/app/(main)/page.tsx` | 挂载 `<ShjHero>`（站点数据与「站点数据」区块同源） | 主题线 |
| `theme-tide.css` 3 处 | `.btn-primary` / `.btn-danger:hover` / `.tide-pager__btn[aria-current]` 的 `#fff` → `var(--text-on-accent)`（老主题该令牌即 #fff，零变化） | 主题线 |
| `arcade-sweet.css` 末段 | 山海经块：`--sweet-*` 重映射 + 大厅画布（gc-hero/arc 奖池/gs-stage-box/gtabs 等）岩彩化，浅色扫描 8 页清零 | 主题线 |
| 分类 chips | `chipLabel` 接 `CatIcon`（22px，实测内高 24px 不破行高）+ `:has(:checked)` 描边环；`promoCls` 扩全 kind 类名 | 主题线 |
| 分类色岩彩化 | `shjCatHue` 散列 → `data-shj-hue` 属性 + 作用域 CSS 重映射（实心块深档 / 行卡 tint 亮档双色板） | 主题线 |

## 3. 图标与资产

| 项 | 内容 | 归属 |
| :--- | :--- | :--- |
| 图标集落位 | `apps/web/public/cat-icons/aurora/`（96 PNG，≈1.9 MB）与 `.../shanhaijing/`（96 PNG，≈5.4 MB） | 图标线 |
| PNG 档位 | 每枚 64/128/256 三档，按显示尺寸取用；含 `-active` 选中态变体（aurora 加深反白 / shanhaijing 金箔描边） | 图标线 |
| 源稿 | `design/icons/`（v3 山海经 / v4 aurora 设计源） | 图标线 |
| 字体自托管 | `scripts/shanhaijing-fonts.py` 生成 `app/styles/fonts-shanhaijing.css`；`public/fonts/` 下 mashanzheng 92 片（3.4 MB）+ zcoolxiaowei 92 片（3.12 MB），unicode-range 分片按需加载，零外部 CDN | 主题线 |
| 历史舞台图 | `stage-bigsmall/farm/scratch.jpg` 全库零引用 → 归档 `design/_archive/unused-stage/`（不删除）；`stage-jgg.jpg` 仍在用 | 主题线 |
| 站点背景 | 山海经下海滩照片由作用域 CSS 压掉，换零资产的星峦纹（`.site-backdrop` `!important` 覆盖内联样式） | 主题线 |

## 4. 门禁与文档

- 测试：`tests/theme-registry.test.ts`（新，注册表门禁）〔图标线/共用〕；`tests/category-icon-dedup.test.tsx`（改）〔图标线〕。
- 门禁口径：`design_guard` 仅对本线触碰的 tsx 做 `--touch` 定向行号重算（守卫注释明示的正规流程），未用 `--update`、未新增 `design-cov-exempt`；`design_cov` 非豁免 hex 维持 13（只降不升）。
- 六条门禁最终态：`vitest run` 127 文件 547 例全过；`tsc --noEmit` 0；`eslint` 改动文件 0 错误；`i18n_guard` / `design_cov_guard` / `design_guard` 全通过。
- 文档：`design/FluxTorrent-山海经UI设计系统.html` 升 **V1.3**（落地清单 + 生产落位）；`docs/customize/packs-themes.md` 新增「内置主题四档与山海经双层架构」节（边界 + 回滚）。

## 5. 兼容性与零回归证据（三套既有主题）

QA 环境：本地全栈（Docker postgres/redis/api + Next dev），root 登录态，脚本均在 `design/_shots/` 可复跑。

| 对照项 | brand | brand-night | 证据（截图/实测） |
| :--- | :--- | :--- | :--- |
| 导航结构 | 顶栏平铺在、无侧栏（`.shj-atlas display:none`） | 同左 | `shj-p1-brand-1440.png` / `shj-p1-brand-night-1440.png` |
| 焦点环 | 蓝 `rgb(15,98,196)` | 浅蓝 `rgb(71,179,255)` | `_qa_shj_final.mjs` 43 组实测 |
| 站点背景 | 海滩照片在（`PHOTO!` 实测） | 同左 | `_qa_shj_p3.mjs` backdropImg |
| 游戏大厅画布 | 浅色甜画布原样 | 同左（gc-hero 冰蓝渐变实测） | `shj-p3-games-night.png` / `shj-p4-stage-*-brand-night.png` |
| 分类 chips | aurora 图标 + 蓝描边环 | aurora 图标 + 浅蓝描边环 | `shj-p5-chips-brand.png` / `shj-p5-chips-brand-night.png` |
| 促销徽章文字色 | 蓝/琥珀文字（soft 徽章） | 浅蓝/琥珀 | `_qa_shj_p4.mjs` seals 实测值 |
| 种子表/论坛/首页 | 五档无溢出，截断启发式与山海经同值 | 同左 | `shj-fin-{brand,brand-night}-*.png`（12 张） |
| 移动底栏/抽屉 | 375 下结构不变 | 同左 | `shj-p4-torrents-brand-375.png` 等 |

## 6. 验收结论表（成熟度 10 条）

| # | 口径 | 结论 | 证据指向 |
| :--- | :--- | :--- | :--- |
| 1 | 五档无溢出/截断/重叠 | ✅ 43 组场景 scrollWidth 全 false | `design/_shots/shj-fin-*`（43 张） |
| 2 | 对比度正文≥4.5 / 大字≥3 / 非文本≥3 | ✅ 脚本实测：正文 11.0、弱文本 4.6+、语义色 5.7–8.1、焦点环 8.1、印章 6–8.7 | 令牌文件头注 + 实测表 |
| 3 | 键盘全流程 | ✅ Tab 首站 SkipLink、金环实测、抽屉 Esc+焦点陷阱、浮层 Esc | `_qa_shj_p1.mjs` / `_qa_shj_p4.mjs` |
| 4 | 动效降级 / 无 CLS / 无闪烁 | ✅ reduced-motion 实测动画归零；no-flash 脚本含 shanhaijing chrome | `_qa_shj_p2.mjs` home-rm |
| 5 | 语义与 a11y | ✅ aria-expanded / aria-current / aria-modal 实测存在；装饰图标 aria-hidden | 同上门禁脚本 |
| 6 | 四套主题互不影响 | ✅ 见第 5 节逐项对照 | 同上 |
| 7 | 全量门禁 | ✅ 六条全绿 | 第 4 节 |
| 8 | 真实渲染 QA | ✅ 6 页 × 5 档 × 山海经 + 对照 | `_qa_shj_{p1..p5,final}.mjs` |
| 9 | 字体与资源自托管 | ✅ 184 分片本地、零外部 CDN、背景零位图 | 第 3 节 |
| 10 | 文档同步 | ✅ 设计系统 V1.3 + packs-themes.md + 本文 | 第 4 节 |

## 7. 回滚方式

**A. 只回退山海经主题（用户级）**：主题切换器换到日间/夜间即恢复；`localStorage.removeItem("flux-theme")` 回 auto。

**B. 代码级回退主题层（保结构）**：删 `globals.css` 中 `fonts-shanhaijing.css` / `theme-shanhaijing.css` 两行 @import + `themes.ts` 的 `shanhaijing` 注册项 + i18n 三语 `themeShanhaijing`/`shj*` 键，即回三套主题（结构组件因无令牌作用域自动隐身）。

**C. 回退结构层（保令牌）**：撤 `(main)/layout.tsx` 的 `<ShjAtlas>`、`page.tsx` 的 `<ShjHero>` 两处挂载 + `components/shj-atlas.tsx` / `shj-hero.tsx` 两文件 + `theme-shanhaijing.css` 末「乙 · 结构层」段 + `arcade-sweet.css` 末山海经块 + `game-stage.tsx` 的 `shjBg` 属性与四个 `_inner.tsx` 的对应传参。

**D. 全量回退**：B + C，再按需 `git checkout --` 兼容层重命名（`--compat-*` → `--brand-*`）与 `docs/customize/packs-themes.md`；图标线（cat-icon / cat-icon-sets / public/cat-icons / 两个测试）单独成组可整组摘除，`CatBlock` 的 CatIcon 调用点回退为单色 Icon 即可。

## 8. 遗留与已知取舍

- 非分类 chip（存活/审核等组）保持纯文本——刻意为之，与分类 chip 形成信息层级。
- 图标无 16px 档：V3.1 在 16px 不达标，chips 图标位实测给到 22px 后两条线统一上图标；更小场景不要降级用 16px。
- `lib/cat-icon-sets.ts` 的 `THEME_TO_CAT_ICON_SET` 对 `tide` 无显式映射（tide 不是注册主题，是 brand 换肤层），兜底走 aurora——若未来 tide 注册为主题需补一行〔图标线〕。
- 位图图标不可令牌化：PNG 自带固有色，主题切换靠「换图标集」而非换色值，故主题切换瞬间有一次性请求（已用悬停预取缓解）。
- 分类色岩彩化是「颜色字符串散列 → 四正色」的近似映射：同色系分类可能落同一岩彩，属气质统一的有意取舍，不是逐分类精确对应。
- 种子库促销/HR 样本为本地造数（`_seed_promo.mjs` / `_seed_hr.mjs`），提交前留意不要把样本带进生产库。

## 9. 验证命令清单

```powershell
# 门禁（apps/web 下前三条，仓库根下后三条）
npx vitest run
npx tsc --noEmit
npx eslint <改动文件>
node scripts/i18n_guard.mjs
node scripts/design_cov_guard.mjs
node scripts/design_guard.mjs

# 本地全栈 QA（Docker 栈 + npx next dev -p 3000 后）
node design/_shots/_seed_promo.mjs        # FREE/2X 样本（幂等，仅本地）
node design/_shots/_seed_hr.mjs           # H&R 样本（同上）
node design/_shots/_qa_shj_final.mjs      # 6 页 × 5 档全量 + 对照
node design/_shots/_qa_shj_p1.mjs         # 侧栏三态/抽屉/回归
node design/_shots/_qa_shj_p4.mjs         # 色块/舞台/印章/浮层
node design/_shots/_qa_chips_hr.mjs       # chips 图标 + HR 行
node design/_shots/_qa_prefetch.mjs       # 图标集预取
```
