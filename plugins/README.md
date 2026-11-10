# FluxTorrent 辅种适配 · 青蛙辅种助手（CrossSeed）

本目录是 **MoviePilot 插件「青蛙辅种助手」（`CrossSeed`，作者 233@qingwa）**
的 **FluxTorrent 站点适配版**。

- 上游源码：<https://github.com/jxxghp/MoviePilot-Plugins> → `plugins/crossseed`（v1）
  与 `plugins.v2/crossseed`（v2）
- 上游许可：GPL-3.0。本适配同样以 GPL-3.0 发布。
- 本适配**只改插件侧，FluxTorrent 站点侧一行代码不动**。

---

## 这个适配做了什么（以及为什么）

上游插件对**所有站点**只有一套方言（NexusPHP 系）：

| | 上游方言 | FluxTorrent |
|---|---|---|
| 查询 | `POST {url}api/pieces-hash` | `POST {url}api/v1/crossseed/lookup` |
| 凭据 | body 里的 `passkey` | 头 `Authorization: Token <fxo_…>` |
| 请求体 | `{passkey, pieces_hash:[…]}` | `{pieces_sha1:[…]}` |
| 响应 | `{"data":{sha1:torrent_id}}` | `{"data":{"results":[…]}}` |
| 下载 | `GET {url}download.php?id=&passkey=` | `GET {url}api/v1/crossseed/dl/{id}` |

三种差异（**请求体 / 凭据位置 / 响应结构**）互不兼容，所以插件按**站点名**
或**凭据形态**分派：站点名命中 `FluxTorrent` / `好学`，或者凭据是 `fxo_`
前缀的开放 API 令牌（只有 FluxTorrent 架构会签发）时走本站原生 API，
其余站点逐字保持上游行为（含「憨憨」的 `nexusapi/pieces-hash` 特例）。

> ⚠️ 最容易踩的坑：两种响应的 `data` 结构完全不同。
> 把上游的 `data.items()` 套到 FluxTorrent 的 `data` 上，会把字符串
> `"results"` 当成**指纹名**，产出一条 `pieces_hash="results"` 的**假命中**。
> 插件随后拿这个假指纹去反查本地种子的保存路径，必然落空。
> 这就是本适配必须单独写解析器的原因，也是回归测试里专门锁死的一条。
>
> ⚠️ 第二个坑：站点每条 result 里有两个容易混淆的字段——
> `match` 是「命中的是哪一类」（`info_hash_v1` / `pieces_sha1`），
> `query` 才是**原指纹本身**。解析只认 `query`；拿 `match` 顶替会得到一个
> 本地永远查不到的假指纹。

**为什么不让站点去实现上游方言**：那要给 FluxTorrent 新增一个
`/compat/nexusphp/api/pieces-hash` 端点——等于为一个第三方客户端的私有
协议改站点。而站点**本来就有** `/api/v1/crossseed/*`（开放 API 的 `crossseed`
scope），语义完全一样。适配插件就够了，站点不必迁就任何人的方言。

---

## 目录结构

```
plugins/
  crossseed/        # plugins.v2/crossseed 对应（v3.0.5，MoviePilot V2 后端）
  crossseed_v1/     # plugins/crossseed 对应（v2.4.3，MoviePilot V1 后端）
  tests/
    conftest.py                 # app.* 依赖的最小桩 + 插件加载器
    test_crossseed_flux.py      # 单元测试（19 用例 × 2 版本 = 38）
    test_contract_flux.py       # 契约验证（起假站点，插件真实代码打过去）
  _upstream/
    package.json / package.v2.json   # 上游市场元数据副本（含版本与 history）
    flux-adaptation.patch             # 对上游两版插件的 unified diff
```

两个版本目录**必须同步改**：它们是同一份上游代码的两个分支，任何一版漏改
都会表现为「有的 MoviePilot 能用、有的不能」。回归测试对两版跑同一套用例。

---

## 怎么用

### 1. 在 MoviePilot 里加站点

站点列表里加一个站点，**名称填 `FluxTorrent` 或 `好学`**（大小写敏感），
地址填站点根 URL（如 `https://pt.example.com`）。

> **为什么是这两个名字**：好学（hxpt.org）已由 NexusPHP 整体迁移到
> FluxTorrent 架构，对外契约与本项目完全一致，所以同一套适配同时覆盖两者。
>
> MoviePilot 官方站点清单里**没有**这两个站（只能以「自定义站点」加入，
> 站点名可任意填写），因此插件除了按站点名分派，还多一条**不依赖名字**的
> 判据：**凭据形态**。`fxo_` 前缀的令牌只有 FluxTorrent 架构会签发，
> 所以站点名随便填也不会漏判。

### 2. 在 FluxTorrent 签发令牌

站内【开放 API】页签签发一枚令牌，**scope 必须勾 `crossseed`**。
令牌形如 `fxo_…`。

> **不是 Tracker passkey。** passkey 烤在用户下载过的每一个 .torrent 里，
> 泄露面天然更大；站点也不拿它做账号查询。

### 3. 填插件的令牌框

插件配置里的令牌框格式是 `站点名:凭据`（多个站点用换行分隔）。FluxTorrent
这一行写：

```
FluxTorrent:fxo_你的令牌
```

好学则写：

```
好学:fxo_你的令牌
```

其余站点照旧写各自的 passkey。

### 4. 选下载器、填种子目录、跑

选好 qBittorrent / Transmission，填对应的种子文件目录（逗号分隔，顺序与
下载器列表一致），然后「立即运行一次」或等定时任务。

---

## 自测

```bash
# 单元测试（38 项：19 用例 × 2 个版本）
cd plugins && pytest tests/ -v

# 契约验证（起假站点，插件真实代码打过去，验请求形状/响应解析/302 跟随）
python plugins/tests/test_contract_flux.py
```

契约验证不需要 Docker——它按 FluxTorrent 的响应契约
（`{"code":0,"message":"ok","data":{"results":[…]}}`）复刻站点行为，
跑的是插件的**真实代码**。

---

## 提交到上游（已提交）

本适配是给上游仓库提的补丁，**PR 已开**：

> <https://github.com/jxxghp/MoviePilot-Plugins/pull/1381>
> 分支：`gntv456:feature/fluxseed-site-adapter`（fork：`gntv456/MoviePilot-Plugins`）

改动清单：

| 文件 | 变更 |
|---|---|
| `plugins.v2/crossseed/__init__.py` | 适配本体（v3.0.3 → **v3.0.5**） |
| `plugins/crossseed/__init__.py` | 适配本体（v2.4.1 → **v2.4.3**） |
| `package.json` / `package.v2.json` | `CrossSeed` 条目版本 + `history` |
| `tests/v2/crossseed/test_flux_site.py` | 上游风格回归测试（18 用例） |
| `tests/v1/crossseed/test_flux_site.py` | 同上（v1 代） |

上游 CI 门禁（`.github/workflows/plugin-gate.yml`）会跑：
`check_plugin_versions.py`（版本号必须与源码 `plugin_version` 一致）、
`check_new_plugin_tests.py`（**新增**插件目录必须有测试目录；改已有插件不触发）、
`tests/run.py`（全量插件回归，用 MoviePilot **V3** 后端）。

### V3 后端兼容性（已逐项核对）

上游 CI 的测试跑在 MoviePilot **V3** 后端上，而 V3 已把 `app/utils/` 整个目录
搬走了。本适配用到的路径逐项核对过：

- `app.utils.http` → V3 兼容清单（`app/runtime/compat/manifest.py`）里登记为
  `app.adapters.network.http`，`RequestUtils` 可用；
- `RequestUtils(headers=…)` 的第一参就是 headers，且**传了 headers 就不自动
  加 UA/Content-Type**（本适配因此在 headers 里显式带 `User-Agent`）；
- `get_res(url=…, allow_redirects=True)` 支持跟随重定向——站点的下载口
  `/crossseed/dl/{id}` 只回 302，真实地址在 Location 的 query 里（30 分钟临时
  凭证 `fxk_`），且该端点**只认 query 里的凭证、不认 Authorization 头**，
  不跟随就拿不到 .torrent；
- `app/__init__.py` 在 import 时自动装 legacy import hook，故
  `from app.utils.http import RequestUtils` 在 V3 下成立。

---

## 浏览器扩展侧：PT-depiler 适配

除了 MoviePilot 插件，浏览器扩展 **PT-depiler**（`pt-plugins/PT-depiler`）
也已提 PR 适配本项目与好学新架构站点：

> <https://github.com/pt-plugins/PT-depiler/pull/1684>
> 分支：`gntv456:feature/fluxtorrent-site-adapter`（fork：`gntv456/PT-depiler`）

两侧的分工不同：MoviePilot 插件做的是**辅种**（拿 info_hash / pieces_sha1
去别的站找种），PT-depiler 扩展做的是**搜索 / 下载 / 推送下载器**，所以它走
的是兼容层那组只读 JSON 端点，而不是辅种端点：

| 能力 | 端点 |
|---|---|
| 搜索 | `GET /api/v1/compat/nexusphp/torrents.json` |
| 详情 | `GET /api/v1/compat/nexusphp/torrent/{id}.json` |
| 用户信息 | `GET /api/v1/compat/nexusphp/user.json` |
| 下载 | `GET /api/v1/compat/nexusphp/download.php?id=&apikey=` |

PT-depiler 侧新增了一个 **FluxTorrent 引擎**（`schemas/FluxTorrent.ts`），
并把已有的好学定义从 NexusPHP 切到该引擎（好学迁移后旧的 `.php` 刮削
选择器已全部作废）。鉴权同样是 `fxo_` 开头的开放 API Token
（走 `Authorization: Token <token>` 头），在扩展的站点设置里填。
