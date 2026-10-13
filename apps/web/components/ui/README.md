# components/ui — 设计系统原语层（L2）

> DS-PLAN-01 §2/§3 的唯一落地目录。**新 UI 先问「这里有没有」**；
> 没有且预见 ≥3 处复用，先立原语工单再写页面（DS-MASTER-01 §5）。

## 分层宪法（违反即打回）

```
L4 页面层 app/**/page.tsx   ── 禁止 import L1（hex / var(--) 直用 / Tailwind 色类）
L3 模式层（搜索/列表/表单/详情组合约定，按页内生发，成熟后下沉）
L2 原语层 components/ui/*   ── 本目录：纯展示+交互，无业务词汇
L1 令牌层 app/styles/*.css  ── 色/字/距/角/影/焦点/断点/动效，唯一真源
L0 守护层 scripts/*guard.mjs ── 覆盖率度量与 CI 门禁
```

- L2 组件**禁止**出现 torrent/bank/medal 等领域名词，只认
  tone/size/variant/state 这类设计词汇；
- 每个组件：≤300 行（line_limit 门禁）、四语 i18n（ja 走段级回落）、
  键盘可达、aria 语义齐全；
- 样式一律进 `app/styles/ui.css` 并只读令牌（tone/表面/动效三族）。

## 设计词汇：tone × size

| tone | 映射（base.css 词汇层） | 语义 |
|---|---|---|
| brand | `--tone-brand*` | 品牌动作/主反馈 |
| neutral | `--tone-neutral*` | 中性信息 |
| success | `--tone-success*` | 成功态 |
| warn | `--tone-warn*` | 警告态 |
| danger | `--tone-danger*` | 错误/破坏性操作 |

size：`sm`(12px) / `md`(13px，默认) / `lg`(14px)。
组件 API **不出现色值与像素宽**——调用方表达「这是什么」，
不表达「长什么样」（DS-PLAN-01 §10.1 宪章 2）。

## 原语 API 约定（2026-10-13 阶段一/二）

规范来源：DS-PLAN-01 §3（清单与 API 骨架的唯一真源）；本节是速查，
两者冲突以 DS-PLAN §3 为准并按 MASTER §7 改文档。

### 反馈通道（§3.1）

**Toast 系统**（`toast.tsx` + `toast-provider.tsx` + `toast-card.tsx`）
编程式反馈的唯一形态：Provider 挂根布局一次，任意深度调 `toast.*`。
用途：**异步写操作的结果**（保存/发送/批量）——成功即确认、失败带诊断。
关键 API：`toast.success/error/info/warn(msg, opts)`；`opts.desc` 第二行、
`opts.action` 动作钮、`opts.requestId` 失败可诊断芯片（必附）；
`toast.optimistic(msg, { run, rollbackMessage })` 乐观更新（W-17）；
`toast.dismiss(id)`。何时不该用：非异步的即时 UI 变化（用行内三态）、
页面级数据加载错误（用 ErrorPanel/StatePanel——错误需要占位而不是浮层）。

**ActionFeedback**（`action-feedback.tsx`）行内三态
`state="busy|ok|fail|idle"`（idle 不渲染）；busy=spinner、ok=✓ 1.5s
自动归隐、fail=✗ 常驻；aria-live 播报；文案默认走 dict.ui 可覆盖。
**BusyLabel**（`busy-label.tsx`）`{ label, busy }`——spinner+文本；
busy=false 退化纯文本，同一调用点数据就绪后不换组件；也导出
`PrimitiveSize`。何时不该用：结果需要用户记住的（toast 更合适）、
按钮自身的提交态（用 SubmitButton）。

**ConfirmDialog**（`confirm-dialog.tsx`）替代 window.confirm 的唯一形态：
焦点陷阱 + Esc + danger 遮罩不可点关 + requireText 删除类二次输入 +
children 补充槽（数量选择/总价等插在 message 下）。何时不该用：
可撤销操作（直接执行 + toast.optimistic 回滚更好）。

### 数据四态（§3.2）

**StatePanel**（`state-panel.tsx`）四态路由器
`{ loading, error, empty, children }`，判定顺序 loading→error→empty；
`loadingFallback` 换骨架形态、`emptyTitle/Desc/Action/Tone` 透传空态、
`onRetry` 给重试。何时不该用：非数据区块（纯静态/交互件不需要）。

**ErrorPanel**（`error-panel.tsx`）错误码 + request_id 一键复制 + 重试
（RSC 用 `retryHref`）+ dev 折叠堆栈。服务端页面先过
`normalizeError()`（Error 实例不可跨 RSC 边界）。何时不该用：可静默
降级的次要数据（藏起来比打扰好）。

**EmptyState**（`empty-state.tsx`）图标+文案+引导三件套；
`tone: neutral|brand|success|warn` 只改图标圆底；`compact` 卡内模式。
何时不该用：空态本身就是错误的表现（先 ErrorPanel）。

**SkeletonRows / SkeletonBlock**（`skeleton.tsx`）表格行 / 面板占位；
默认与真实行高对齐防跳动。何时不该用：操作等待（用 spinner 族）。

### 表单协议（§3.3）

**FormField**（`form-field.tsx`）label+控件+hint+error 四槽位，
`name` prop 决定稳定 id（控件 `field-<name>`、错误节点
`field-<name>-err`），id/aria-describedby/aria-required/aria-invalid
自动织入子控件（单元素子节点约定）。
**FieldErrors** `<FieldErrors fields={{field: [msgs]}}>` 表单级汇总
（点字段名聚焦对应控件）+ `fieldsFrom(data)` / `fieldErrorsFrom(e)`
直出后端 1002 信封；`ApiError.fields` getter（api-client.ts）把
data=[{field,error}] 解成 Record<field, msgs[]>。
**SubmitButton**（`submit-button.tsx`）`{ busy, ok, fail, label }` 四态
与 ActionFeedback 联动、busy 防双击。何时不该用：非 submit 语义的
动作钮（普通 button + BusyLabel）。

### 布局与 a11y（§3.4/§3.5）

**SkipLink**（`skip-link.tsx`）全站唯一，挂根布局，勿二次挂载。
**PosterImage**（`../poster-image.tsx`）海报统一渲染：2:3 口径 +
挂图回退 + no-referrer（E 批交付，纳入清单管理）。

## 新增原语 checklist（全部满足才进 ui/）

1. ≥3 处复用前景（贴出调用点清单或迁移计划）；
2. 无业务词汇——props 只用 tone/size/variant/state；
3. 四语 i18n（文案走 dict，ja 段级回落，禁 CJK 字面量）；
4. 键盘可达 + aria 语义（role/live/label 逐条自检）；
5. ≤300 行、样式进 ui.css 只读令牌；
6. 走 DS-PLAN-01 §10.2 流程：提案→评审→实现→本表登记→
   design_cov_guard 数字变化写进 PR 描述（MASTER §7）。
