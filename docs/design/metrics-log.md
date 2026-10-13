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
