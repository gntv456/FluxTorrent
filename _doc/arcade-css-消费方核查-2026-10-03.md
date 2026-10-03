# arcade CSS 消费方核查报告

**核查日期**：2026-10-03
**核查范围**：`apps/web/app/styles/arcade.css`（55KB）、`arcade-sweet.css`（68KB）
**核查方式**：只读静态分析（括号深度感知 CSS 解析 + TSX 类名 token 扫描 + import 图可达性分析）
**结论一句话**：**不能盲移**。有 **18 个类名**被 `/games` 之外的 `/gacha` 路由真实消费，盲移会导致 `/gacha` 抽卡页掉样式；裸类名**无实际重名冲突**（仅 1 例且双方都已作用域化）。

---

## 一、核心结论速览

| 问题 | 结论 |
|---|---|
| 能不能盲移到 `games/layout.tsx`？ | **不能**。必须先处理 `/gacha` 的 18 个类名 |
| 站外消费者有哪些？ | **只有 `/gacha` 一个路由**（涉及 18 个类名、3 个源文件） |
| `arcade.css` 有站外消费者吗？ | **没有**。可安全整体搬走 |
| `arcade-sweet.css` 有站外消费者吗？ | **有**。18 个类名被 `/gacha` 消费 |
| 裸类名有重名冲突吗？ | **实质无**。仅 `.font-display` 1 例，且双方均带作用域限定 |
| 全站 93 页是否真在下载这 120KB？ | 是。`globals.css` 根 `@import` 链，无条件对所有路由生效 |

---

## 二、关键前提修正：两个文件的「作用域」情况不同

这点直接决定结论，必须先说清：

| 文件 | 顶层规则 | 其中**无** `[data-arcade]` 作用域 | 说明 |
|---|---|---|---|
| `arcade.css` | 307 个选择器片段 / 297 条规则 | **307（全部）** | 完全无作用域，纯全局 |
| `arcade-sweet.css` | 314 个选择器片段 / 298 条规则 | **144 条（约 46%）** | 头部有作用域声明，但**近一半规则没写作用域** |

> **踩坑提示**：`arcade-sweet.css` 文件头注释写着「作用域：`[data-arcade="sweet"]`，仅 `/games/*` 生效」，
> 但**实际有 144 条顶层规则并未加作用域限定**（如 `.gtabs`、`.sw-cardbook*`、`.sw-synth*`、`.sw-stat*`、`.sw-cta*`、`.sw-ten*` 等）。
> 这批「漏网」规则正是 `/gacha` 今天能吃到样式的原因，也是本次搬移的唯一真风险点。

---

## 三、`arcade.css` 顶层类选择器（219 个，去重排序）

括号深度感知解析，只取顶层（排除 `@media` / `@supports` / `@keyframes` 内部嵌套）。

> 用户提供的「307 个顶层选择器」对应**逗号分隔后的片段数**；去重后的**类名**为 219 个。两个数字都对，只是口径不同。

<details>
<summary>展开完整 219 个类名</summary>

`.arc` `.arc-album` `.arc-bignum` `.arc-board` `.arc-board-col` `.arc-btn` `.arc-call` `.arc-checks` `.arc-chip` `.arc-chk` `.arc-crest` `.arc-crest-side` `.arc-gate` `.arc-goods` `.arc-hd` `.arc-lab` `.arc-link` `.arc-node` `.arc-note` `.arc-q` `.arc-qsub` `.arc-ribbon` `.arc-ring` `.arc-sec` `.arc-shelf` `.arc-stub` `.arc-track` `.arc-track-cell` `.bar` `.beam` `.bl` `.br` `.bs-cursor` `.bs-plate` `.bs-plates` `.bs-run` `.bs-ticks` `.bs-track` `.cap-base` `.cap-dome` `.cap-drop` `.cap-egg` `.cap-eggs` `.cap-knob` `.cap-machine` `.cap-mouth` `.cap-rig` `.cells` `.coral` `.crest-frame` `.ct` `.d` `.dead` `.dot` `.eating` `.exp` `.fall` `.fp-bobber` `.fp-hud` `.fp-label` `.fp-pond-svg` `.fp-window` `.fs-album` `.fs-album-n` `.fs-event` `.fs-fish` `.fs-fish-ev` `.fs-fish-face` `.fs-fish-n` `.fs-fish-name` `.fs-rod` `.fs-rod-bonus` `.fs-rod-ico` `.fs-rod-lv` `.fs-rod-up` `.gc` `.gc-art` `.gc-art-grid` `.gc-badge` `.gc-body` `.gc-card` `.gc-glyph` `.gc-go` `.gc-grid` `.gc-group-hd` `.gc-group-line` `.gc-group-n` `.gc-hero` `.gc-hero-main` `.gc-hero-orb` `.gc-hero-stat` `.gc-hero-sub` `.gc-hot` `.gc-soon` `.gc-sub` `.gc-title` `.gc-tone-candy` `.gc-tone-coral` `.gc-tone-indigo` `.gc-tone-mint` `.gc-tone-sky` `.gc-tone-sun` `.gc-tone-teal` `.gc-tone-violet` `.gn` `.go` `.got` `.gs-art` `.gs-corner` `.gs-inner` `.gs-scrim` `.gs-stage` `.gv` `.high` `.hunger` `.ic` `.is-soon` `.jack` `.jgrid` `.lamp` `.lb-head` `.lb-hero` `.lb-rank` `.lb-row` `.lb-table` `.lb-who` `.lit` `.lock` `.locked` `.low` `.lw-disc` `.lw-disc-wrap` `.lw-hub` `.lw-pointer` `.lw-rig` `.medal` `.mini` `.mono` `.mood-hungry` `.mood-max` `.nm` `.num` `.nv` `.ok` `.on` `.open` `.pg-eyebrow` `.plot-bed` `.pp-bar` `.pp-bar-lb` `.pp-bar-n` `.pp-bars` `.pp-bubble` `.pp-coupon` `.pp-custom` `.pp-custom-btn` `.pp-custom-hd` `.pp-custom-input` `.pp-custom-lb` `.pp-custom-save` `.pp-custom-wrap` `.pp-custom-x` `.pp-fill` `.pp-glow` `.pp-ground` `.pp-ground-shadow` `.pp-lv` `.pp-name` `.pp-pat` `.pp-pat-heart` `.pp-pen` `.pp-pet` `.pp-sp` `.pp-sp-line` `.pp-sp-name` `.pp-species` `.pp-species-tag` `.pp-stage` `.pp-track` `.primary` `.qty` `.r` `.r2` `.r3` `.r4` `.r5` `.rare` `.reached` `.ready` `.ring` `.ripe` `.rk` `.roll` `.run` `.sc-face` `.sc-pop` `.sc-stars` `.shake` `.sky` `.socket` `.spin` `.tiny` `.tk` `.tl` `.tname` `.top1` `.top2` `.top3` `.tpill` `.tr` `.up-pet` `.up-pet-face` `.up-pet-meta` `.val` `.warn` `.who` `.why` `.win` `.zero`

</details>

---

## 四、`arcade-sweet.css` 顶层类选择器（238 个，去重排序）

<details>
<summary>展开完整 238 个类名</summary>

`.arc` `.arc-bignum` `.arc-btn` `.arc-call` `.arc-chip` `.arc-chk` `.arc-crest` `.arc-goods` `.arc-hd` `.arc-link` `.arc-node` `.arc-q` `.arc-ribbon` `.beam` `.bg-mint` `.bg-sun` `.bg-sun-soft` `.bico` `.blabel` `.blaze` `.bs-cursor` `.bs-dice` `.bs-dice-cap` `.bs-die` `.bs-die-tag` `.bs-pick` `.bs-pick-s` `.bs-pick-t` `.bs-picks` `.bs-recent` `.bs-ticks` `.bstar` `.cap-base` `.cap-dome` `.cap-knob` `.cap-machine` `.cells` `.cta-gold` `.cta-primary` `.dead` `.decoy` `.done` `.dot` `.empty` `.exp` `.font-display` `.fp-hud` `.fp-label` `.fp-pond` `.fp-pond-svg` `.fs-fish` `.fs-rod` `.gc-art` `.gc-art-grid` `.gc-badge` `.gc-body` `.gc-card` `.gc-glyph` `.gc-go` `.gc-grid` `.gc-group-hd` `.gc-group-line` `.gc-group-n` `.gc-hero` `.gc-hero-cta` `.gc-hero-orb` `.gc-hero-stat` `.gc-hero-sub` `.gc-hot` `.gc-qi` `.gc-qi-badge` `.gc-qi-ic` `.gc-qi-lb` `.gc-quick` `.gc-soon` `.gc-sub` `.gc-title` `.gc-tone-candy` `.gc-tone-coral` `.gc-tone-indigo` `.gc-tone-mint` `.gc-tone-sky` `.gc-tone-sun` `.gc-tone-teal` `.gc-tone-violet` `.go` `.gold` `.got` `.green` `.gs-art` `.gs-back` `.gs-balance` `.gs-corner` `.gs-scrim` `.gs-stage` `.gs-stage-box` `.gs-title` `.gs-title-ic` `.gtab` `.gtab-ic` `.gtabs` `.high` `.hot` `.hunger` `.ic` `.jack` `.jgrid` `.jpk-burst` `.jpk-piece` `.lamp` `.lose` `.low` `.lw-disc` `.lw-go` `.lw-hub` `.lw-pointer` `.mx` `.num` `.on` `.on-gold` `.on-mint` `.open` `.pg-eyebrow` `.plot-bed` `.pp-act` `.pp-acts` `.pp-bar-lb` `.pp-bar-n` `.pp-coupon` `.pp-evolve` `.pp-fill` `.pp-hero` `.pp-hero-face` `.pp-hero-name` `.pp-hero-sub` `.pp-name` `.pp-pen-cap` `.pp-pen-cell` `.pp-pen-grid` `.pp-species-tag` `.pp-stage` `.primary` `.rare` `.ready` `.ripe` `.rounded-` `.sc-face` `.sc-stars` `.sky` `.streak-pill` `.sun` `.sw-album` `.sw-album-acts` `.sw-album-btn` `.sw-album-card` `.sw-album-cta` `.sw-album-empty` `.sw-album-empty-face` `.sw-album-exch` `.sw-album-face` `.sw-album-face-name` `.sw-album-face-r` `.sw-album-grid` `.sw-album-head` `.sw-album-meta` `.sw-album-panel` `.sw-cap-chits` `.sw-cap-collect` `.sw-cap-crow` `.sw-cap-ctier` `.sw-cap-meter` `.sw-cap-meters` `.sw-cap-pct` `.sw-cap-r` `.sw-cap-track` `.sw-card` `.sw-card-face` `.sw-card-name` `.sw-card-rank` `.sw-card-stars` `.sw-cardbook` `.sw-cardbook-cap` `.sw-cardbook-grid` `.sw-coat-cell` `.sw-coat-face` `.sw-coat-grid` `.sw-coat-hit` `.sw-coat-lid` `.sw-cta` `.sw-cta-row` `.sw-env-head` `.sw-env-info` `.sw-envelope` `.sw-farm-dot` `.sw-farm-stats` `.sw-farm-strip` `.sw-farm-weather` `.sw-hamt` `.sw-hist` `.sw-hist-empty` `.sw-hrow` `.sw-htime` `.sw-htxt` `.sw-pool-pill` `.sw-prize-ic` `.sw-prize-item` `.sw-prize-lb` `.sw-prize-nm` `.sw-prize-odds` `.sw-prize-row` `.sw-prize-strip` `.sw-reveal-all` `.sw-rules` `.sw-stat` `.sw-stat-lb` `.sw-stat-row` `.sw-stat-vl` `.sw-synth` `.sw-synth-cap` `.sw-synth-note` `.sw-synth-row` `.sw-synth-track` `.sw-ten` `.sw-ten-hd` `.sw-ten-overlay` `.sw-ten-row` `.sw-ten-total` `.sw-ten-x` `.sw-tier` `.sw-tier-b` `.sw-tier-s` `.sw-tiers` `.sw-tone-gold` `.sw-tone-sky` `.text-` `.tk` `.win` `.wx-right`

</details>

---

## 五、消费方核查（重点）

### 5.1 方法说明：为什么不能只看文件位置

`components/` 下的组件被**多个路由共享**。例如 `components/game/game-kit.tsx` 同时被 `/games/*` 和 `/gomoku` 引用。
所以「文件在 `components/game/` 下」**不等于**「只服务 `/games`」。

本次核查构建了完整的 **import 可达图**：

1. 枚举 `app/(main)/` 下 **47 个一级路由**；
2. 从每个路由的 `page.tsx`（缺失则 `layout.tsx`）出发，递归解析 `import` / `require`（支持 `@/` 别名与相对路径）做 BFS，得到该路由**实际能渲染到的文件集合**；
3. 扫描全部 500 个源文件，抽出 `className` / `class` 属性及 `cn()`/`clsx()`/`cva()` 调用中的类名 token；
4. 与 219 + 238 个 arcade 类名求交集，再按「该类名所在文件被哪些站外路由可达」判定归属。

### 5.2 核查结果总览

| 分类 | 类名数 | 说明 |
|---|---|---|
| 仅 `/games` 可达（安全） | **73** | 搬走无影响 |
| **站外可达且会真的掉样式** | **18** | ⚠️ 全部来自 `/gacha` |
| 站外可达但仅作用域定义（本来就不命中） | 19 | 搬走无影响 |
| 站外可达但全局 CSS 已有定义 | 4 | 搬走无影响 |
| Tailwind 片段误报 | 2 | 假阳性 |

### 5.3 ⚠️ 真实风险：18 个类名被 `/gacha` 消费

**唯一站外路由是 `/gacha`（抽卡）**。涉及 3 个源文件：

| 源文件 | 路由归属 | 说明 |
|---|---|---|
| `apps/web/app/(main)/gacha/_inner.tsx` | `/gacha` | 票根册 + 合成面板 |
| `apps/web/components/gacha-album.tsx` | `/gacha`（被 `_inner.tsx:4` 引用） | 专辑册 |
| `apps/web/components/game/game-tabs.tsx` | `/games/*` + `/gacha`（被 `components/gacha-tabbed.tsx:9` 引用，后者被 `_inner.tsx:5` 引用） | 标签栏 |

**类名 → 消费位置 → 路由 对照表**（全部 18 个均在 `arcade-sweet.css`，且**均无 `[data-arcade]` 作用域限定**）：

| 类名 | 定义位置（`arcade-sweet.css`） | 消费文件:行号 | 路由 |
|---|---|---|---|
| `.sw-cardbook` | L1659 | `app/(main)/gacha/_inner.tsx:171` | `/gacha` |
| `.sw-cardbook-cap` | L1669 | `app/(main)/gacha/_inner.tsx:172` | `/gacha` |
| `.sw-cardbook-grid` | L1676 | `app/(main)/gacha/_inner.tsx:175` | `/gacha` |
| `.sw-card` | L1682, L1695, L1698 | `app/(main)/gacha/_inner.tsx:177` | `/gacha` |
| `.sw-card-stars` | L1743 | `app/(main)/gacha/_inner.tsx:178` | `/gacha` |
| `.sw-card-face` | L1748, L1753 | `app/(main)/gacha/_inner.tsx:181` | `/gacha` |
| `.sw-card-name` | L1757 | `app/(main)/gacha/_inner.tsx:184` | `/gacha` |
| `.sw-card-rank` | L1761 | `app/(main)/gacha/_inner.tsx:185` | `/gacha` |
| `.sw-synth` | L1701 | `app/(main)/gacha/_inner.tsx:191` | `/gacha` |
| `.sw-synth-cap` | L1706 | `app/(main)/gacha/_inner.tsx:192` | `/gacha` |
| `.sw-synth-row` | L1712, L1733 | `app/(main)/gacha/_inner.tsx:195` | `/gacha` |
| `.sw-synth-track` | L1718, L1726 | `app/(main)/gacha/_inner.tsx:196` | `/gacha` |
| `.sw-synth-note` | L1738 | `app/(main)/gacha/_inner.tsx:212` | `/gacha` |
| `.sw-stat-row` | L1264 | `components/gacha-album.tsx:110` | `/gacha` |
| `.sw-stat` | L1273 | `components/gacha-album.tsx:111,115,119` | `/gacha` |
| `.sw-stat-lb` | L1280 | `components/gacha-album.tsx:112,116,120` | `/gacha` |
| `.sw-stat-vl` | L1285, L1291, L1294 | `components/gacha-album.tsx:113,117,121` | `/gacha` |
| `.gtabs` | L906, L918 | `components/game/game-tabs.tsx:18` | `/gacha`（经 `gacha-tabbed.tsx`） |

> `.sw-stat*` 与 `.gtabs` 同时也被 `/games/*` 大量使用（如 `components/game/pet-log.tsx`、`game-records.tsx`、`bigsmall-road.tsx`），属于**两边共用**。

**结论**：这 18 个类名一旦随 `arcade-sweet.css` 搬进 `games/layout.tsx`，`/gacha` 页面将失去票根册、合成面板、统计条、标签栏的全部样式（退化成无样式的裸 DOM）。

### 5.4 站外可达但**安全**的类名（21 个）

这些类名虽然出现在站外文件中，但**不会因为搬移而掉样式**，原因分三类：

**（a）仅 `[data-arcade="sweet"]` 作用域定义 —— 站外本来就没命中（19 个）**

`/gacha` 的 DOM 上没有 `data-arcade="sweet"` 属性（该属性只由 `games/layout.tsx:9` 挂载），所以这些规则**今天就没生效**，搬走无影响：

`.sw-album` `.sw-album-acts` `.sw-album-btn` `.sw-album-card` `.sw-album-cta` `.sw-album-empty` `.sw-album-empty-face` `.sw-album-exch` `.sw-album-face` `.sw-album-face-name` `.sw-album-face-r` `.sw-album-grid` `.sw-album-head` `.sw-album-meta` `.sw-album-panel` `.bg-sun` `.bg-mint` `.rounded-`（Tailwind 片段）`.text-`（Tailwind 片段）

> ⚠️ 但这暴露一个**既有问题**：`gacha-album.tsx` 里大段 `sw-album-*` 类名（`gacha-album.tsx:87,102,104,106,107,110-123`）**在当前生产环境下就是无样式状态**——因为作用域限定导致从未命中。这不是本次搬移造成的，但搬移后可以顺手修复。

**（b）全局 CSS 已有定义，arcade 不是唯一来源（4 个）**

| 类名 | 已有定义位置 | 判定 |
|---|---|---|
| `.pg-eyebrow` | `styles/layout.css:2558` | 不依赖 arcade（14 个站外路由在用，但样式来自 layout.css） |
| `.num` | `styles/base.css:452` | 不依赖 arcade |
| `.dot` | `styles/layout.css:2541`（`.chip-sel .dot`） | arcade 里只是复合修饰位，基础样式来自 layout.css |
| `.font-display` | `styles/theme-tide.css:703` | 不依赖 arcade |

**（c）无任何独立定义，仅作复合选择器修饰位（6 个）**

`.gold` `.green` `.on` `.gtab-ic` —— 只在 `.sw-stat-vl.gold`、`.gtab.on` 等复合位置出现，自身不带独立样式，搬走无影响。

### 5.5 重点路由逐个核实

| 路由 | 是否有站外 arcade 消费 | 备注 |
|---|---|---|
| `/gacha` | ⚠️ **有，18 个类名** | 唯一风险点。独立模块，依赖 `gacha_http` |
| `/farm`（PT 农场） | ✅ 无 | `app/(main)/farm/` 下只有 `page.tsx`，不引用 arcade 类名。**与娱乐屋"魔力农场"（`/games/farm`）确为两套独立实现** |
| `/gomoku` | ✅ 无 | 五子棋未引用任何 arcade 顶层类名 |
| `/medal-wall` | ✅ 无 | 用 `.pg-eyebrow`（来自 layout.css） |
| `/medals` | ✅ 无 | 同上 |
| `/shop` | ✅ 无 | 同上 |
| `/magic-pool` | ✅ 无 | 同上 |
| `/my-spark` | ✅ 无 | 同上 |
| `/tasks` | ✅ 无 | — |
| `/top` | ✅ 无 | 同上 |

---

## 六、裸类名全局污染风险核查

### 6.1 重要修正：93 个裸类名 ≠ 93 个污染源

用户提到「约 109 个不带 `arc-/gc-/fp-/pp-` 前缀的裸类名，有全局污染风险」。核查发现**这个担忧需要修正**——绝大多数「裸类名」并不是独立定义，而是**后代选择器的后半段**：

```css
.arc-sec .num   { … }   /* .num 裸，但只在 .arc-sec 内生效 —— 不污染 */
.jgrid .go      { … }   /* .go 裸，但只在 .jgrid 内生效 —— 不污染 */
.arc-q .bar     { … }   /* 同理 */
```

这类 `.num` / `.go` / `.bar` / `.ring` / `.dot` / `.win` / `.ready` 等**不会泄漏**，因为主体（subject）仍被父类约束。

**真正会全局泄漏的**＝该类名在顶层规则中**独立作为主体**出现（无父类前置）：

| 文件 | 裸类名总数 | 其中**独立定义（真泄漏）** | 受父类约束（安全） |
|---|---|---|---|
| `arcade.css` | 93 | **20** | 73 |
| `arcade-sweet.css` | 55 | **16**（其中 14 个已被 `[data-arcade]` 收窄） | 39 |

### 6.2 真泄漏的 20 个（`arcade.css`，全部未加作用域）

`.arc` `.jgrid` `.plot-bed` `.sc-face` `.sc-stars` `.cap-rig` `.cap-machine` `.cap-dome` `.cap-eggs` `.cap-egg` `.cap-base` `.cap-knob` `.cap-mouth` `.cap-drop` `.lb-hero` `.lb-table` `.lb-row` `.lb-head` `.lb-rank` `.lb-who`

> 注：`.lb-*` 是 leaderboard（/game/leaderboard）的排行榜样式；`.cap-*` 是扭蛋机；`.plot-bed`、`.sc-face`、`.sc-stars` 是刮卡。

### 6.3 真泄漏的 16 个（`arcade-sweet.css`）

`.arc` `.text-`（Tailwind 片段）`.font-display` `.bg-sun` `.bg-mint` `.sc-stars` `.sc-face` `.jgrid` `.beam` `.cap-machine` `.cap-dome` `.cap-base` `.cap-knob` `.gtabs` `.gtab` `.streak-pill`

### 6.4 重名冲突检测结果

将这 20 + 16 个真泄漏类名与 `pages.css` / `layout.css` / `base.css` / `medal-shop.css` / `comments.css` / `hover-card.css` / `theme-tide.css` / `motion-tide.css` 全部交叉比对：

### **结论：实质无冲突（仅 1 例，且无害）**

| 冲突对 | arcade 侧 | 全局侧 | 是否真冲突 |
|---|---|---|---|
| `.font-display` | `arcade-sweet.css:582` `[data-arcade="sweet"] h1.font-display` | `theme-tide.css:703` `[data-theme="baozi"] h1.font-display` | ❌ **不冲突** |

**为何不冲突**：两侧选择器都带各自的作用域属性限定（`[data-arcade="sweet"]` vs `[data-theme="baozi"]`），作用元素集不相交，永远不会打架。即使 `@import` 顺序变化也无影响。

**其余 35 个真泄漏类名**在 8 个全局 CSS 文件中**均无任何同名选择器**（无论独立定义还是后代/复合位置），因此：
- 不存在「@import 顺序决定谁赢」的竞争；
- 搬走后这些类名在站外页面**不会**因为丢失 arcade 规则而变化（因为站外页面本来就没有对应 DOM 使用它们——已由 import 图验证）。

> 补充：`.arc` 这个类名在两个文件都有独立定义（`arcade.css` / `arcade-sweet.css`），但同样无全局同名对手，且 `arcade-sweet.css` 侧带作用域限定，实际无冲突。

---

## 七、方案建议

### 推荐方案：拆成三个文件（改动最小、语义最清晰）

```
apps/web/app/styles/
├── arcade.css          # 保留：仅 /games/*，移入 games/layout.tsx
├── arcade-sweet.css    # 拆走「站外段」
└── arcade-gacha.css    # 新增：/gacha 需要的 18 个类名，移入 gacha/layout.tsx
```

**具体操作**：

1. **新建 `arcade-gacha.css`**：把 `arcade-sweet.css` 中下列未作用域规则整段剪切过去（约 300 行）：
   - L906–L960：`.gtabs` / `.gtabs::-webkit-scrollbar` / `.gtab` / `.gtab .gtab-ic` / `.gtab.on` / `.gtab:not(.on):hover`
   - L1264–L1300：`.sw-stat-row` / `.sw-stat` / `.sw-stat-lb` / `.sw-stat-vl` 及其 `.gold` / `.green` 修饰位
   - L1659–L1765：`.sw-cardbook*` / `.sw-card*` / `.sw-synth*` 全段
2. **`globals.css`**：删除第 26、28 行两条 `@import`（第 27 行注释一并清理）。
3. **`games/layout.tsx`**：加 `import "../styles/arcade.css"; import "../styles/arcade-sweet.css";`
4. **新建 `app/(main)/gacha/layout.tsx`**：加 `import "../../../styles/arcade-gacha.css";`

**收益**：全站 93 页不再下载 arcade 的 120KB；`/gacha` 只多下载几 KB 的 `arcade-gacha.css`；`/games` 行为完全不变。

### 备选方案对比

| 方案 | 优点 | 缺点 |
|---|---|---|
| **A. 拆第三个文件**（推荐） | 边界清晰；`/gacha` 零改动；可独立演进 | 需新建 1 文件 + 1 layout |
| B. 站外页面各自 `import` 整份 `arcade-sweet.css` | 改动最少（只加 2 行 import） | `/gacha` 白下载 68KB（虽比全站 93 页好，但仍是浪费）；且未来 `/gacha` 会被 144 条作用域规则持续干扰 |
| C. 给站外消费者改用别的主题 | 最干净 | 需改 `gacha/_inner.tsx`、`gacha-album.tsx` 的类名，改动面大、回归风险高 |
| D. 顺手给 `arcade-sweet.css` 剩余 144 条补作用域 | 根治「文件头注释与实现不符」 | 超出本次范围，建议**另开任务**；注意这会让 `sw-album-*` 在 `/gacha` 首次真正生效，**视觉会变**，需设计确认 |

### 建议的执行顺序

1. 先做方案 A（一进一出，风险可控）；
2. 单独提一个任务处理 D（补作用域），因为它会**改变 `/gacha` 的现有外观**，不能和搬移混在一次变更里；
3. 搬移后跑门禁：`tsc` + `vitest`，并**重点回归 `/gacha` 与 `/games` 两条线**的视觉。

---

## 八、方法论与可信度说明

- **CSS 解析**：自研括号深度感知解析器，剔除注释后按 `{`/`}` 深度切分，只取 `depth === 0` 的规则；`@media` / `@supports` / `@keyframes` 内部一律排除。已交叉验证：解析器输出「顶层规则 322 / 逗号片段 307」与用户提供的 307 口径吻合。
- **类名 token 提取**：同时覆盖 `className="..."` / `class="..."` 属性形式，以及 `cn()` / `clsx()` / `cva()` / `twJoin()` 调用内的字符串字面量（`arcade` 组件大量使用条件类名拼接）。
- **路由归属**：基于 import 图可达性，**不是**简单的文件路径前缀匹配——这是本次能识别出「`components/game/*` 被 `/gomoku` 共用」这类情况的关键。
- **已知局限**：
  - Tailwind 原子类（如 `text-[#0f62c4]`、`rounded-[4px]`）在提取时被误切为 `text-` / `rounded-`，报告中已标注为**假阳性**并排除。
  - 动态拼接的类名（如 `` `sw-card sw-tone-${c.tone}` ``）中的模板变量部分无法静态解析，`.sw-tone-*` 已在 sweet 类名列表中捕获。
  - 本报告为**纯静态分析**，未在浏览器中实际渲染验证。建议按方案 A 落地后做一次 `/gacha` + `/games` 的视觉回归。

---

*本报告为只读核查，未修改任何业务代码。*
