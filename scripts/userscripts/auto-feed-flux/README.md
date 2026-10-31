# auto_feed · FluxTorrent 适配版

把 [auto_feed](https://github.com/tomorrow505/auto_feed_js)（PT 站一键转种油猴
脚本，GPL-3.0）接到 FluxTorrent：

- **源站**：FluxTorrent 种子详情页右下角出现「转载」按钮，列出你在 auto_feed
  里启用的目标站，点击即带完整载荷跳到对方发布页；面板同时显示
  MediaInfo/标签识别结果，并给出 PTGen 跳转；
- **目标站**：在 FluxTorrent 发布页收到载荷后，自动填标题/副标题/简介/
  MediaInfo/封面/豆瓣/PT-Gen/分类/媒介/分辨率/编码/制作组，按标签字典勾选
  中字/国语/DIY 等标签，并自动放入种子文件；
- 支持「查重」模式（只打开对方搜索页）；
- 线上站 `https://fluxtorrent.ptang.top/`（同站别名 `https://ft.ptang.top/`）
  已注册为站点 `FluxTorrent`；旧 `mfwg.ptang.top` 已不再解析。

> 上游脚本默认从 greasyfork/unpkg 加载依赖，国内网络常失败；本构建把
> `@require` 换成 jsdelivr/cdnjs/Gitee 镜像，并移除了 `@updateURL`，避免被
> 上游自动更新覆盖掉适配层。

## 安装

1. 浏览器装 Tampermonkey；
2. 构建产物（需要 Python 3；默认从 GitHub/Gitee 镜像拉上游）：

   ```bash
   python scripts/userscripts/auto-feed-flux/build.py
   # 已有上游文件时（快、可离线）：
   python scripts/userscripts/auto-feed-flux/build.py --src path/to/auto_feed.user.js
   ```

3. 安装 `scripts/userscripts/auto-feed-flux/dist/auto-feed-flux.user.js`：
   直接把文件拖进浏览器窗口，或在本地起个静态服务后打开 URL，例如：

   ```bash
   cd scripts/userscripts/auto-feed-flux/dist
   python -m http.server 8765
   # 浏览器打开 http://127.0.0.1:8765/auto-feed-flux.user.js
   ```

   注意 `dist/` 被 `.gitignore` 忽略（上游整包 1.6 MB，不入库），换机器要重新
   跑一次构建。已装过旧版时，重新拖入新的 dist 文件让 Tampermonkey 更新即可
   （当前版本 `2.1.1.4.6`）。

## 用法

### 从 FluxTorrent 转出

1. 打开任意已过审种子详情页；
2. 点右下角「转载」（首次会签发一枚 30 分钟有效的种子直链）；
3. 在面板里筛选/点击目标站，对方发布页会自动填充；
4. 勾「查重」后点击站点只打开对方搜索页，不跳发布页。

### 从其它站转入 FluxTorrent

在**同样装了这个适配版**的浏览器里，打开其它站的种子详情页，在 auto_feed
的转发列表里点 `FluxTorrent`（或你改的站点名），就会跳到
`https://fluxtorrent.ptang.top/upload#separator#…`（`ft.ptang.top` 同站）并自动填充。

> 上游原版 auto_feed 不知道 FluxTorrent；只装原版时，别的站不会列出 `FluxTorrent`。

### 付费种子

`FluxTorrent` 的下载直链签发会过下载资格/付费闸门：未购买的付费种子会提示先在本站
详情页购买下载；面板仍可生成文字载荷，但不会带种子文件。

## 项目侧契约

适配层依赖两件项目侧的小改动（已随本批落地）：

- `apps/web/app/layout.tsx`：`metadata.generator = "FluxTorrent"`，用于识别
  FluxTorrent 实例（线上域名同时有硬编码兜底，未部署也能用）；
- `apps/web/components/upload-form-*.tsx`：发布表单字段暴露稳定 `name`
  （`file/name/small_descr/imdb/douban/tmdb/ptgen_url/nfo/attach/mediainfo/
  price/poster/descr/category_id`）。表单自己拼 `FormData`，这些 `name`
  只服务外部工具，不影响原生提交。

`apps/web/tests/upload-transfer-names.test.tsx` 钉死这些字段名，防止重构
把转种链路静默弄断。

旧镜像没有这些 `name` 时，适配层会按行标题（简/繁/英）兜底定位；线上当前
镜像即可转入，部署新版后优先用 `name`。

## 实现要点

- 源站数据走 `/api/v1/torrents/{id}/aggregate`（标题/简介/MediaInfo/分类/
  维度/IMDb/海报/标签）；MediaInfo 优先读 `detail.mediainfo`，列为空时从
  简介的 `[quote]/[code]` 或正文里提取（含蓝光 DISC INFO），不会把整段
  剧情灌进 MI 框；
- 标签随载荷传递：源站标签会写进简介的「标签：…」行（跨站可被 auto_feed 的
  `get_label` 识别），并保留结构化 `tags` 字段供本站目标勾选；NP 的数字
  位掩码（1/10/100）、Unit3D 的数组/逗号串也会归一成标签名；**禁转 / 官种 /
  官方 / 首发属于站方行政标记，转种时一律跳过、不勾选**；
- PT-Gen：源站载荷带 IMDb/豆瓣链接，目标站会填进 PT-Gen 输入框；源站面板
  也提供 PTGen 跳转；
- 种子文件不再让目标站跨站拉 token：源站面板同源下载一次，用上游
  `build_blob_from_torrent` 清洗掉原 announce/passkey 与 announce-list，存进
  GM 存储（安全种子、占位 announce），载荷只带 key；目标站用自己的 announce
  重建文件。不受 Cloudflare/跨站限制，也不会把源站 passkey 暴露给目标页。
  >8MB 或 GM 存储不可用时回退 30 分钟 token 直链；无存档/读取失败会在面板
  直接显示真实原因，不再演成目标站的「种子文件加载失败」；
- 种子文件走 `POST /api/v1/torrents/{id}/download-link` 签发的临时直链，
  跨站下载不需要带本站 cookie；
- 载荷格式与上游 `dictToString` 完全一致（单测直接抽取上游函数对质）；
- 目标站填充用原生 value setter + `input/change` 事件驱动 React 受控表单，
  并等 React 水合完成后再写，避免首帧 state 覆盖；
- 分类按站型方言匹配（电影/电视剧/动漫/纪录片/音乐/游戏/软件/电子书/体育/
  综艺）；维度按「kind key → 标签 → 值别名」三层匹配：站长把 `media` 改成
  `format`、`codec` 改成 `video_codec`、值改成 `蓝光原盘`/`HEVC`/`UHD` 也能
  命中；制作组按目标站字典 ID 选（198 项 select 不会当自由文本）。

## 测试

```bash
cd scripts/userscripts/auto-feed-flux
node test_core.js       # 映射/载荷/aggregate 解析；含上游编解码对质
node test_patches.js    # 从 dist 抽取真实 set_jump_href / fill_torrent 执行
```

浏览器端到端（本机需有 FluxTorrent 栈）：

- 本地 `localhost:3000` 已验证：详情页面板、载荷、上传页自动填充、分类联动、
  种子文件写入、React 重渲染后值保持；
- 线上 `fluxtorrent.ptang.top` / `ft.ptang.top` 已验证：实例识别（域名兜底）与接口契约（分类/
  `section-dict` 字典）。

## 已知边界

- 标签、字幕组做**尽力**映射：本站目标按标签字典勾选中字/国语/DIY 等；
  跨站时 NP 的标签块能识别常见标签，但 Unit3D 各站标签字段实现不一，个别
  站仍可能需要手动补勾；字幕组不做自动选择；
- MediaInfo 只认文本（简介里的 MI 或独立字段）；如果源站把 MI 只做成
  截图/图片，无法提取；
- 豆瓣 / TMDB 只在源站简介里带链接时自动填；IMDb 走聚合接口始终带出；
- 站点启用/排序仍用 auto_feed 自带的设置页（那个页面需要任意 NexusPHP
  站的个人设置页做托管；FluxTorrent 页面本身不需要托管页）；
- 查重模式沿用上游各站的搜索 URL 规则，个别站可能改版失效；
- 临时直链 30 分钟过期；面板久开后再点会重新签发（有 10 次/分钟限流）；
- 上游 `@require` 的 imgCheckbox/music-helper 仍走 Gitee 镜像；如果 Gitee
  不可达，图片/音乐增强功能会降级，但 FluxTorrent 的转种链不受影响。

## 维护

- 上游更新后重跑 `build.py`；任一注入锚点命中数不为 1 会**直接失败**并指出
  是哪个锚点（不做静默跳过），按提示更新 `build.py` 的锚点即可；
- 只想换线上域名：改 `build.py` 的 `LIVE_SITE`/`LIVE_ICON`/`NAMESPACE`、
  `adapter.js` 的 `FLUX_SITE_HOSTS` 与 `@match` 行；`NAMESPACE` 是 TM 的
  更新身份，换域名时不要改，否则会装成第二个脚本；
- 上游协议为 GPL-3.0，本目录保留原作者与许可证信息。
