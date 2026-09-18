# FluxTorrent 论坛 UI/UX 设计策划

> 版本：v1.0 · 日期：2026-09-18
> 配套文档：`_doc/论坛版块完善策划案.md`（功能/数据/经济方案）
> 定位：在现有 **Aurora 设计系统** 之上，为「互动层 / 形态层 / 治理层」补齐定义一套一致、可落地、移动优先的论坛 UI 规范。
> 设计依据：现有 `apps/web/app/(main)/forums/**` 页面 + `components/forum-*.tsx` + `app/globals.css` 真实 token；竞品 UI 范式来自 Zsens / BBS-GO / Flarum / 修罗 / rhex 的实站与文档。

---

## 0. 一页结论

- 现有论坛 UI **结构正确、但表现力停在「PT 表格时代」**：版块首页是 `pting` 卡片流（已不错），但**主题列表与帖子详情都是 `nexus-table` 楼层表**，纯文本、无头像、无富渲染、无互动按钮、无标签/类型、移动端只靠隐藏列。
- 五大竞品 UI 的共同方向：**卡片化（rhex 画廊模式）、信息密度可调（列表/卡片切换）、头像+用户卡、富内容渲染（Markdown/代码高亮）、行动按钮就近（赞/藏/回复内联）、SPA 流式体验（Flarum）**。
- 本方案给出：**信息架构 → 页面级规范（含 ASCII 线框）→ 组件库 → 交互/状态模式 → 落地顺序**，全部复用 Aurora token，不另起炉灶；新增的仅是一组「论坛语义色 + 论坛组件」。

---

## 1. 设计目标与原则

| 原则 | 说明 |
|---|---|
| **对齐 Aurora，不造第二套** | 颜色只用 `--sky/--coral/--mint/--info/--danger` 及 surface 梯度；圆角 `--r-*`；阴影 `--shadow-card/--shadow-hover`；字体 `--font-display`（标题）+ 系统无衬线（正文）。 |
| **移动优先** | 主流程在 375px 可用；表格只在 ≥sm 保留，移动端转卡片列表。现有 `nexus-table` 仅作「桌面增强」，不作为唯一载体。 |
| **权限即 UI** | 无权限的按钮**直接隐藏**（不渲染灰禁用），与现有 `can_create/can_write/can_mod` 判定的前端消费方式一致。 |
| **行动就近** | 赞/藏/回复/举报等高频操作内联在帖子卡内，不藏进「···」菜单（除非 ≤sm 空间紧张）。 |
| **乐观更新** | 点赞/收藏/关注先本地翻转再回滚；与 `notifications`/`spark_ledger` 的异步写入解耦。 |
| **内容即富文本** | 帖子正文走 `MarkdownRenderer`（白名单 sanitize + 代码高亮），与 Phase 0 的 Markdown 管线对接。 |
| **状态完整** | 每个列表/流都有 加载中 / 空态 / 错误 / 末页 四态，统一用现有 `baozi-*`/`sky-soft` 卡片样式。 |

---

## 2. 设计系统对接

### 2.1 现有 token（直接复用，globals.css 已定义）
- 主色 `--sky #2fa8ff` / `--sky-deep #1e8ae8` / `--sky-soft #e8f4ff`
- 强调 `--coral #ff7a59`（现有「发主题」按钮即此色）
- 中性 `--ink #1f2a44` / `--sub #6b7a99` / `--line #e3edf7` / `--cloud #f5faff`
- 表面 `--surface-card #fff` / `--surface-page #f5faff` / `--surface-hover` / `--surface-sunken` / `--surface-cream #fffdf6`
- 阴影 `--shadow-card` / `--shadow-hover`；圆角 `--r-lg 20 / --r-md 14 / --r-sm 10`
- 字体 `--font-display: "ZCOOL KuaiLe"...`（标题用，保留论坛的「活泼社区感」）

### 2.2 需新增的论坛语义 token（建议加进 globals.css `:root`）
```css
/* 帖子类型语义色（映射策划案 Phase 2 的 topic_type） */
--type-normal:  var(--sky);
--type-bounty:  #f5a623;   /* 悬赏：琥珀金，强化「奖」 */
--type-poll:    var(--mint); /* 投票：青绿 */
--type-lottery: #9b6bff;   /* 抽奖：紫 */
--type-redpacket:#ff5a5f;  /* 红包：红 */

/* 状态徽标配色 */
--status-digest: #f5a623;   /* 精华 ⭐ */
--status-sticky: var(--coral); /* 置顶 📌 */
--status-locked: var(--sub);   /* 锁定 🔒 */

/* 标签调色板（循环取，12 色柔和） */
--tag-1..12: 预置 12 个低饱和 pastel，文字用 --ink、底用对应 soft。

/* 互动激活态 */
--like-active: #ff5a7a;     /* 点赞红 */
--fav-active:  var(--type-bounty); /* 收藏金 */
```

### 2.3 组件库存（现状 vs 待建）
| 现状已有 | 待建（本方案新组件） |
|---|---|
| `pting-forums`/`pting-card`/`pting-sidenav`（首页） | `CategorySection`、`BoardCard`、`TopicCard`（列表/画廊两态） |
| `nexus-table`（主题列表/帖子流） | `PostCard`、`UserAvatar`、`UserCard`(侧栏) |
| `forum-composer.tsx`(TopicComposer/ReplyBox/ModActions/PostActions) | `RichEditor`(Markdown+预览+@)、`TypePicker`、`TagInput`、`BountySetup`、`PollSetup`、`VisibilityPicker` |
| `forum-search.tsx`(ForumSearch) | `FilterSortBar`、`SearchResultsPage`、`MentionTextarea` |
| — | `VoteBar`(赞/藏)、`FollowButton`、`TypeBadge`、`TagChip`、`RedPacketWidget`、`BountyCard`、`PollWidget`、`Toast`/`useOptimistic` hook |

---

## 3. 信息架构（增强后站点地图）

```
/forums                         论坛首页（分类分组 + 节点栏 + 列表/画廊切换）
/forums/{id}                    版块页（筛选/排序条 + 主题流 + 发帖入口）
/forums/{id}?filter=...&sort=hot|new&view=list|gallery
/forums/new                     选版块（仅 can_create）
/forums/topic/{id}              主题详情（帖子流 + 侧栏 + 回复框）
/forums/topic/{id}?before=...   游标翻页（沿用现有）
/forums/search?q=              搜索结果页（全文/标签/作者/版块）
/forums/tags/{tag}              标签聚合页
/notifications                  通知中心（@提及/关注更新/悬赏采纳/红包到账）
/u/{username}                   个人主页（新增「论坛」Tab：发帖/回复/徽章/贡献）
/admin/forums                   版块管理（已有）
/admin/forums/reports           举报/审核台（Phase 3 新增）
```

---

## 4. 页面级 UI 规范（含 ASCII 线框）

### 4.1 论坛首页 `/forums`（在现有 pting 布局上升级）
- 左栏 `pting-sidenav` 升级为**分类折叠**（点分类跳锚/过滤），保留彩色圆点。
- 右栏顶部 `ForumSearch` + 「最新/热门」全局切换；主体改为**按 `forum_categories` 分组**的 `CategorySection`，每组内 `BoardCard` 网格。
- `BoardCard` 含：图标圆点、版块名、简介、统计(主题/帖)、最新帖预览、类型标识（若该版块主营悬赏等）。

```
┌─────────────┬───────────────────────────────────────────┐
│ 全部分类 ▾   │ [🔍 搜索论坛…]      [最新▾] [☰列表/▦画廊]   │
│ ● 站务       ├───────────────────────────────────────────┤
│ ● 资源       │ 资源交流                                     │
│ ● 技术       │ ┌────────┐ ┌────────┐ ┌────────┐           │
│ ● 求助       │ │💡资源发布│ │📌置顶区│ │❓求助问答│ ...      │
│              │ │ 12主题   │ │ 8主题   │ │ 30主题  │           │
│              │ │ 最新：…  │ │ 最新：… │ │ 最新：… │           │
│              │ └────────┘ └────────┘ └────────┘           │
│              │ 技术讨论                                     │
│              │ ┌────────┐ ┌────────┐                       │
│              │ │💻编程   │ │🎬影视   │                       │
│              │ └────────┘ └────────┘                       │
└─────────────┴───────────────────────────────────────────┘
```

### 4.2 版块页 `/forums/{id}`（主题列表现代化）
- 顶部：面包屑 + 版块名 + `TopicComposer`（仅 can_create）+ `FilterSortBar`（类型/标签筛选、sort=hot|new、view=list|gallery）。
- 列表态：保留 `nexus-table` 但**加列**——类型徽标、标签 chips、最后回复人头像、热度条；移动端转 `TopicCard` 列表。
- 画廊态（rhex 借鉴）：封面卡 + 悬停预览摘要（仅对有封面的帖子；无封面用首字/渐变占位）。

```
[📌站务] 站务公告            [+ 发主题]
[类型▾][标签▾][排序:最新▾] [☰列表][▦画廊]

列表态（≥sm 表格，<sm 卡片）：
 ⭐📌 类型  标题                 作者      回复  浏览  最后回复
 ⭐   💡   关于XX的征集…        用户A      12   300   🟢用户B 2h
      💡   求助：…              用户C       3    80   🟢用户D 5h
 ●  📌  公告一则              管理員      40  1200  🟢管理員 1d

画廊态：
 ┌──────┐ ┌──────┐ ┌──────┐
 │ 💡   │ │ ❓   │ │ 📌   │  封面/占位 + 标题 + 作者 + 热度
 │ 标题 │ │ 标题 │ │ 标题 │
 └──────┘ └──────┘ └──────┘
```

### 4.3 主题详情 `/forums/topic/{id}`（帖子流卡片化）
- **放弃纯 `nexus-table` 楼层**，改 `PostCard` 流（桌面双栏：左窄用户卡 / 右宽内容；移动端上下堆叠）。
- 每帖 `PostCard`：楼号 + `UserAvatar` + 用户名 + 等级/勋章小标 + 时间 +（编辑留痕）+ 正文(`MarkdownRenderer`) + `VoteBar`(赞/藏) + 回复/@ + 举报 + 楼中楼(Phase 3)。
- 顶部：`TopicModActions`（版主，已有）+ 类型大徽标 + 标签 + 可见性提示（如「回复后可见」）。
- 右侧/底侧：`UserCard`（楼主或楼主+热门回复者）、相关悬赏/投票/红包挂件、目录(长帖)。
- 底部：`RichEditor` 回复框（can_write 才显示；locked 显示提示，沿用现有逻辑）。

```
[← 版块] » 站务公告     ⭐精华  💡普通
[置顶][锁定][加精][移动▾][删除]            ← 仅 can_mod

┌─ 1F ──────────────────────────────┐
│ 🟢用户A Lv.5 🏅 [头像]  2小时前      │
│ 正文（Markdown 渲染：代码高亮…）      │
│ 👍12 💾3  ↩回复  @提及  🚩举报       │
│ └ 楼中楼（Phase3）                    │
└────────────────────────────────────┘
┌─ 2F ── ... ───────────────────────┐
└────────────────────────────────────┘
[ 悬赏挂件：剩余 200 spark · 2 人参与 ]  ← 类型挂件
[ 💬 回复框（Markdown + @ + 预览） ]
```

### 4.4 发帖/编辑器（RichEditor，Phase 0/2 核心入口）
- 顶部 `TypePicker`：普通 / 悬赏 / 投票 / 抽奖（默认普通；非普通展开对应配置卡）。
- 标题输入（沿用现有 maxLength=120）。
- `MarkdownRenderer` 预览联动的 `MentionTextarea`：工具栏(粗体/代码/图片/表情) + 实时预览 + `@` 弹出用户候选（接 `/users/search`）。
- 侧/底配置：`TagInput`（标签 chips）、`VisibilityPicker`（公开/登录可见/回复可见/VIP 可见/匿名）、悬赏额/投票项/抽奖参数（按类型动态显隐）。
- 提交走现有 `POST /forums/topics`，body 存 Markdown 源 + `body_text` 摘要（Phase 0）。

### 4.5 通知中心 `/notifications`（Phase 1）
- 分组：未读/全部；条目类型着色（@提及=sky、悬赏采纳=type-bounty、红包=type-redpacket、关注更新=sub）。
- 点击跳对应主题/楼层，标记已读（乐观）。

### 4.6 个人主页·论坛 Tab（Phase 1）
- 复用现有 `/u/{username}`（如有）或新建：发帖数、回复数、精华数、获赞、活跃勋章、`spark` 贡献；Tab 切换「主题/回复/收藏/关注」。

### 4.7 搜索结果页 `/forums/search`（Phase 3 增强）
- 从现有 `ForumSearch` 内联结果升级为独立页：左侧筛选项（版块/标签/作者/类型/时间），右侧结果卡列表（标题+摘要+类型+标签+热度），高亮命中词。

### 4.8 举报/审核台 `/admin/forums/reports`（Phase 3）
- 队列卡片：被举报对象缩略、举报理由、状态(待处理/已处理)、处理动作（删帖/禁言/忽略），与现有 `post_delete`/`forumpost` 打通。

---

## 5. 核心组件库规范（接口草图）

```tsx
// 头像+等级+勋章小标（侧栏/帖子卡复用）
<UserAvatar user={{id,username,class,medals}} size="sm" />

// 类型徽标：normal|bounty|poll|lottery|redpacket
<TypeBadge type={t.topic_type} />

// 标签 chip（可点跳转 /forums/tags/{tag}）
<TagChip tag={name} onClick={...} />

// 赞/藏条（乐观更新 + 调 /forums/posts/{id}/like）
<VoteBar likes={n} liked={bool} faved={bool} onLike={} onFav={} />

// 关注按钮（forum|topic|user）
<FollowButton target={{type,id}} followed={bool} />

// Markdown 渲染（白名单 sanitize + highlight.js）
<MarkdownRenderer source={post.body} onMention={(u)=>...} />

// 悬赏/投票/红包 挂件（按 topic_type 挂载于主题详情）
<BountyCard topic={t} />  <PollWidget topic={t} />  <RedPacketWidget topic={t} />

// 富文本编辑器（标题/正文/@/预览/类型/标签/可见性）
<RichEditor forumId={fid} topicType={...} onSubmit={...} />

// 轻提示
<Toast />  // 或 useToast()
```

---

## 6. 交互与状态模式

- **乐观更新**：`VoteBar`/`FollowButton` 点击即翻转本地态，失败回滚并 toast；与服务端 `spark_ledger` 幂等互不阻塞。
- **权限门控**：组件接收 `can_*` 布尔，无权限直接不渲染（与现有 `TopicComposer` 仅 `can_create` 渲染一致）。
- **响应式断点**：`sm`(640) 以下 `TopicCard`/`PostCard` 卡片堆叠；`sm+` 表格/双栏；`lg+` 详情页右置 `UserCard` 侧栏。
- **深色模式**：全部走 CSS 变量，组件**禁止写死颜色**，沿用 globals.css 的 `:root`/暗色块切换。
- **加载/空/错/末**：列表骨架用 `surface-sunken` 占位条；空态用 `--sky-soft` 居中插画+文案（复用现有 `py-10 text-center text-sub` 范式）；末页隐藏「加载更早」按钮（沿用 `has_more` 逻辑）。
- **无障碍**：按钮 `aria-label`；徽标 `title`；表单 `label` 关联（现有 composer 已规范）。

---

## 7. 关键 UX 决策与理由

| 决策 | 选择 | 理由 |
|---|---|---|
| 帖子流形态 | **卡片流**（弃纯表格） | rhex/BBS-GO/Flarum 一致；表格不利富文本/头像/行动按钮；现有 `nexus-table` 仅留作桌面密度选项 |
| 列表/画廊双视图 | 都做，toggle | rhex 验证「画廊提升浏览效率」；列表满足信息密度党 |
| 楼层 vs 楼中楼 | 默认楼层（兼容现有 `before` 游标），Phase 3 加楼中楼 | 不破坏现有分页/SSR；楼中楼仅作长帖增强 |
| Markdown 渲染 | 服务端/客户端白名单 sanitize | 防 XSS（策划案风险已列）；代码高亮对技术社区刚需 |
| 移动端 | 卡片堆叠 + 底部发帖 FAB | 现有仅隐藏列体验差；FAB 提升发帖转化 |
| 类型表达 | 色彩徽标 + 挂件 | 悬赏/投票/红包用强色区分，降低认知成本 |

---

## 8. 落地顺序（对接功能策划案 Phase 0–3）

1. **Phase 0（地基 UI）**：首页分类分组 + `BoardCard`/`CategorySection`；`TypeBadge`/`TagChip`；`MarkdownRenderer` 接入主题详情；列表/画廊 toggle 骨架。
2. **Phase 1（互动 UI）**：`PostCard` 替代详情表格；`UserAvatar`/`UserCard`；`VoteBar`/`FollowButton`；`/notifications` 页；`@` 提及输入雏形。
3. **Phase 2（形态 UI）**：`RichEditor`(TypePicker/TagInput/Visibility/Mention/预览)；`BountyCard`/`PollWidget`/`RedPacketWidget` 挂件挂载。
4. **Phase 3（治理 UI）**：`/forums/search` 独立页 + 筛选；`/admin/forums/reports` 审核台；敏感词前端提示；RSS 入口。

> 复用纪律：所有新组件**只用 Aurora token + 现有 `baozi-*` 兼容层**；颜色/圆角/阴影不硬编码；暗色主题自动继承。

---

## 9. 风险与注意
- **XSS**：`MarkdownRenderer` 必须白名单 sanitize（如 DOMPurify / 受控渲染），禁止 `dangerouslySetInnerHTML` 裸用。
- **性能**：`PostCard` 流长帖分页沿用现有 `before` 游标；头像/封面懒加载；画廊模式封面缺失用渐变占位避免破图。
- **一致性**：新增 `forum` 相关页面须过现有 `requireModule("forums")` 门控，保持 site_type 可关。
- **不动已有接口**：组件只消费既有/新增 API，不修改 `forum_access` 等已验证权限逻辑。

---

## 10. 落地偏差记录（实测推翻的判断，务必先读）

| 策划原案 | 实测结论 | 处置 |
|---|---|---|
| 新建 `/notifications` 页 + 顶栏红点 + `notifications` 表 | **FT 已有**：`messages` 收件箱（`location=1`）、`pmboxes` 自建文件夹、未读筛选、顶栏 `unread_messages` 红点（`user-box.tsx`），且既有范式就是插私信（勋章礼物） | **不建新表新页**：论坛通知插 `messages`，`sender_id IS NULL` 即系统通知；发件人列显示「系统通知」。红点/未读/归档零改动生效 |
| 举报台需从零做（Phase 3） | **FT 已有**：`reports` 表 + `POST /reports`（`ref_type` 白名单**已含 `forum`**，指向 topics 且校验存在性）+ 管理端 resolve/claim/release + `/reports` 页 | Phase 1 只补**上下文内一键举报**按钮；Phase 3 的「举报台」降级为「审核体验优化」 |
| `TagChip` 复用 `tags` 表 | `tags` 是 **torrent 标签**（`torrent_id, tag_id`），语义不同 | 论坛标签须新建 `topic_tags`/`tag_dict`，不能复用 |
| 帖子详情改 `PostCard` 卡片流 | 现表格含楼层游标（`before`）、编辑/删除/版主操作，已闭环 | 保持表格骨架，先补互动件（点赞条/举报）与 Markdown；卡片流作为后续视觉迭代，不与功能改动混做 |

> 教训固化：**「这功能有没有」的判据在接口层与页面目录，不在表结构**。
> 动手前四步：`ls migrations/` → `grep "pub async fn" worker/jobs.rs` → `grep -rn "关键词" apps/api/src/*_http.rs` → `ls "apps/web/app/(main)/"`。
