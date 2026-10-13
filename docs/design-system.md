# FluxTorrent 设计系统（对外正式文档）

> 本页是设计系统的**对外速查与上手入口**；API 细节速查见
> `apps/web/components/ui/README.md`，策划与验收真源见
> `docs/design/design-system-plan.md`（DS-PLAN-01），日常工作
> 操作面见 `docs/design/master-plan.md`（DS-MASTER-01 §5 坐标定位法）。

## 1. 分层宪法

```
L4 页面层 app/**/page.tsx   禁止 import L1（hex / var(--) 直用）
L3 模式层（页面级组合约定，成熟后下沉原语层）
L2 原语层 components/ui/*   纯展示+交互，无业务词汇
L1 令牌层 app/styles/*.css  色/字/距/角/影/焦点/断点/动效，唯一真源
L0 守护层 scripts/*guard.mjs + CI
```

## 2. 原语目录（一件一行；API 细节见 ui/README.md）

| 原语 | 用途 | 关键 props | 替代了什么 |
|---|---|---|---|
| Toast（toast.tsx 族） | 异步写操作编程式反馈 | `toast.*` 四档 | alert/静默/局部反馈（已收编）；带诊断 |
| ActionFeedback | 行内三态（busy/ok/fail） | `state`，文案可覆盖 dict.ui | 手写「提交中…」20+ 处 |
| BusyLabel | spinner+文本组合 | `label, busy, size` | 手写「加载中」散句 |
| ConfirmDialog | 危险确认唯一形态 | `tone`、`requireText` | window.confirm/alert |
| StatePanel | 数据四态路由器 | `loading/error/empty/children` | 三态各自手写的 if 链 |
| ErrorPanel | 错误码+request_id 复制+重试 | `code`、`requestId` | 手写「操作失败」无诊断 |
| EmptyState | 空态图标+文案+引导 | `tone`，`compact`，`action` | 手写「暂无数据」 |
| SkeletonRows/Block | 表格行/面板占位骨架 | `variant`，行高与真实对齐防跳动 | 无加载态白屏 |
| FormField | label+控件+hint+error 四槽 | `name`（织入 aria id） | 手写 label/error 布局 |
| FieldErrors | 后端校验错直出 | `fields`，`fieldErrorsFrom(e)` | 手写逐字段错误渲染 |
| SubmitButton | 提交生命周期+防双击 | `busy/ok/fail/label` | 手写 disabled+loading |
| SkipLink | 跳主内容锚点（全站唯一） | 无（根布局已挂载） | 无（a11y 新增） |
| PosterImage | 海报统一渲染（components/ 根，E 批） | 挂图回退/no-referrer | 裸 img 海报 |

## 3. tone × size 矩阵速查

| tone | 语义 | size | 字号 |
|---|---|---|---|
| brand | 品牌动作/主反馈 | sm | 12px |
| neutral | 中性信息 | md（默认） | 13px |
| success | 成功态 | lg | 14px |
| warn | 警告态 | | |
| danger | 错误/破坏性操作 | | |

组件 API 不出现色值与像素宽——调用方表达「这是什么」，不表达
「长什么样」。tone 与色的映射在令牌层完成（base.css `--tone-*`）。

## 4. 新增页面 checklist（引用 master-plan §5 坐标定位法）

动手前 30 秒自检：该页属 P0/P1/P2 哪批？P2 被触碰 → 顺带迁移
（master-plan §4 页面-阶段映射表）。

- [ ] 数据 UI 四态齐全（loading/empty/error/success——StatePanel 或
      显式骨架，答不上=设计未完成）
- [ ] 用了 components/ui 原语（无合适的且 ≥3 处复用前景 → 先立
      原语工单再写页面）
- [ ] 零硬编码 hex / 裸像素断点（用 var(--*) 语义令牌或 tone class；
      平台字面量加 design-cov-exempt 行级豁免并注明原因）
- [ ] 写操作反馈走 toast/ActionFeedback（禁 window.confirm/alert）
- [ ] 键盘可达（Tab/Esc/Enter）+ 焦点环可见；aria 语义到位
- [ ] 文案三语齐（zh-CN/zh-TW/en；ja 走回落）
- [ ] 失败态带 request_id 展示
- [ ] 本地跑门禁全绿（下一节一条命令一行）

## 5. 门禁清单与本地跑法（一条命令一行）

```bash
node scripts/line_limit_guard.mjs     # 行数 ≤300 / 行宽 ≤80，只许瘦
node scripts/i18n_guard.mjs           # 文案防新增硬编码中文
node scripts/design_cov_guard.mjs     # 覆盖率四指标只许向好
node scripts/ui_design_guard.mjs      # 令牌层退化六则 + tsx hex 棘轮
node scripts/check_dark_contrast.mjs  # 暗色 AA 回潮 + 明色债棘轮
node scripts/route_manifest.mjs --check      # 路由清单防漂移
node scripts/check_type_drift.mjs     # 前后端契约漂移对拍
node scripts/error_registry.mjs --check      # 错误码注册表对拍
```

修完一类问题后 `--update` 刷基线（确认是「有意放行」才刷）。

## 6. 门禁红测记录（W-47，2026-10-13 实际执行）

三项「故意违规被拦」的真实执行记录（验证门禁真的挡）：

**① 非豁免页面加 hex 被 ui_design_guard 规则 9 拦**

```bash
printf 'const _redTest = "#ff0000";\n' >> \
  "apps/web/app/(main)/about/page.tsx"
node scripts/ui_design_guard.mjs
# 输出：== 9. L4→L1：tsx 硬编码 hex（只减不增） ==
#       FAIL：1 处（基线 0，增加 1）  → exit 1（还原后复测 PASS）
```

**② design_cov 覆盖率数字调劣被拦（临时把基线调高复现）**

```bash
node -e "const f='scripts/design_cov_baseline.json';const b=require(
  './'+f);b.routeLoading=99;require('fs').writeFileSync(f,
  JSON.stringify(b,null,2))"   # 模拟指标恶化（基线 19→99）
node scripts/design_cov_guard.mjs
# 输出：FAIL：覆盖率指标恶化（只许向好）：
#       路由加载态覆盖 19% < 基线 99%（exit 1；git checkout 还原后 PASS）
```

**③ 路由增删未刷清单被拦（W-12 既有红测 + 本批活捉）**

W-12 落地批（ebc2e89c）做过红测双验证后字节级还原，记录见
master-plan §2 W-12。本批复核时**活捉一次真实漂移**——批五十九
（a1412028）把 12 条 admin 路由拆进新文件（perms_user/ops_jobs/
categories_admin/wiki_revisions/custom_pages_public 等），路由本体
未变但 handler 归属文件漂移，guard 如设计拦截：

```bash
node scripts/route_manifest.mjs --check
# 输出：ROUTE_MANIFEST_FAIL 清单过期：库中 745 条，源码 745 条
#       （新增 12 / 移除 12 / handler 改名 0）
#       + GET /admin/user-permissions @ .../admin_http/perms_user.rs
#       - GET /admin/user-permissions @ .../admin_http/perms.rs（余 11 组同类）
# 修复：路由变更须同步 docs/api/route-manifest.json，
#       运行 node scripts/route_manifest.mjs --write（exit 1）
```

该漂移属 ROUTE 线工作范围（route-manifest.json 不在本批文件清单
内），已移交：跑 `node scripts/route_manifest.mjs --write` 刷新即可。

三项均为真实执行：①② 在本批工作区执行并当场还原（还原后门禁
全绿），③ 为本批执行 `--check` 时的真实红灯输出（非构造）。

## 7. 已知债（登记不隐瞒）

- 明色套对比度：25 组关键配对中 17 组 <AA（muted/faint 族 3.1-4.3、
  状态色/白底 1.9-2.8、brand 3.4-3.6）——W-40 登记，只测不调（调值
  牵动 soft 底配套，独立决策）；暗色套 W-40 已校准全过线，脚本
  按回潮门禁盯防。清单与数字见 `docs/design/metrics-log.md`。
- window.confirm/alert 存量 50 处：P2 域触碰即迁，新增=0 由评审把关。
