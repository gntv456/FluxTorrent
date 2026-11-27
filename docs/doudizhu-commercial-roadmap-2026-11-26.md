# 斗地主模块 · 头部商业产品差距对照与排期（2026-11-26）

> 目标：把 FluxTorrent 内置斗地主（doudizhu）从「可上线的中等商业水准」推进到头部商业产品体验。
> 本文档先固化差距清单、依赖方、优先级与验收标准；再按 P0 → P1 → P2 分阶段开发。
> 所有结论以当前仓库源码与后端契约为准，不引入后端尚无字段/端点的「造假 UI」。

---

## 0. 基线现状（本轮排期起点）

### 0.1 技术与运行环境

| 项 | 值 |
|---|---|
| 前端 | Next 15.3.8 / React 19 / pnpm，`apps/web`，包名 `@fluxtorrent/web` |
| 后端 | Rust / actix-web，`apps/api`，package `flux-api` |
| 斗地主前端 | `apps/web/app/(main)/games/doudizhu/`（容器 `_room.tsx`，部件在 `_parts/`） |
| 唯一样式表 | `apps/web/app/styles/arcade-doudizhu.css`（作用域 `.doudizhu-page`） |
| 素材 | `apps/web/public/games/ddz/`（头像 / 桌布 / 牌背 / 皇冠 webp） |
| 后端规则/状态机 | `apps/api/src/games/doudizhu.rs`、`doudizhu_game.rs`、`doudizhu_settle.rs` |
| 读侧接口 | `games_http/doudizhu_http.rs`、`doudizhu_pvp_http.rs` |
| 本机服务 | web `3000`、api `8080`、postgres `5432`、redis `6379` 均在监听 |

### 0.2 已完成能力（基线）

- 牌型前端识别器 `combo.ts`：单张 / 对子 / 三张 / 三带一 / 三带二 / 顺子 / 连对 / 飞机 / 炸弹 / 王炸 / 四带二，合法/非法按钮反馈。
- AI 对局状态机经 API 探针联调通过；classic / wild 癞子两变体。
- 叫分 1/2/3、底牌、记牌器、快捷键、三语文案、回放折叠视图。
- 徽章光晕、选牌红边反馈、低张数报牌、剩余张数、倒计时、合成音效 cue（Web Audio，默认静音）。
- 三张角色头像与墨绿桌布已重绘统一为同套 2.5D Q 版风格；旧素材备份在 `_scratch/ddz-assets/old/`。
- 基线质量门：`tsc --noEmit` 0 错误；combo 单测 29 PASS；fixture 十一视图无页面级溢出。

### 0.3 评分口径

当前核心玩法界面约 **6.5–7 / 10**：框架完整、信息层级清楚、不廉价；与头部产品（欢乐斗地主量级）仍差一档，集中在美术厚度、动效演出、结算/社交包装、场景氛围，以及规则扩展与后端配套。

---

## 1. 差距矩阵（十维度）

依赖方标记：`前端` 可独立完成 · `素材` 需补美术资产 · `后端` 需先补字段/端点 · `规则` 需先定玩法与经济口径。
优先级：**P0** 不卡后端、本轮可推进；**P1** 需后端先行；**P2** 需先定产品规则与经济口径。

### 1.1 玩法规则层

| 头部已有 | 我方现状 | 依赖 | 优先级 | 验收标准 |
|---|---|---|---|---|
| 抢地主：叫分后轮流可抢，每抢一次 ×2 | 仅叫分 1/2/3，无抢地主环节 | 规则 + 后端 | P2 | 状态机含抢地主动作；倍数按抢次翻倍；前端叫分/抢分按钮联动；单测覆盖抢分倍数 |
| 明牌：开始明牌 ×5 / 发牌明牌 ×3–4 / 收牌明牌 ×2 | 无明牌选项 | 规则 + 后端 | P2 | 明牌阶段可操作；倍数档位落库；明牌状态在座位与底牌可见 |
| 加倍 / 超级加倍：地主出牌前翻倍 | 无加倍操作 | 规则 + 后端 | P2 | 加倍动作端点；倍率链计入结算；前端加倍按钮与禁用态正确 |
| 弃三张：地主后弃 0–3 张，弃牌牌面影响倍数 | 无弃牌阶段 | 规则 + 后端 + 前端选牌 | P2 | 弃牌选牌 UI；后端校验张数与合法性；结算口径一致 |
| 不洗牌玩法（起手牌更整） | 仅 classic / wild | 规则 + 后端 | P2 | 变体入口；发牌概率策略落库；前端模式卡片标注 |
| 残局闯关 / 每日挑战（PVE 单机） | 无单人闯关内容 | 规则 + 后端 + 前端 | P2 | 关卡配置端点；进度记录；闯关入口与失败/通过反馈 |

现有结算口径（后端事实，前端分解须对齐）：
倍数 = 叫分底(1–3) × 2^炸弹数（王炸按两颗炸弹计，封顶左移 3 次）× 春天×2，**封顶 8 倍**；
派彩：赢家回款 = 本金 `stake` + `stake × share × multiplier × (1 − rake)`；share 农民 = 1、地主 = 2；输局净 = `−stake`。

### 1.2 匹配与场次层

| 头部已有 | 我方现状 | 依赖 | 优先级 | 验收标准 |
|---|---|---|---|---|
| 分场准入：新手/初级/普通/中级/大师场，按金币、段位准入，卡上标底分、准入区间、在线人数、加成 | 三档底分 100/500/2000，无门槛、无人气标签 | 后端 + 规则 | P1 | 场次含准入区间与底分；不满足门槛置灰并提示；在线人数由后端下发 |
| 智能匹配：按段位/胜率匹近水平对手 | AI 练习 + 基础 PvP 房间，无匹配分层 | 后端 | P1 | 匹配请求按段位/胜率路由；匹配等待态有明确反馈 |
| 好友房 / 锦标赛 / 定时赛事 / 排行榜 | 均无 | 后端 + 规则 | P2 | 好友开房码；赛事定时编排；排行榜端点与前端榜单 |

### 1.3 段位与称号体系

| 头部已有 | 我方现状 | 依赖 | 优先级 | 验收标准 |
|---|---|---|---|---|
| 大段位（新手→斗神）星星升降、数十级头衔勋章、赛季结算奖励 | 无段位/称号/勋章/赛季/排行 | 后端 + 素材 | P1 | 段位存储与升降星规则；局内称号展示；段位徽章成套素材 |
| 赛季奖励与排行 | 无 | 后端 + 前端 | P1 | 赛季周期结算；奖励发放；榜单页 |

### 1.4 角色与装扮经济

| 头部已有 | 我方现状 | 依赖 | 优先级 | 验收标准 |
|---|---|---|---|---|
| 角色商城、角色等级与技能 | 三张固定头像，无成长 | 产品 + 后端 + 素材 | P2 | 角色条目/等级；解锁状态；已装备态回显 |
| 头像框、聊天气泡、互动表情、专属语音/闹钟 | 无 | 产品 + 后端 + 素材 | P2 | 装扮槽位；库存与装备端点；预览面板 |
| 牌桌皮肤、牌面皮肤、出牌特效道具 | 固定桌布/牌背/皇冠 | 产品 + 素材 + 前端 | P2 | 皮肤切换落库；桌布/牌背/牌面可换；预览一致 |

### 1.5 社交互动

| 头部已有 | 我方现状 | 依赖 | 优先级 | 验收标准 |
|---|---|---|---|---|
| 实时语音、快捷消息/表情、组队开黑、送礼、好友 | 无聊天/表情/语音/好友/送礼 | 后端 + 素材 | P2 | 消息频道端点；表情贴纸；快捷消息条；送礼流水 |
| 托管（AI 代打）、在线状态点、PvP 准备状态、断线重连 | 后端 Seat/GameState 无对应字段与动作端点，前端未造假 UI | 后端 | P1 | Seat 增在线/准备字段；托管动作；断线重连恢复手牌与桌面状态 |

### 1.6 任务与活动经济

| 头部已有 | 我方现状 | 依赖 | 优先级 | 验收标准 |
|---|---|---|---|---|
| 每日签到、日常任务、任务宝箱、限时活动、转盘抽奖 | 仅对局结算魔力余额 | 后端 + 前端 + 素材 | P2 | 任务定义与进度端点；签到发放；活动入口与面板 |
| 商城货币（金币/钻石/体力）、VIP 特权 | 无多货币与体力 | 产品 + 后端 | P2 | 货币账本；体力恢复规则；商品与特权配置 |

### 1.7 对局演出与音视频

| 头部已有 | 我方现状 | 依赖 | 优先级 | 验收标准 |
|---|---|---|---|---|
| 出牌飞牌轨迹、底牌翻开、加倍动画、炸弹/王炸爆裂火光、胜利礼花与震屏、角色待机动作与表情 | 基础脉冲 + 12 粒下落粒子；无飞牌轨迹、无场景图、无角色动作 | 前端 + 素材 | P0/P1 | 出牌有落点轨迹；炸弹/王炸分层光晕与粒子；底牌翻转动画；胜利礼花 |
| BGM、出牌/胜负语音播报、场景化背景 | Web Audio 合成 cue 且默认静音；无 BGM/语音/场景 | 素材 + 前端 | P1 | 场景背景挂接；BGM/语音走可开关的合成或授权资产；默认静音口径不变 |

### 1.8 结算与战绩

| 头部已有 | 我方现状 | 依赖 | 优先级 | 验收标准 |
|---|---|---|---|---|
| 倍率分解明细（底分/炸弹/王炸/春天/加倍逐项列出） | 净赢 + 春天标签 + 折叠回放；本轮已补底分/倍数/炸弹/身份倍率分解徽章 | 前端（本轮 P0） | P0 | 结算面板展示倍数链分解徽章，口径与后端 `settle_payout` 一致 |
| 经验/段位星级、胜负评价、回放分享 | 无成长反馈、无分享 | 后端 + 前端 | P1 | 战绩持久化；回放可分享链接；评价标签 |
| 对局历史列表、对手胜率数据 | 无历史战绩列表 | 后端 | P1 | 历史局列表端点；分页与空态；对手数据可见 |

### 1.9 工程与对局保障

| 头部已有 | 我方现状 | 依赖 | 优先级 | 验收标准 |
|---|---|---|---|---|
| 断线重连、托管续局 | 缺失 | 后端 | P1 | 重连后恢复完整桌面；托管续跑至终局 |
| 防作弊/校验提示 | 无展示层 | 后端 + 前端 | P1 | 关键操作校验结果可见；异常对局提示 |
| 设置项（画质/音量/护眼） | 音效开关全局已具备（默认静音）；动效降级跟随系统 reduced-motion；独立画质/护眼/动效持久化开关待补 | 前端 | P0/P1 | 音量开关可持久化；动效降级；画质/护眼开关列入 P1 |
| 实名防沉迷、多端互通 | 无合规口径 | 产品 | P2 | 接入站点既有合规体系；数据跨端一致 |

### 1.10 美术资产厚度

| 头部已有 | 我方现状 | 依赖 | 优先级 | 验收标准 |
|---|---|---|---|---|
| 场景背景（厅堂/夜景）、木/金桌沿包边、底部筹码条、头像框装饰 | 头像/桌布刚重绘；桌内环境与装饰件偏少 | 素材 + 前端 | P0 | 桌沿/聚光/筹码条 CSS 成型；场景背景可切换 |
| 角色全身立绘与待机动作 | 仅胸像头像 | 素材 | P1 | 全身立绘成套；待机帧/动作接入 |
| J/Q/K/A 花牌人物位图、精致大小王造型 | 人头牌为花色阵列；王牌角标「大/小」 | 素材 | P1 | 花牌位图成套且牌面一致；王牌造型精致化 |
| 成套 ICON 与特效边框 | 基础图标沿用全站 emoji 惯例（大厅注册入口保留） | 素材 | P1 | 对局内符号纯 CSS/矢量；成套特效边框 |

---

## 2. 分阶段排期

### 2.1 P0 — 前端 + 素材即可推进，不卡后端（本轮）

| 编号 | 事项 | 依赖 | 落点 | 状态 |
|---|---|---|---|---|
| P0-1 | 结算倍率分解明细条（底分 / 倍数 / 炸弹 / 身份倍率） | 前端 | `settle.tsx` + 三语字典 + CSS | ✅ 本轮已落地 |
| P0-2 | 牌桌包边、中央聚光、底部筹码/倍数条 | 前端 + CSS | `table.tsx` / `fullgame.tsx` + CSS | ✅ 筹码片已落地；包边/聚光前轮已成型 |
| P0-3 | 炸弹 / 王炸分层光晕与粒子强化 | 前端 + CSS | 桌面 fx 层 + CSS | ✅ 双层径向光晕 + 回弹扩散动画 |
| P0-4 | 音量 / 动效开关（默认静音口径不变） | 前端 | 全局 `useSfxToggle` + 媒体查询 | ✅ 音效开关已具备（`localStorage.arcade_sfx`，默认关）；动效降级跟随系统 `prefers-reduced-motion`；独立动效持久化开关列入 P1 可选 |
| P0-5 | 全量基线复验：tsc 0 错误、combo 单测全过、fixture 十一视图无溢出 | — | 复跑命令见 §4 | ✅ 本轮已复验通过 |

> 本轮 P0 落地范围仅限不卡后端、不造假数据的前端 + CSS 项；P1/P2 需后端字段/端点或产品规则先行，见 §2.2 / §2.3。
> ~~浏览器真实登录态页面视觉复核（任务 58）受 BrowserUse 窗口绑定超时阻塞~~ → **✅ 2026-11-26 已闭环**：BrowserUse 窗口绑定恢复，手铸 JWT（iat/nbf 回拨 60s）双 cookie 登录态复核完成——大厅第三变体「不洗牌」/ 明牌勾选 /「更多服务」十入口 + 三项「待产品定稿」灰显全部在线；实开一局（game 1235）走完 明牌→叫3→AI 抢地主→AI 弃三张→加倍×4→出牌→地主胜 结算全链，DB `arcade_doudizhu_games` 落账 `multiplier=8`（明牌×2 × 抢×2 × 加倍×4 = 16 封顶 8）、净收 -100 与余额 19,842→19,742 对平。复核中发现并处置环境隐患：本地 web/api 两服务均为旧进程（旧产物锁 `.next` 致 rebuild EPERM、API 不识 `reveal` 字段），已停旧重建重启（`pnpm build` EXIT=0、`cargo build` 0 error），3000/8080 现挂全量新构建。

### 2.2 P1 — 需后端先补字段与端点

| 编号 | 事项 | 后端前置 |
|---|---|---|
| P1-1 | 托管 / 断线重连 / 在线状态 / PvP 准备状态 | Seat/GameState 增字段与动作端点 |
| P1-2 | 匹配分层与场次准入、在线人数 | 房间/匹配模型与准入规则 |
| P1-3 | 战绩历史列表、回放分享、对手数据 | 战绩持久化与分享端点 |
| P1-4 | 段位称号、星级升降、赛季奖励、排行 | 段位存储与结算规则 |
| P1-5 | 场景背景、角色全身立绘、花牌位图、成套 ICON | 素材先行，前端挂接 |
| P1-6 | BGM / 语音播报（可开关，默认静音） | 授权资产或合成方案 |

### 2.3 P2 — 需先定产品规则与经济口径

| 编号 | 事项 | 产品前置 |
|---|---|---|
| P2-1 | 抢地主 / 明牌 / 加倍 / 弃三张 / 不洗牌规则扩展 | 玩法规则与倍数口径定稿 |
| P2-2 | 商城装扮（角色、头像框、气泡、表情、桌布/牌面皮肤） | 经济与道具口径定稿 |
| P2-3 | 任务、签到、活动、转盘、多货币与体力 | 活动与货币账本规则 |
| P2-4 | 好友房、赛事、排行榜 | 赛事编排与关系链 |
| P2-5 | 语音 / 表情 / 组队 / 送礼社交 | 消息频道与关系链 |
| P2-6 | 防沉迷合规与多端互通 | 站点合规体系对接 |

### 2.4 本轮落地状态回填（2026-11-26）

> 口径：后端无字段/端点的能力前端不造假 UI；经济红线静态模型 EV<1 只回收不增发；段位只读派生不建表不发货币；零音频资产（Web Audio 合成 cue，默认静音）。

**P1 已落地**

| 编号 | 落地情况 |
|---|---|
| P1-1 | ✅ 在线状态（30s 在线窗口 `last_seen`）、PvP 准备状态、心跳/托管续跑已落地；断线重连恢复完整桌面仍待补 |
| P1-3 | ✅ 对局历史复用通用 `GameRecordsPanel`（`/games/rounds` ref_type=`doudizhu`）；❌ 回放分享链接、对手胜率数据待后端 |
| P1-4 | ✅ 段位从已结算局实时派生（七档 tier 0/1/5/15/30/60/100 胜场，只读端点 `GET /games/doudizhu/rank`，不建表不发货币）；❌ 赛季奖励发放、排行榜榜单待后端 |
| P1-2 | ❌ 匹配分层、场次准入门槛、在线人数：无后端模型，前端未造假置灰 |
| P1-5 | 🟡 已落地：① 夜景场景背景 `ddz-bg-night.webp`（1920×1080，38KB，深墨绿暗金中式飞檐/灯笼/烟花，中央下部压暗留桌，挂页面底层、渐变兜底、浅色档不挂）；② 大小王徽章 `ddz-joker-big.webp`（金橙双龙山冠日轮）/ `ddz-joker-small.webp`（紫银月轮星芒祥云），256 透明 webp；③ J/Q/K 花牌人物成套 12 张 `ddz-face-<j|q|k>-<spade|heart|diamond|club>.webp`（中式武将/仕女半写实 2.5D，256×340 透明 webp，19.8–30.6KB，按花色配色），牌面中央叠人像、花色降级，A 与数字牌仍走花色阵列；④ 角色全身立绘成套 3 张 `ddz-full-<landlord|farmer|ai>.webp`（256×340 透明 webp，沿用胸像身份母版，农民/AI 经专用去底工具抠除黑底），按我方座位身份挂结算面板代表站位，失败态降饱和提亮；⑤ 成套金边图标 5 张 `ddz-icon-<stake|mult|bomb|rocket|spring>.webp`（64×64 透明 webp，Q 版 2.5D），接入桌内底分/倍数/炸弹状态、落牌炸弹/王炸徽章、春天胶囊与结算明细胶囊；特效边框沿用既有 CSS 分层光晕与金边胶囊，未另造位图边框。⚠️ 浅色主题档下人像/图标的融合观感尚未专门复验 |
| P1-6 | 🟡 合成短音已补：P2-1 新阶段专属 cue（抢地主 `grab` / 弃三张 `discard` / 加倍 `double`），PvP 准备与明牌勾选复用既有 `tick`/`select`；仍 ❌ 真人台词播报与 BGM：零音频口径阻塞，须先取得可商用、可再分发的授权素材或自营录制方案，禁止抓取第三方音效板的商业游戏原声 |

**本轮 HUD / 控件补齐（对照第二张在运营参考图，2026-11-26 第二批）**

| 项 | 落地情况 | 阻塞 / 未造假 |
|---|---|---|
| 顶部 HUD 左右分区 | ✅ 全屏壳顶栏：左区房间名 + 局号 + 状态胶囊（`playing` 绿「对局进行中」/ 等待灰）；右区底牌迷你三张 + 底分 + 倍数 + 炸弹 + 音效开关 + 全屏 + 余额金点 + 退出房间红钮；非全屏桌中央状态行保留参数，全屏壳内不重复 | — |
| 座位标识 | ✅ 头像右上剩牌数角标（地主位金底）；落牌方为地主时牌型徽章旁挂红色「地主」签；任一座位手牌出完（`count===0`）中央出播报条 | — |
| 记牌器形态 | ✅ 由 8 列数字网格改为底部横向胶囊点数条（点数 + 剩余张数并排），宽屏一行、窄屏两行自然换行 | — |
| 系统控件 | ✅ 音效开关复用 `useSfxToggle`（localStorage 持久化、开通即 tick）；全屏 `requestFullscreen/exitFullscreen` + `fullscreenchange` 监听回写；退出房间红边危险态 hover 反白 | — |
| 三语 | ✅ 新增 `soundOn / soundOff / fullscreen / exitRoom / outDone` 五键，zh-CN / zh-TW / en 同步 | — |
| 观战人数 `0/20` | ✅ 后端底座：`arcade_doudizhu_spectators`（一人一房一行，不占座不扣底分，退房级联）；端点 `POST /pvp/spectate`（已是座位玩家拒绝、房须 waiting/playing）、`POST /pvp/spectate/leave`；房间视图带 `spectators` 计数；前端 RoomWait 只读渲染观战数。✅ **本轮打通观战身份读局回显**：无本人活跃局时回退 `spectate_room_db` 定位观战房 → `load_room_game_db` 取权威活跃镜像行，`spectator_mask` 清三座 cards/wildcards、`is_me=false`、`my_turn=false`，响应带 `spectating:true`；`/state` 与 `/pvp/state` 两端点同口径回退；公开列表放宽到 `waiting/playing` 并回带 `status`；前端大厅房间行「观战」入口 + 全屏只读旁观牌桌（隐手牌/记牌器/操作区/本人余额，旁观标签，4s 轮询，退出走 `spectate/leave`） | 观战上限 20 人的风控口径待定稿；waiting 未开局房的大厅态「旁观中」标记与退出入口未专门分支；观战回显依赖 `/pvp/state` room.seats 对齐昵称，缺失时回退立绘与 AI 名 |
| 私房 / 公开标签 | ✅ 房间加 `visibility`（`public`/`private`，CHECK 约束）；公开匹配列表 `list_waiting` 与随机撮合只扫 public；join body `private` 建私房；前端等待视图渲染「私密」标签 | `join_code` 口令字段预留未启用、HTTP 不校验；凭房号入座端点与口令验真待产品口径定稿后再开 |
| 座位昵称 / 座位各自余额 | ✅ 座位视图 LEFT JOIN users 取 `username`/`avatar_url`/`title`/`spark_balance`；余额仅本人座下发、他人座为 null（不亮筹码）；等待房渲染昵称、头像（avatar_url 优先、立绘兜底）、称号、己方余额；对局中牌桌座位卡经 `seatMeta` 按座位号对齐 PvP 昵称/头像/称号，AI 房回退既有立绘与 AI 名 | 对局中座位余额仍不亮他人筹码；观战者资料卡待旁观视图落地后再挂 |
| 聊天消息频道 | ✅ 后端底座：`arcade_doudizhu_room_messages`（按 `(room_id, created_at DESC)` 索引）；`GET /pvp/messages`（最近 50 条、时间升序、昵称回填）、`POST /pvp/messages`（仅座位或当前观战者可发；`users.status=1` 禁言拦截；正文 trim 后截断 120 字、空拒）；前端 `PvpMessage` 类型 + `RoomChat` 组件已接等待房：消息列表（自动滚底）、输入框、回车/按钮发送、5s 轮询刷新、前端先截 120 字 | 表情贴纸、快捷消息条、敏感词词库、对局中牌桌内嵌频道待产品定稿；禁言本人状态前端未预取（发送失败静默保留草稿） |
| 对局中「查看结算」回看 | ✅ 复用既有 `state.last` 终局快照（`StateResp.last: DzGame | null`），本轮未新增独立回放端点 | ❌ 对局中「查看结算」入口按钮待前端挂接；回放分享链接、牌局录像落库仍待后端 |

**后端社交底座落地（2026-11-26 第三批，Rust actix + Next15/React19）**

- 迁移 `0526_doudizhu_social.sql`：`arcade_doudizhu_rooms` 加 `visibility`（public/private，CHECK）与预留 `join_code`；新建 `arcade_doudizhu_spectators`（PK room+user，双索引，CASCADE）、`arcade_doudizhu_room_messages`（identity 主键，`(room_id, created_at DESC)` 索引，CASCADE）。
- 读侧 `doudizhu_pvp_view.rs`：座位 LEFT JOIN users 取昵称/头像/称号/余额；房间带 `visibility`/`spectators`；`list_waiting` 只扫 public。写侧 `doudizhu_pvp_repo.rs`：`create_room_tx`/`join_or_create_tx` 签名加 `visibility`；新增 `is_seat_db`、`spectate_tx`（upsert）、`leave_spectate_db`、`spectate_room_db`、`post_message_tx`、`list_messages_db`（库内倒序取 N 条后反转为升序）；`MSG_BODY_MAX=120`。
- HTTP 端点（均过 `require_auth` + `check_minclass("doudizhu")`）：`POST /pvp/spectate`、`POST /pvp/spectate/leave`、`GET /pvp/messages`、`POST /pvp/messages`；join body `private` 映射 visibility；发言前自查 `users.status`（禁言 1 拦截，封禁 2 已由鉴权层拦）。
- 复验：后端 `cargo check` 0 error；非 ignored 单元 `cargo test doudizhu` **71 passed / 0 failed**；新增两个 DB 级测试 `pvp_social_spectate_messages_and_profile`、`pvp_visibility_list_filter` 本地连库通过；前端 `tsc --noEmit` 0 错误、combo 全 PASS、fixture 十视图无溢出。
- 前端第四批挂接：`RoomChat`（`_parts/room-chat.tsx`）接入等待房，消息列表 + 输入框 + 5s 轮询 + 回车发送 + 前端截 120 字；牌桌 `DzTable` 新增可选 `seatMeta`，由 `_room` 从 `pvpRooms.mine.seats` 映射昵称/头像/称号经 `FullGame` 透传，AI 房无 meta 回退立绘与 AI 名；三语补 `chatTitle / chatPlaceholder / chatSend / chatEmpty / spectators / privateTag / myBalance`。复验 tsc 0 错误、combo 29 项全 PASS、桌面/移动等待房 fixture 无横向纵向溢出。
- ✅ 本轮（观战读局 + 旁观视图）：repo 新增 `load_room_game_db(db, rid)`（按 room_id 取最早一条 `bidding/playing` 活跃镜像行，只读不持行锁，复用 `decode_row`）；抽取共享 `spectator_mask(v)` 清三座手牌/癞子并置 `is_me=false`、`my_turn=false`，`doudizhu_http.rs` 与 `doudizhu_pvp_http.rs` 复用同一函数；两个 state 端点在 `load_active(uid)` 为 None 时回退观战房取局回显，game 套 mask 并带 `spectating:true`；`list_waiting` SQL 放宽到 `status IN ('waiting','playing')`，rooms 项补 `status`。新增 DB 测试 `pvp_spectator_reads_room_game`（第四人观战：断言无本人活跃行、按观战房能取到权威活跃局、非座位）本地连库通过。前端：`DzGame`/`StateResp`/`PvpRoom` 类型补 `spectating`/`status`；`PvpLobby` 新增房间列表与 `onSpectate`；主件 `spectate(rid)`/`leaveWatch()` 走统一管线（`/pvp/spectate` → `/pvp/state` 存 specRoom → `loadState` → `loadRooms`），旁观 4s 轮询；`FullGame`/`DzTable` 传 `spectating`，旁观态隐手牌、记牌器、控件、本人余额，顶栏旁观标签、退出文案切 `leaveWatch`；三语补 `watchBtn/watchingTag/leaveWatch/roomListEmpty`；CSS 补 `.dz-room-list/.dz-room-row(+info/title/meta)` 与 `.is-watch` 标签。复验：tsc 0 错误、combo 0 FAIL、fixture 既有视图无溢出；后端非 ignored 71 passed、ignored 11 passed（唯一 FAILED 为 #102 mirror 坐标系缺口）。
- ⚠️ 遗留 DB 测试（本轮复查后）：原 3 个 ignored 未达标项中，`full_game_settles_with_ledger`、`full_wild_game_settles` 已修复转绿——根因是测试驱动只分 `Bidding/play`，未适配 P2-1 新增的 Grab/Discard/Double 阶段，轮到玩家处于这三段时误发 `play` 被状态机拒、提前退出；已按全阶段分发动作（grab 不抢 / discard 空弃 / double 不倍 / play 过牌），本地连库 10 passed。仅剩 `pvp_mirror_events_fan_out` 未达标，根因已定位：`discard()` 末把 Double 起始 `turn` 写死行内 0，而镜像三行行内 0 指向不同物理玩家；轮转事件 seat 经轮换变换后与 peer 行内 turn 指针脱节，加倍三票无法在镜像行凑齐，peer 停在 Double。bid/grab/discard/play 链路已同步，仅剩加倍（及抢地主多票轮转）需在状态机/重放层统一物理座位坐标后补齐，单独排期。
- 口径默认（澄清超时未答，按推荐落地，若改口需回滚）：余额仅己方座位可见；私密房本轮只做标签与公开列表过滤、不启用口令；六项本轮全做（表 + repo + HTTP + 单测，前端类型与只读视图同步）。

**P2-1 玩法规则：✅ 五项一次性落地**

- 状态机阶段：`Bidding → Grab(抢地主) → Discard(弃三张) → Double(加倍) → Playing → Done`；旧 jsonb 快照 serde default 兼容。
- 抢地主：候选本人不表态，另两家各一票；抢成功换候选并 `grabs+1`；倍数按抢次翻倍（×2^grabs，封顶 2 次）。
- 明牌：开局选择，倍数 ×2；AI 房勾选，PvP 镜像统一 false。
- 加倍：出牌前三票轮转，档 0/1/2（×1 / ×2 / ×4），取三座最高档计入倍数链。
- 弃三张：地主落定后弃 0–3 张弱牌，后端校验张数与手牌合法性。
- 不洗牌：`nolook` 变体按 rank 聚类排序后切 17/17/17+3，起手更整；classic / wild / nolook 三变体。
- 倍数链 `multiplier_full`：叫分底 ×2^炸弹（王炸双弹）×春天 ×明牌2 ×抢地主2^grabs ×加倍档，整体仍封顶 8；不破 EV<1 回收口径。
- 后端：状态机 / AI 三段循环 / 托管 match 全阶段 / PvP 镜像重放 / HTTP 动作白名单 `bid|grab|discard|double|play|hint` / 视图新字段；单测覆盖抢候选计数、弃牌限三张、加倍进 Playing、nolook 守恒 54 张、倍数叠乘封顶。`cargo test doudizhu` 71 passed / 0 failed / 9 ignored。
- 前端：变体第三入口、明牌勾选、抢/弃/加倍三段控件与回调链路、阶段状态行标题、三语字典、CSS；tsc 0 错误。

**P2-2~6 最小骨架（只接全站已有底座，不增发货币、不造伪数据）**

| 编号 | 落地情况 | 阻塞 / 待定稿 |
|---|---|---|
| P2-2 装扮商城 | ✅ 大厅入口跳转全站底座：`/shop` 魔力商城、`/dressup` 角色装扮、`/avatar-frames` 头像框 | 斗地主专属角色技能、桌布/牌面皮肤切换落库、库存装备端点：经济与道具口径定稿后再建 |
| P2-3 任务签到活动 | ✅ 入口跳转：`/checkin` 签到、`/tasks` 任务中心、`/seasonal` 限时活动 | 斗地主专属任务定义与进度、多货币/体力账本：活动与货币规则定稿后再建 |
| P2-4 好友房 / 赛事 / 排行 | ✅ 入口跳转：`/friends` 好友、`/contests` 大赛报名、`/top` 通用排行榜 | 斗地主好友开房码、定时赛事编排：无编排底座，阻塞待产品定稿 |
| P2-5 语音 / 表情 / 社交 | ✅ 入口跳转：`/messages` 消息中心 | 实时语音频道、表情贴纸、送礼流水、组队开黑：无消息语音/关系链动作底座，阻塞待产品定稿 |
| P2-6 防沉迷 / 多端 | ❌ 无入口、不造假 | 实名防沉迷强制口径、跨端互通：站点合规体系对接后落地，阻塞待产品定稿 |

**本轮复验基线**：前端 `tsc --noEmit` 0 错误；combo 单测 29 PASS；fixture 十一视图无页面级溢出（手牌横滚不计）；后端 `cargo test doudizhu` 71 passed、`cargo build` 0 error（仅既有 warning）。浏览器真实登录态页面视觉复核（任务 58）已于 2026-11-26 下午闭环（见 §2.1 末注）：登录态大厅新件核对通过 + 实局全链结算对平（game 1235，multiplier=8 落账）。

---

## 3. 约束与不可造假清单

- **后端无字段/端点的能力，前端不造假 UI**：托管、在线状态点、PvP 准备状态、断线重连、战绩历史、段位排行、在线人数、赛事榜单——在后端落地前，界面不出现伪数据。
- **零音频资产口径**：不引入 mp3/wav；音效走 Web Audio 合成，默认静音（`localStorage arcade_sfx` 缺省 off）；BGM/语音须先定授权或合成方案。
- **版权边界**：角色立绘、桌布纹样只借鉴通用布局/层级/配色/反馈范式，不复制受版权保护的人物造型、桌布纹样、logo、专有美术与音频；参考仓库 `nexusphp-doudizhu`（GPL-2.0-only）不引入代码与位图。
- **既有取舍**：记牌器浮层展开会遮挡手牌中段，属既定交互取舍；手牌移动端 `handScroll > handClient` 为横滚设计，不算页面级溢出；大厅注册入口图标沿用全站 emoji 惯例，保留不动。

---

## 4. 复跑方式（pwsh 7）

```powershell
# 类型检查（0 错误）
cd "D:\FluxTorrent\apps\web"; node node_modules/typescript/bin/tsc --noEmit

# combo 单测（全过；改 combo.ts 后需先编译到 combo-build/combo.js）
node "D:\FluxTorrent\_scratch\dz-shots\combo-test.mjs"

# fixture 十一视图截图（chromium channel msedge，deviceScaleFactor 2）
node "D:\FluxTorrent\_scratch\dz-shots\shoot.mjs"

# 后端编译
cd "D:\FluxTorrent\apps\api"; cargo build --message-format short
```

页面级溢出判定：`tableScroll/topScroll ≤ client`；手牌横滚不算页面溢出。
