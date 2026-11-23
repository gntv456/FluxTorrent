# 甜梦钓场 · 钓鱼玩法（可运行实现）

按 `../钓鱼玩法样图-装备商店与背包扩容.html`（21 个手机画面 + 6 档断点 + 2 张对接表）落地的独立可玩版本。
纯前端、无构建、无依赖，双击即可玩；存档落在浏览器本地。

## 一、怎么运行

- **直接打开**：双击 `index.html`（`file://` 下也能跑，存档自动降级）。
- **本地服务（推荐）**：在 `game/` 目录执行 `python -m http.server 8088`，访问 `http://localhost:8088/`。
  字体走 CDN，服务方式下首次加载会拉取 Noto Serif/Sans SC；离线时自动回落到系统字体。
- 建议窗口：手机 390×844 / 折叠 680×800 / 平板 768×1024 / 桌面 ≥900（自动切双栏，1180 上限）。

## 二、目录分层

```
game/
├─ index.html          应用壳 + 内联 SVG 图标库（无 emoji，避免字体缺失变豆腐块）
├─ css/game.css        设计系统（甜梦蓝-金-紫）、全断点布局、动效与低动效降级
├─ js/data.js          配置数据层：稀有度 / 4 场景（昼·夜·雨三套水体）/ 5 类×4 件装备 /
│                      4 套装 / 38 条鱼 / 30 条成就 / 6 好友 / 8 榜位 / 全部 CONST
├─ js/engine.js        纯逻辑引擎：属性汇总、抽鱼、抛竿、溜鱼、结算、订单、签到、
│                      好友、赛季排行、成就、图鉴分区、存档补全（无 DOM 依赖，可 Node 直跑）
├─ js/store.js         存档层：localStorage → sessionStorage → 内存 三级降级，2s 节流落盘
├─ js/ui.js            界面层：11 个视图 + 10 类弹层（详情对比 / 养成 / 结算 / 图鉴 / 场景 /
│                      规则 / 套装部件 / 好友 / 确认）
├─ js/main.js          主控层：路由、事件委托、状态机、单 rAF 循环、自动垂钓、存档心跳
└─ tools/
   ├─ test-engine.mjs  逻辑自测（node tools/test-engine.mjs）
   └─ frame.html       断点预览壳：?w=390&h=844&q=view%3Dgear
```

## 三、已实现的功能

**装备与场景**
- 6 类装备（场景 / 鱼竿 / 鱼饵 / 鱼钩 / 鱼线 / 鱼漂）各 4 件，魔力购买、即时换装、属性实时重算。
- 装备详情弹层给出「当前 vs 候选」增量对照；购买即装备，原装备自动回背包。
- 单件养成 Lv.1–5，逐级加窗口 / 稀有率 / 收线效率等，单级花费 = 原价 × 0.45 × 当前等级。
- 4 套装（海湾新手、月夜史诗、冰雪凛冬、沧海传说），六件齐备自动激活；可一键装配（自动切到套装所属场景）。
- 4 场景顺序解锁（海湾 → 冰雪峡谷 → 月夜深潭 → 珊瑚礁海），每场景昼 / 夜 / 雨三套水体渐变与鱼池。

**主链路**
- 抛竿 → 等待（气泡提示、按钮禁用）→ 咬钩（环形倒计时 + 水面也可点起竿）→ 起竿（中钩瞬间有脱钩判定）→
  溜鱼（按住收线 / 空格，张力红区前松手泄力，触顶断线、24s 超时）→ 结算弹层。
- 结算含：魔力奖励（已入账）、体型 / 重量、图鉴是否新收录、去向（背包 / 临时鱼缸 / 溢出丢弃）。
- 连竿：传说套 34% 概率额外 1–2 尾；鱼竿 Lv≥4 时 16% 额外 1 尾；多尾走批量结算页。

**背包与鱼缸**
- 背包 30 / 50 / 80 / 120 四档，魔力扩容；容量 = 渔具占用 + 渔获占用。
- 满包自动转入临时鱼缸（容量 = 档位 × 8），缸满则提示丢失。
- 鱼缸支持单条取回、一键取回（受空位限制）、单条放生、一键放生。

**日常与社交**
- 每日 3 单（按日期种子确定性生成，跨日自动刷新），进度随钓获推进；全清可领额外 ✦300。
- 体力：上限 12，6 分钟回 1，✦40 补满；免费抛竿次数（签到 / 赠饵获得）优先于体力。
- 七日签到（魔力 / 体力 / 鱼饵 / 传说饵箱），同日不可重复。
- 好友鱼塘（6 位好友，各自 3 尾展示鱼 + 场景 / 天气），每日赠饵 5 次、可收 6 份（转为免费抛竿）。
- 成就 30 条（钓获 / 稀有 / 场景 / 收集 / 社交等分组），达成后可领魔力。
- 赛季 14 天一轮，排行含自己与 8 位对手，名次与奖励实时计算。
- 自动垂钓（✦1500 买断）：按张力阈值自动收线，6 尾批量结算一次；体力耗尽或包 / 缸双满自动停止。

**系统**
- 规则弹层（概率、窗口、断线、订单、体力、赠饵口径）。
- 存档导出（下载 JSON）/ 重置（二次确认）。
- 全部图标为内联 SVG；无外部图片依赖。

## 四、主链路状态机

```
idle ──cast()──▶ wait ──biteAfter(2.4–5.0s，饵料/阴雨提速)──▶ bite
  ▲                │                                            │
  │                │ 超时未起竿 → settleMiss('miss')              │ hook()
  │                ▼                                            ▼
  └──── settle / break / escape ◀── reelStep ── reel（单 rAF）
```

- 断线：张力 ≥ 上限（初始 100，鱼线决定）；超时：24s；脱钩：中钩瞬间 `max(0, 0.10 − 中钩率 − 脱钩率×0.5)`。
- 溜鱼公式：`rise=(17+河道×4.2)×稀有度力量×0.55×(1−大鱼承受×0.45)`，`decay=15/s`，
  `gain=15.5×收线效率`，软上限 `min(上限−12, 86)`；软上限内收线才涨进度。

## 五、存档

- **站内模式（`FT_API` 存在）**：唯一事实源是服务端 `arcade_fishing_pond`，按登录用户一行
  `state jsonb`；客户端 `loadRemote()` 首帧前拉取，所有动作/抛竿/结算走远程接口，本地只做表现缓存。
- **独立打开 / 离线**：键名 `ft.fishing.save.v1`，`localStorage` 不可用时退 `sessionStorage`，再退内存（`file://` 受限场景）；
  写入 2s 节流，`visibilitychange` / `pagehide` 立即 flush。
- `hydrate()` 对残缺 / 篡改存档逐字段补默认值，不会因坏档白屏。
- 菜单里可导出 JSON 备份；重置会清空魔力、装备、渔获、图鉴、订单、成就（站内模式同时重置服务端存档）。

## 六、验收与自测

```powershell
cd D:\FluxTorrent\design\arcade-sweet-dream\game
node tools/test-engine.mjs      # 87 项断言：数据 → 体力 → 属性 → 套装 → 抽鱼 → 抛竿 →
                                # 溜鱼 → 结算 → 满包入缸 → 购买养成扩容 → 场景链路 →
                                # 订单 / 签到 / 赠饵 / 排行 / 图鉴 → 坏档容错 → 60 次连钓全链路 → 性能
```

浏览器内一键自检（会走真实 DOM 与状态机，最后把结果打在页面左上角）：

```
index.html?selftest=1        # 13 项：抛竿 → 咬钩 → 起竿 → 溜鱼 → 结算 → 图鉴 → 回到待机
index.html?debug=1           # 打印 inner/frame/scrollWidth 等排版数据
index.html?view=gear         # 直接落到指定页（pond/shop/gear/bag/album/orders/stamina/friends/ach/rank/aquarium）
index.html?view=shop&cat=bag # 商店的背包扩容分类
index.html?preview=bite      # 过程态：bite / reel / catch / batch / sheet / grow / scene / rules / friend / setparts
```

快捷键：`空格` 起竿 / 按住收线；`Esc` 关闭弹层。

## 七、性能设计

- 单 `requestAnimationFrame` 驱动溜鱼，`dt` 上限 50ms，掉帧不会“瞬移”导致误判断线。
- 咬钩倒计时同样用 rAF 更新两个节点，不做整页重绘；1s 心跳只更新时钟 / 倒计时 / 体力。
- 视图重绘只在阶段切换、结算、切页时发生；`paint()` 保留滚动位置，避免列表跳动。
- 引擎全部纯函数，10 万次抽鱼 / 属性汇总在 Node 下 < 0.1s（见自测第 19 组）。
- 无第三方依赖、无图片请求；除字体外零网络。

## 八、与主站（apps/web + Rust）对接建议

当前是独立可玩实现，`localStorage` 存档。若要并入主站，建议按下面顺序推进：

1. **接口契约**（REST，均带鉴权与幂等键）
   - `GET /api/v1/fishing/profile` → 完整存档（服务端权威时间、体力、订单、赛季）
   - `POST /api/v1/fishing/cast` → 扣体力 / 免费次数，返回 `biteAfter`、`windowMs`、`fish`（鱼种不下发具体稀有度以外的随机种子）
   - `POST /api/v1/fishing/settle` → 提交 `castId + 溜鱼结果`，服务端复核时长 / 张力曲线采样，发奖
   - `POST /api/v1/fishing/shop/{buy|equip|upgrade|expand}`、`/orders/{claim|bonus}`、`/sign`、`/gifts`、`/rank`
2. **必须服务端裁决**：体力与免费次数、订单种子（按 `date + uid` 派生）、结算发奖、排行与赛季、赠饵上限。
   客户端只做表现，避免改本地时间 / 改存档刷魔力。
3. **存档迁移**：`hydrate()` 的字段即服务端 DTO 形状；`ft.fishing.save.v1` 可直接作为一次性导入体。
4. **复用方式**：已完成 **Shadow DOM 内嵌**（见第九节）：游戏壳、CSS、脚本整包挂进 `apps/web` 的独立路由，
   不复用 `GameShell` / `GameTabs`（那套组件有自己的视觉与状态，混用会与游戏内设计系统打架）；
   `engine.js` 保持纯函数，接后端时可直接由服务端复用同一套规则做复核。
5. **风控**：`settle` 带上服务端下发的一次性 `castId` 与最近 N 帧张力采样，校验“溜鱼时长 ≥ 理论最短、张力不越界”。

## 九、已并入主站（apps/web + Rust API）

站点内入口：**`/games/fishing/pond`**（需登录，`requireModule("games")` 守卫）；
原 `/games/fishing` 押注玩法页顶部有「甜梦钓场 · 完整渔场」入口条，两者并存，不替换服务端结算的老玩法。

### 9.1 前后端分工

| 层 | 位置 | 职责 |
| --- | --- | --- |
| 路由页 | `apps/web/app/(main)/games/fishing/pond/page.tsx` | 鉴权后的页面壳、页头与「独立打开」外链 |
| 挂载器 | `apps/web/components/game/fishing-arcade.tsx` | Shadow DOM 挂载、注入 `FT_ROOT/FT_NS/FT_API`、卸载清理；桌面/手机两档高度 |
| 静态副本 | `apps/web/public/arcade/fishing/` | index.html / css / js（运行时实际加载这份） |
| 服务端 | `apps/api/src/games_http/fishing_pond/` | 存档、掷骰、动作校验、赛季榜（`mod/data/state/rules/http`） |
| 迁移 | `apps/api/migrations/0513_fishing_pond.sql` | `arcade_fishing_pond` + `arcade_fishing_pond_rounds` |

**服务端权威**：抛竿抽到什么鱼、多大、何时咬钩、结算发多少魔力，全部由 Rust 决定并落库；
客户端只负责表现与溜鱼手感。存档在 `POST/GET` 事务里 `SELECT … FOR UPDATE` 串行化，
同一账号多标签页并发也不会互相覆盖。

接口（`/api/v1`，cookie `flux_token` 或 Bearer 均可）：

- `GET /games/fishing/pond` — 拉存档并推进体力/订单/签到区/好友/赛季
- `POST /games/fishing/pond/cast` — 扣体力或免费次数，插一条 round，返鱼种/体型/咬钩延迟/窗口
- `POST /games/fishing/pond/hook` — 按 `bite_at` 与 `window_ms` 判定起竿，窗口外记 late
- `POST /games/fishing/pond/settle` — 按落库结果结算；`land` 必须已起竿，且上报时长与服务器观测的
  起竿→结算间隔同量级（±900ms），防「钩上立刻领奖」
- `POST /games/fishing/pond/act` — 购买/装备/升级/场景/背包/补体力/签到/订单/好友/成就/放生/重置，
  统一按「动作前后 `magic` 差值」写站点魔力流水
- `GET /games/fishing/pond/rank` — 真实玩家按 `state->season->catches` 排名，最多 50 行 + 自己补行

**货币口径（场内即站点魔力，没有第二套币）**：`state.magic` 只是 `users.spark_balance`
的镜像，每次读档/结算/动作都先取真实余额覆盖它，再交给规则层做「买不买得起」的判断：

- 支出（买装备、升级、扩背包、补体力、自动垂钓等）→ 负差额走 `spend_spark_tx`，kind `game`；
- 收入（卖鱼、订单、签到、成就、赛季奖励等）→ 正差额走 `earn_spark_tx`，kind `fishing_pond`；
- 每笔动作/结算各一条流水，幂等键 `pond_settle|pond_act:<uid>:<uuid>`，与渔场存档同一事务；
- `reset` 只清渔场进度，**不会把站点余额重置回默认 3200**；新账号从站点真实余额（通常 0）起步，
  先用每日 3 次免费抛竿渡过开荒。
- 没有任何兑换入口：商店直接以 ✦ 标价，捕获收益直接进站点钱包，站内余额与渔场 HUD 永远一致。

### 9.2 几条必须知道的约束

1. **不用 iframe**：站点 `next.config.ts` 全站 `X-Frame-Options: DENY` + CSP `frame-ancestors 'none'`；
   放宽安全头换 iframe 不划算，故用 Shadow DOM 隔离样式与 DOM。
2. **静态资源必须匿名可达**：`middleware.ts` 的 `isAnonymousOk()` 里加了 `pathname.startsWith("/arcade/")`。
   少了这一条，`/arcade/fishing/index.html`、`game.css`、`js/*.js` 会被 307 成登录页 HTML，
   挂载器拿到的是登录页，脚本静默失败、界面永远停在「正在布置钓场…」。
3. **写接口要过 CSRF**：站点对「带 cookie 的写请求」启用双提交 token（`flux_csrf` cookie +
   `X-CSRF-Token` 头），`store.js` 的 `req()` 会自动读取/申请。另有 Origin 闸，
   本地开发若 Next 跑在 3001，需把 `http://localhost:3001` 加进 `apps/api/.env` 的 `CORS_ORIGINS`
   （默认只有 3000）；生产站点与 `PUBLIC_SITE_URL` 同源则无需额外配置。
4. **Shadow DOM 里没有 `:root`**：`game.css` 的令牌块写成 `:root,:host`，否则 `var(--sky-deep)`
   等全部失效，排行渐变/文字颜色/卡片底色会整片变白。`overlay`/`toast` 也在 `fishing-arcade.tsx`
   里从 `fixed` 改成相对 `:host` 的绝对定位，避免弹层盖住整站。

### 9.3 数据与同步

服务端配置由客户端 `data.js` 生成，单一数据源：

```powershell
node design\arcade-sweet-dream\game\tools\gen-server-data.mjs
```

改完源工程后同步到运行时副本（`design/.../game/` 是源，`public/arcade/fishing/` 是实际加载的副本）：

```powershell
Copy-Item design\arcade-sweet-dream\game\index.html apps\web\public\arcade\fishing\ -Force
Copy-Item design\arcade-sweet-dream\game\css\game.css apps\web\public\arcade\fishing\css\ -Force
Copy-Item design\arcade-sweet-dream\game\js\*.js apps\web\public\arcade\fishing\js\ -Force
```

### 9.4 验收

```powershell
# 客户端规则（87 条）
cd design\arcade-sweet-dream\game; node tools\test-engine.mjs
# 服务端规则（12 条：抽鱼/扣费/结算/连竿/签到/扩容/价格曲线/场景池）
cd D:\FluxTorrent; cargo test -p flux-api fishing_pond
# 前端类型
cd apps\web; pnpm exec tsc --noEmit
```

联调需要同时起 API（8080，cwd `apps/api`）与 Web（3001），并用真实登录态访问
`/games/fishing/pond`；只能验证挂载时可临时加 `flux.session` 标记 cookie，但写接口仍需真实
`flux_token` + CSRF。E2E 冒烟脚本在 `tools/e2e-pond.mjs`（`FT_TOKEN=<flux_token>` 或传 token 文件；Playwright + 本机 Edge），
产出 `_shots/pond-server-1440.png`、`pond-server-reel.png`、`pond-server-settle.png`、
`pond-server-rank.png`、`pond-server-shop.png`、`pond-server-390.png`。

## 十、已知边界

- 无音效 / 无震动反馈（样图未定义，避免自行加戏）。
- 好友鱼塘仍是 `FRIENDS` 配置的模拟好友（每日赠饵/收饵已服务端落库，但没有真实用户关系链）。
- 场内不发行第二套货币：`magic` 就是站点 `spark_balance` 的镜像，收入/支出全部落 `spark_ledger`；
  赛季榜已是真实玩家，但只有玩过本玩法且本赛季有渔获的账号才会上榜。\n- 赛季名次奖励目前只展示额度（✦500/300/200），尚未做跨赛季自动发放；赛季切换时渔获清零。
- 移动端浏览器地址栏会压缩可视高度；嵌入卡片按 `100dvh - 350px`（手机）/ `- 176px`（桌面）留出站点
  顶栏与底部导航，极矮窗口靠内部滚动。

