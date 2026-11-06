# 5. 内容包与主题

> 入口：后台「内容包」（生态商店目录）。

## 内容包四品类

| 品类 | 内容 | 生效出口 |
| :--- | :--- | :--- |
| taxonomy | 分类/维度/标签词表整套 | 资源库组织 |
| theme | 主题令牌预设 | 全站配色/形态 |
| rules | 运营规则预设（HR/考核/经济参数簇） | 对应模块行为 |
| assets | 素材（勋章图/头像框/精品图） | `/medals`、`/avatar-frames` 展示与佩戴 |

## 安装流

1. 后台「内容包 → 目录」浏览内置包 + 远程索引（`marketplace_index_url` 指向的货架）；
2. 预览 diff（会应用哪些段、覆盖哪些键）→ 安装；
3. 应用走站型包同一套 apply 台账，可回滚。

## 自建与分发

- 自建：当前配置「另存快照」后即成 custom 包，全量配置可导出 JSON（密文掩码、diff 空跑）；
- 分发：把包 JSON 放到任意静态地址，加进远程索引即可让其他站长安装——这就是「生态商店」的货架机制。

## 主题

主题包 = 一组 theme_token 预设；配合 TIDE 系 CSS 令牌（`--compat-glow`、明暗双态），换主题不改布局只换气质。想要完全自定义：后台取色器逐键调，或导出主题包备份。

## 内置主题四档与「山海经」双层架构（2026-11-03）

切换器四档：日间（brand）/ 夜间（brand-night）/ 山海经（shanhaijing）/ 跟随系统。
主题注册表唯一真源 `apps/web/lib/themes.ts`（id → chrome/scheme），
no-flash 脚本、切换器、注册表门禁测试都从它消费。

山海经（岩彩长卷）分两层落地，边界如下：

| 层 | 内容 | 落点 |
| :--- | :--- | :--- |
| 甲 · 令牌层 | 约 140 个令牌槽位（四正色/玄墨表面/宣纸文本/金线/状态/圆角/阴影/焦点环/--compat-\* 兼容层/--mood-\* 画布色/--tide-\* 桥接） | `apps/web/app/styles/theme-shanhaijing.css`（只写 `[data-theme="shanhaijing"]` 作用域） |
| 乙 · 结构层 | 图鉴侧栏三态（`components/shj-atlas.tsx`）、Hero 山海图卷（`components/shj-hero.tsx`）、竹简表/印章角标皮肤、游戏大厅岩彩化（`arcade-sweet.css` 末段山海经块）、站点背景 CSS 星峦纹 | 同上 + 组件（恒渲染、CSS 控显隐） |

纪律：新色值只能进令牌块；组件零硬编码；三态结构组件对 brand/brand-night 一律 `display:none` 零影响。

回滚：① 用户侧：切换器换档即恢复；② 代码侧：删 `globals.css` 里 `theme-shanhaijing.css` /
`fonts-shanhaijing.css` 两行 @import + `themes.ts` 注册项 + `(main)/layout.tsx` 的
`<ShjAtlas>` 与 `page.tsx` 的 `<ShjHero>`，即回到三套主题，无残留开关。

2026-11-04 起切换器改为**下拉直选**（鼠标悬停不再误触展开），山海经档含
「宣纸」亮色亚档。分类图标与主题联动：内置两套位图图标集（aurora /
山海经），主题切档时 `CatIcon` 自动跟随。

## 站点与登录页背景图（2026-10-31 起）

后台「外观与主题」可配三处背景，URL 填站内附件或任意外链：

| 键 | 作用面 | 缺省 |
| :--- | :--- | :--- |
| `site_bg_url` / `site_bg_opacity` | 全站内容底图（固定底层，内容壳浮在其上） | 无图；不透明度 25 |
| `login_bg_url` / `login_bg_opacity` | 登录页背景 | 无图；50 |
| `login_page_bg_url` / `login_page_bg_opacity` | 登录页**主视觉整页层**（比 login_bg 更靠前，做满屏 hero 用） | 无图；40 |

不透明度 0–100（越大越清晰）；仓库 `public/backgrounds/` 自带三张默认样图可直接引用。
