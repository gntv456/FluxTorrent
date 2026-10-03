# 运行时验证：UI 修复效果（2026-10-03）

本轮改的全是 CSS 令牌与层叠——**静态门禁全绿不能证明运行时正确**。
之前的审计就是靠无头浏览器量 computed style，才抓出「切了夜间但
body 仍是 #f6f8fc」。深色模式这类问题只有真浏览器能验。

## 为什么脚本不在仓库里

脚本依赖 `playwright-core`，它装在 WorkBuddy 的 node 工作区
（`C:/Users/52sy/.workbuddy/binaries/node/workspace/node_modules`），
**不在 FluxTorrent 的 `node_modules` 里**。若放仓库：
- CI 跑不了（无该依赖）；
- 且 ESM 不认 `NODE_PATH`，Windows 绝对路径还要转 `file://` URL。

运行时副本：`node workspace/_verify_ui.js`（CJS，用
`require("playwright-core")`），由 `_scan_night.js` 同目录的
勘察脚本转写而来。

## 六项断言与对应修复

| # | 断言 | 对应 |
|---|---|---|
| 1 | 夜间 `body` 背景亮度 < 40 | P0-1 深色模式 |
| 2 | 夜间 `--surface-page` 为深色 | P0-1 |
| 3 | 夜间 `--ink` 为浅色 | P0-1 |
| 4 | 夜间 `--font-display` 非空 | 674 行第二处 `:root,` |
| 5 | 登录提示框完整可见、字号 ≥12px | P0-3 |
| 6 | `/games` 与 `/gacha` 样式均生效 | 娱乐屋 CSS 路由加载 |

## 夜间发白元素扫描

`_scan_night.js` 逐元素量 computed background 亮度，把发白的按
累计面积排序。**这是抓「令牌层修好了但硬编码色还在」的唯一手段**——
首轮跑出 9 个选择器（顶栏 `rgba(255,255,255,.9)` 在 8/8 页面发白、
娱乐屋 `.gc-card` 17.3 万 px²），改法都是换语义令牌或补夜间覆盖。

教训：令牌层只解决「语义色」，不解决「谁没走语义色」。硬编码色值
在夜间会原样保留，必须靠实测扫描逐个揪。

## 复跑方式

```bash
python _mint_jwt.py > _jwt.txt
cd <node workspace> && node _verify_ui.js    # 六项断言
cd <node workspace> && node _scan_night.js   # 发白元素扫描
```

改动 CSS 令牌后**必须重建镜像**（`docker compose -f
docker/docker-compose.yml up -d --build web`，约 25 分钟），
否则验的是旧产物——本轮已踩过一次（`--z-modal` 明明在源码里，
产物中查不到）。
