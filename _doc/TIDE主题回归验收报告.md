# TIDE「潮汐」主题 · 全站回归验收报告（P6）

日期：2026-09-21
范围：P1 令牌层 → P5 字体层全部上线后的全站回归
方法：playwright + 系统 Edge，带真实登录态（`flux.session=1` + `flux_token`，`path=/`）遍历 46 条主要路由，
每页采集 HTTP 状态、控制台/页面错误、横向溢出（`scrollWidth - innerWidth`）、渲染文本量、SVG 数量、
骨架 emoji 残留、失败请求。

## 一、结论

| 项 | 结果 |
|---|---|
| 遍历路由 | 46 |
| 首轮问题页 | 8 |
| **由本次主题工程引入的问题** | **1**（`/users/2` 头像占位 emoji `👤`，已修复并复验通过） |
| 复验问题页 | 7，**全部为误报或既有设计**，与本次改动无关 |
| 横向溢出 | **46 页全部 0px**（无布局破坏） |
| 首轮/复验报错 | 均为既有的权限/模块/水合问题，见下表 |

**判定：本次主题重设计（P1–P5）零业务回归。**

## 二、本次引入并已修复

| 路由 | 问题 | 根因 | 处置 |
|---|---|---|---|
| `/users/[id]` | 残留 emoji `👤`（头像占位） | P3 图标替换时漏了 `_parts/` 子目录下的文件（同批 grep 被 `head_limit` 截断所致） | `profile-hero.tsx:51` 改为 `<Icon name="user" size={34} />`，复验 OK |

## 三、既有问题 / 设计如此（不在本次范围）

| 路由 | 现象 | 定性 | 证据 |
|---|---|---|---|
| `/preserve` | 403 `/api/v1/seed-stats` | **设计如此**：该端点需 `SEED_STATS_VIEW` 权限（保种员/VIP），验证券是普通用户 | `staff_http/stats.rs:58-64` 注释明确"不走 staff 门槛，保种员/VIP 持该权限即可" |
| `/me/exams` | 404 `/api/v1/me/exams` | **设计如此**：`module_exams = no`，网关按模块开关拦截 | `modules/gateway.rs:65`；DB `site_settings.module_exams='no'` |
| `/reports` | 403 `/api/v1/admin/reports` | **权限设计**：页面内管理员区块调用管理端点，普通用户被拒 | 页面本身 200 且内容正常 |
| `/rules` | 无顶栏、SVG=0 | **独立布局**：`app/rules/page.tsx` 不在 `(main)` 分组下，不挂导航 | 路由位置 |
| `/appeals` | 91 字、无顶栏 | **独立布局 + 表单页**：内容完整（申诉类型/内容/记录），字数天然少 | 实测文本：`申诉通道 \| ... \| 我的申诉记录 \| 暂无申诉记录` |
| `/register` | 78 字 | **表单页**：内容完整（邀请码/用户名/邮箱/密码/验证码） | 实测文本；验收脚本的 120 字阈值对表单页不适用 |
| `/my-spark` | `PAGEERROR Minified React error #418`（水合不一致） | **既有缺陷**：`my-spark/page.tsx:40` 用 `const loggedIn = hasSessionCookie()`，该函数读 `document.cookie` → SSR 返回 `false`、客户端返回 `true` → 首屏 HTML 不一致 | `lib/api-client.ts:19-26` |

### 建议（可选，非本次范围）

`my-spark` 的水合问题标准修法是**挂载门控**：

```tsx
const [loggedIn, setLoggedIn] = useState(false);
useEffect(() => setLoggedIn(hasSessionCookie()), []);
```

即首屏（SSR 与首次客户端渲染）统一按未登录渲染，挂载后再切到真实状态。同类模式建议全仓扫一遍
（`hasSessionCookie()` / `typeof window` 分支 / `Date.now()` 直出）。

## 四、验收脚本

`shot-tide-p6.js`（playwright）：46 路由遍历 + 六项检查；结果落 `_tide_p6_regression.txt`。
可重复运行以做后续回归基线。

## 五、遗留待决策（产品层面）

1. **文案语气 emoji**：全站约 80 文件 / 300+ 处，其中 `i18n/*.ts` 每语言 47 处（如「✓ 上传成功」「🚀 安装向导」）。
   属**产品文案风格**而非 UI 图标体系，改动需动三语字典。
2. **字体自托管**：当前走 Google Fonts（已验证 Bricolage Grotesque 与 Noto Serif SC 均加载成功）。
   若要摆脱外网依赖，可自托管 woff2 子集，需引入构建步骤。
