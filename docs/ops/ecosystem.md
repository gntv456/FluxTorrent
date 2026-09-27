# 工具生态收录（PT-Plugin-Plus / Jackett / cross-seed）

站点的真实「日常界面」往往是浏览器插件与自动化工具，而非网页本身。本系统已内置三条兼容通道，本文说明如何让工具生态认得你的站。

## 已内置的三个通道

| 通道 | 端点 | 适配对象 |
| :--- | :--- | :--- |
| PTPP 用户信息 | `GET /api/v1/plugins/ptppUserInfo`（API Token 鉴权） | PT-Plugin-Plus / PT-depiler 的用户信息卡 |
| NP 兼容形状 | `GET /compat/nexusphp/*`（user.json / torrents.json / download.php） | 按 NexusPHP DOM/API 口径工作的旧工具 |
| 开放 API + RSS | `/openapi` spec、`/rss/{token}` | flexget / Sonarr 类 / 自写脚本 |

详细字段与用法：`_doc/开放API接入指南.md`。

## 提交收录（站长可选的对外动作）

1. **PT-depiler**（PT-Plugin-Plus 的 MV3 继任者，明确兼容 NexusPHP/UNIT3D/Gazelle 三系）：
   - 到其仓库 `pt-plugins/PT-depiler` 提交站点定义（site config JSON）；
   - 我们走自研站通道：用户信息卡用 `ptppUserInfo` 端点（朱雀口径，已验证的最短路径），搜索/推送走开放 API；
   - 提交前自测：插件「用户信息」能拉到等级/上传量即通。
2. **Jackett**：基于 UNIT3D/NexusPHP 通用定义改 FluxTorrent 定义——由于 `/compat/nexusphp/torrents.json` 提供同形状输出，通用定义通常直接可用；提 PR 到 `Jackett/Jackett/definitions`。
3. **cross-seed**：走 Torznab（Jackett 定义就绪后自动可用）或 RSS 模式；注意其对 tracker 负载的影响（建议 RSS 低频）。

## 对站长的意义

收录后用户可以用熟悉的插件聚合搜索、一键推送到下载器、跨站辅种——显著提升留存与保种率。收录动作只需一次，之后随版本保持 `/compat` 端点稳定即可（这属于公开 API 承诺的一部分）。
