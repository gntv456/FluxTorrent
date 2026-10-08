# PT tracker 五轮深挖实测审计（2026-10-08）

> **尺子**（与 10-07 报告同一把，不重复论证）：本项目是**私募站 tracker**（passkey 鉴权、
> 按 announce 差值计流量、承担反作弊与版主治理），所以一律对
> **UNIT3D / NexusPHP / Ocelot(Gazelle) 的私募站惯例**评，不对公网 tracker 功能清单评。
> 本轮另外做了一件事：**把引用的主流口径逐条拉源码核实**（见 §八），
> 因为上一轮我自己就复述过两条讹传（opentracker 有 passkey、chihaya 有限速中间件）。
> 核实结果既纠正了外部讹传，也**纠正了我本轮自己准备落地的一个判据**（§八-6）。
>
> **本轮定位**：10-07 那份报告的四批修复（批一～批四，已提交已部署已线上复验）
> 是本轮的**基线**，不是本轮的对象。本轮查的是「修完之后还剩什么」以及
> 「修的过程有没有把别的东西弄坏」——结论：**修 P0 的那一批把全站流量入账弄停了**，
> 而它当时的验收探针 22/22 全绿。

---

## 一、结论摘要

| 级别 | 条数 | 一句话 |
|---|---|---|
| **P0** | 2 | ① 批一的锚点判据把「正常单调上报」也落进钉死分支 ⇒ **全站流量入账恒 0**，且连带把所有真做种者打成 `ghost_seed`（实测 L2 告警 hits=70）；② 交叉佐证（xreport）是**没有上界的自报量**，`upload_corroborated` 成了铸币机，而 10-08 的辅种豁免判据是 `> 0` ⇒ **另一个账号发一条 `xreport=<你的 peer_id>:1` 就把幽灵做种与 completed 两条不变量一起作废** |
| **P1** | 4 | 速率钳的秒窗没有上界（announce 越稀疏单笔越能吃）；近零重置被 `GREATEST` 钉回高位（真重启的客户端第二次 announce 即被判作弊且流量永久冻结）；**UDP 两个读口完全绕过准入 gate**（假 left 照收、待审种子的 peer 照样外发）；scrape 的待审档位判据写反（**档位越严，scrape 越漏**，owner_only 下全量外泄） |
| **P2** | 7 | 作弊处置链三处断口（含 `agent = 'torrent:%'` 把 LIKE 写成等值）；外置存储模式下 HTTP 的 `conn` 读内存表（探测结果写在 Redis）⇒ 多副本下幽灵判据静默放水；白名单兜底 DB 抖动时**实际 fail-close 还写 60s 负缓存**（与函数注释承诺相反）；`refresh_guard` 无单飞（60s 到点全体并发全量重拉，惊群打 PG）；interval / 待审档位查询失败被缺省值**覆盖**（违反本函数自述纪律）；显式「重置密钥」与「改密顺带轮换」共用 7 天宽限 ⇒ **泄露后的撤销动作撤销不掉**；7 个新开关只写在代码与文档里，compose 与 .env.example 没有 ⇒ 站长看不见也拧不动 |
| **P3** | 5 | `numwant` 双数（入口 200 / 表侧 50）；`percent_decode` 不把 `+` 还原成空格（UA/黑白名单口径）；`/metrics` 仍与 announce 同口暴露；`sweep_stale_peers` 阈值与 `seed_cap` 口径不一致；外置 `upsert` 忽略 Lua 返回的「顶号被拒」码 |

**同时确认仍然为绿、不必重复上报的**（本轮探针 + 三个常驻探针复跑，见 §十）：
10-06 八条、10-07 深挖的四批修复项（IP 取信 / 段封禁 / scrape 上限去重 / passkey 宽限读口统一 /
顶号归属 / 待审准入 HTTP 侧 / 参数严格化 / XADD MAXLEN / 消费者回收 / 同账号 peer 不外发 /
事件合并窗）**全部仍然成立**。

---

## 二、实测环境与探针

- 被测：本机 `flux-tracker`（镜像 `90429fae451a`，10-08 10:28 构建）、
  `flux-worker`（`36317c509215`，11:28 构建）、`flux-api`、`flux-postgres`、`flux-redis`。
  Docker Desktop 本轮由我启动（会话开始时未运行）。
- **判据是行为，不是 grep 二进制**：`docker inspect .Created` 与 commit 时间只做旁证
  （worker 镜像 11:28 > 批一 07:23 ⇒ 缺陷确在线上）。
- 新探针 **`.workbuddy/_probe_r5_gaps.py`（29 条断言，A 计费入账 / B UDP gate /
  C 四读口×三档可见性矩阵 / D 交叉佐证铸币 / E 端口黑名单）**。
  它编码的是**修复后的期望**，所以修复前跑必然一片红——修复前的实际结果见 §十。
- 常驻回归（本轮复跑）：`_verify_batch_1008.py`、`_verify_tracker_round.py`、
  `_verify_event_merge.py`、`_verify_policy_switch.py`；`cargo test -p flux-tracker`、
  `-p flux-worker`；`scripts/line_limit_guard.mjs`、`check_type_drift.mjs`。
- C 组这张矩阵是本轮方法上最值钱的一笔：**HTTP announce / HTTP scrape /
  UDP announce / UDP scrape 是同一份 swarm 数据的四个出口**，只测其中一个口的
  「被挡住」等于没测。上一轮的待审准入复验就只测了 HTTP announce。

---

## 三、P0-1 全站流量入账停摆：修 P0 的那一批把计账本身改死了

**实测**（A 组，全新账号 + 全新种子，两次 announce 间隔 1800 s）：

```
第一次 announce（up=0,down=0,left=size,event=started）→ snatches 建行：up=0 down=0 last_up=0
   last_seen_at 推回 1800 s
第二次 announce（up=1 GiB,down=512 MiB,event=started，读数单调、无回退）
→ snatches.uploaded=0  downloaded=0  last_up=0  traffic_ledger 无任何流水
```

1 GiB / 1800 s ≈ 0.6 MiB/s，是站点速率上限（128 MiB/s）的 **0.4%**——
这是最标准的正常做种曲线，入账却是 0。

**根因**（`apps/worker/src/jobs/ledger_guard.rs`，批一 `01b23aa` 引入）：

```rust
let reset_ok = dropped_up || dropped_down;                 // 只有「读数下降」才为真
let (anchor_up, anchor_down) = if reset_ok && (accept_up && accept_down) {
    (ev.up, ev.down)
} else {
    (last_up, last_down)                                   // ← 正常单调上报也走到这里
};
let raw_up = (anchor_up - last_up).max(0);                 // ⇒ 恒等于 0
```

写这段时的意图是「把基线钉死，防搬基线」，但判据的**肯定式条件把最常见的路径
（没有任何回退）当成了例外分支**：`reset_ok` 恒 false ⇒ 走 else ⇒ 锚点原地不动 ⇒
增量恒 0。

**打穿的面**（这条不是「少记一点」，是整条链停摆）：
`users.uploaded/downloaded` 冻结 → ratio 恒不变、`traffic_ledger` 不再产行、
`snatches.downloaded` 恒 0 ⇒ `phys_down` 恒 0 ⇒ **每一个做种者的 `seeding` 都判 false**
→ 在种数塌、保种时长不累计、`torrents.seeders` 由 snatches 重算所以也塌、
完成数 `times_completed` 不再增长；同时 `ghost_seed` 留痕条件
（`left=0 且无物理下载量`）对**所有**做种者成立：

```
docker logs flux-worker | grep cheat_enforce
  WARN cheat_enforce: L2 告警 user=39 agent=ghost:1acaa148 hits=70
  WARN cheat_enforce: L2 告警 user=39 agent=ghost:f1dd9a3e hits=28
cheat_events 聚合：ghost_seed 15 行 / 152 hits
```

**为什么 22/22 全绿的验收探针没抓到**（这是本轮最该记住的一条）：
`_verify_batch_1008.py` 的 G 组断言形如「谎报者应得到 `seeding=false`、
`times_completed` 不涨、**行内 credited 下载为 0**」。而锚点被钉死之后
**所有人**都是 `seeding=false`、都是 `credited=0`——
「攻击被挡住」与「防线把门拆了」在同一组否定式断言里长得**一模一样**。
凡是只用否定式断言编码的验收，都必须配一条**正控**（合法用户必须拿到合法结果），
否则等于没有验收。这条已写进探针 A1–A4 与 §十一。

**修法**（本轮已落地）：
- 判据抽成**纯函数** `anchor_for()`（`ledger_guard.rs`），三种情形显式分开：
  ① 单调增长 ⇒ 锚点跟随新读数；② 下降且新值 ≤ 1 GiB ⇒ 认可真实重启（并允许锚点**下调**，见 P1-2）；
  ③ 下降但新值仍大 ⇒ 钉死 + `counter_reset` 留痕。
- 新增 6 条单测钉住这三种情形，其中
  `monotonic_reading_advances_anchor` 就是这次的反例钉；
  `reboot_then_resume_keeps_accrueing` 钉「真重启的用户之后还能照常入账」。
- **首报（无基线）raw 恒 0**，只立基线。原写法首报按 `ceiling × 120 s` 给额度，
  而客户端累计计数器是**全局**的（不是每种一个），把首报读数当增量入账
  等于把别的种子的量记到这颗种上——换种子无限首报就是假种审计里的
  「单种首报额度 600 GiB」那一档，本轮顺手收掉。

---

## 四、P0-2 交叉佐证是一台没有上界的铸币机，而豁免判据是「> 0」

**机制回顾**（10-07 的 P0-2 治本）：leecher 在 announce 上带
`xreport=<peer_id_hex>:<bytes>` 声明「本轮从这位 peer 收到了 N 字节」，
tracker 校验目标 peer 在本 swarm 存活且在做种，worker 把 N 累加进
`upload_corroborated(uploader, torrent)`，`process_event` 再用它
**封顶**上传者的入账量（`room = corroborated - 已入账`）。

**实测**（D 组，两个全新账号，双方在本种上 credited 下载都是 0）：

| 断言 | 结果 |
|---|---|
| D0 正控：零下载的声称做种者先要被判**不在种** | PASS（`seeding=f`，判据基座成立） |
| D1 佐证量 ≤ 佐证者自己被记账的下载量 | **FAIL：B 自己 credited 下载 = 0，却为 A 确认了 9,663,676,416 字节（9 GiB）** |
| D2 一条他人自报的佐证不该把幽灵种点亮 | **FAIL：`seeding=t`** |
| D3 1 字节佐证不构成「手里有数据」的证明 | **FAIL：`upload_corroborated=1` ⇒ `seeding=t`** |

**根因两处**：
1. `apps/worker/src/jobs/xreport.rs` 对 `bytes` 唯一的限制是
   「单笔 > 10 GiB 判 absurd 并丢弃」。9 GiB 一次、每 30 min 一轮，
   一颗种子的 `upload_corroborated` 一天能涨到 432 GiB，
   而 `snatches.uploaded ≤ 种子大小 × 1000` 的终身天花板对**小种子极宽**
   （32 MB 的种 = 32 TB 额度）。⇒ 「可信上界」本身完全由攻击者供给。
2. `process_event.rs:126`（辅种豁免，`f8fd42c`）：
   `cross_seed_vouched = corroborated.is_some_and(|c| c > 0)`，
   然后 `seeding` 与 `completed` 的**两条**证据条件都被它 `||` 掉。
   1 字节即点亮，且 `seeding` 在 upsert 里是**粘性 OR**（只在 `stopped` 时才落回 false）。

**主流对照**（本轮源码核实，见 §八-5）：UNIT3D / NexusPHP / Ocelot **三家都没有
leecher 侧的交叉佐证机制**（announce 参数白名单里就没有这类字段）。
也就是说这条不是「业界标配我们没抄全」，是**我们自研的**判据——
自研的东西没有上游现成的边界可以抄，就必须自己给它立物理不变式。

**修法**（本轮已落地）：
- **物理不变式**：`一个 leecher 在这颗种上能背书的总量 ≤ 它自己被记账的 credited 下载`。
  抽成纯函数 `corr_room(claimed, leecher_credited, already)` + 6 条单测。
- 新增迁移 **`0305_xreport_physical_bound.sql`**：长存台账
  `xreport_credited(leecher, info_hash, credited)`。
  **为什么不直接聚合 `leecher_xreports`**：那张表按 7 天就地裁剪（0301 的口径，
  给 `down_under` 对账用），拿被裁剪的表算额度 = 每 7 天免费重置一次。
  明细表 `leecher_xreports` 保持**原始自报口径**不动（另加 `over_bytes`
  记未被认可的部分），这样 `down_under_check` 的「佐证和 vs 自报 downloaded」
  对账链一个字都没改，谎报信号不丢。
- 豁免门槛从「> 0」抬到**与 `phys_down` 同一把尺子**（`size × 10.4%`）：
  `seeding_gate::corr_threshold()`。辅种人群要拿到豁免，必须站内真有 leecher
  从他身上下载过达到可信规模的量——这才是「手里有数据」的证明。
- tracker 侧 `http_track/xreport_in.rs`：声称量 > 种子大小直接丢 +
  新计数器 `flux_tracker_announce_xreport_rejected_total`
  （一颗 4 MiB 的种不该被背书 TiB 级）。

**遗留建议**（比 xreport 更该做，且**不需要客户端配合**）：
NexusPHP `check_cheater()` 里有一条按 swarm 一致性说的判据——
「声称上传 > 10 MiB 而**该 swarm 当时一个 leecher 都没有**」直接判作弊
（`functions_announce.php:112-190`）。我们的 qBittorrent/Transmission
**永远不会发 xreport**，所以佐证上界今天对绝大多数账号是空转的；
而「有没有人在你这下载」是 tracker 自己就知道的事实。建议下一轮把这条
做成 `phys_down`/`corroborated` 之外的第三条独立证据线。

---

## 五、P1（4 条）

### P1-1 速率钳的秒窗没有上界：announce 越稀疏，单笔越大

`ledger_guard.rs:90`（原文）：

```rust
let secs = last.and_then(|(_, _, s, _, _)| s).unwrap_or(120).max(1);
let allowance = ceiling_bps.saturating_mul(secs);
```

`secs` 取自 `now() - snatches.last_seen_at`，也就是**由攻击者的沉默时长决定**。
一次 announce 前静默 24 h ⇒ 单笔额度 = 128 MiB/s × 86400 s ≈ **11.26 TiB**，
一周则是 78 TiB。「增量不得超过秒数 × 声明速率」这条防线被自己的时间基准反噬：
正常客户端每 interval 一次，攻击者**少发**就行。

对照主流（§八-1）：**三家根本不做「时间 × 速率」入账钳制**——
UNIT3D / NexusPHP / Ocelot 全是 `max(0, 上报 − 存储)`，超额靠**事后可疑检测**
（NexusPHP `check_cheater` 的速率阈值 + `leechers==0` 一致性）而不是事前扣量。
所以我们既然选择了「按速率钳」这条更强的路，钳的分子（秒窗）**必须封上界**，
否则钳等于没有。

**修法**（本轮已落地）：`secs` 夹在 `[1, secs_cap]`，`secs_cap` 就是调用方已有的
做种时长容忍窗（`seed_cap = 2×interval`，`announce_main.rs:24-34`）——
与 `seeded_seconds` 的封顶**同一个口径**，两条判据不会互相矛盾；
正常客户端的相邻事件间隔本来就不该超过它。

探针 A5 钉这条：沉默 24 h 后一笔 +10 TiB 的 announce，入账必须 ≤ 上限 × 2×interval。
（A1 红的时候 A5 无法判定，探针里显式打印「跳过而不是假绿」。）

### P1-2 近零重置被 `GREATEST` 钉回高位：真重启的用户流量永久冻结

批一允许「读数掉到 ≤ 1 GiB」算真实重启（客户端重启/换机就是这个形状），
但 `process_event.rs:186` 写库是：

```sql
last_up = GREATEST(snatches.last_up, EXCLUDED.last_up)
```

`GREATEST` 的本意是「锚点只进不退」的兜底，它**把刚认可的那次下调又顶回旧值**。
后果不是漏判而是**误杀 + 永久冻结**：做过 500 GiB 的用户重启 NAS，
第一次 announce（比如 8 MiB）被认可 → 锚点仍留在 500 GiB；
第二次 announce（3 GiB）对照 500 GiB 就是「下降且非近零」⇒
记 `counter_reset` **作弊留痕** + 增量恒 0，此后要等真实计数器爬回 500 GiB
才重新开始记账——表现就是「我做种一整天，上传一动不动，还被站方标记为异常」。

**修法**（本轮已落地）：`anchor_for()` 返回 `reset_up/reset_down`，
SQL 改 `CASE WHEN $18 THEN EXCLUDED.last_up ELSE GREATEST(...) END`（两侧各一格），
认可的重置允许锚点下调，其余路径的 `GREATEST` 兜底照旧。
单测 `near_zero_reset_is_accepted_and_lowers_anchor` 钉住「只降一侧时另一侧不被标记」。

### P1-3 UDP 两个读口完全绕过准入 gate（实测）

`udp/announce.rs` 自己抄了一份「特权端口 + peer 落表」的代码，
**没有调用 `http_track::gate`**，于是批二/批四在 HTTP 侧建的三道判据在 UDP 上全部不存在：

| 判据 | HTTP | UDP（实测） |
|---|---|---|
| `left > 种子大小` 判 fake announce 并拒 | 拒（B1 PASS，对照组） | **照收并进事件流（B2 FAIL）** |
| 待审种子 `self_seed_only` 对局外人清空 peer/计数 | 清空（C2） | **照样把发布者 ip:port 发给任何人（C2 FAIL）** |
| 待审种子 `owner_only` 拒服务 | 拒（C1） | **照样服务（C1 FAIL）** |
| 顶号归属校验 | 有 | 有（这份抄写恰好带上了） |

正控也已实测：同一 UDP 会话发合法 `left` 得到 `ok {'interval':1800,'leechers':1,...}`，
所以 B2 的红**不是通道没通**，是 gate 没跑。

危害与 10-07 P1-1/P1-2 完全同型，且更隐蔽：版主以为待审种子已经不外泄了。
**同一份 swarm 数据两个入口，严的那个形同虚设——攻击者只要换协议。**

**修法**（本轮已落地）：UDP 改调 `gate::announce_gate` + `gate::write_peer`，
待审时 `hide_peers` 让响应回 0 计数空 peer（与 HTTP 的 `reply.rs` 同语义）；
`udp/scrape.rs` 补同一份档位判据。顺带删掉两处「各抄一份」的重复代码：
特权端口判据（现在只活在 `announce_gate`）、peer 落表逻辑（现在只活在 `write_peer`）。

### P1-4 scrape 的待审档位判据写反：档位越严，scrape 越漏

`http_track/scrape.rs:41`（原文）：

```rust
let pending_hidden = pending_policy() != 0 && pending_policy() != 2;
```

`!= 2` 把**最严档 owner_only** 从隐藏名单里排除掉了。实测（C1）：

```
owner_only：announce(HTTP) 不回真实计数 = True
            scrape(HTTP)  不回真实计数 = False   ← 待审种的做种数照样送给局外人
self_seed_only：两口都收口（默认档恰好是对的，所以一直没被发现）
```

站长照面板提示选最严档，得到的是**最宽的 scrape**。
`self_seed_only` 是默认值，所以这条只在「有人真的把策略拧到 owner_only」时显形——
正是「配置面拧动之后才暴露」的那类缺陷（与 §十三 的开关复验同一口径）。

**修法**（本轮已落地）：判据收成一个函数、四个读口共用一份：
`guard_store::hides_pending(info_hash, uid, class_id)` = 「档位非 allow_all ⇒ 隐藏」。
档位语义从此只能单向变严。

---

## 六、P2（7 条）

1. **作弊处置链三处断口**（`cheat_enforce.rs`）：
   - `c.agent = 'torrent:%'` 把 **LIKE 写成了等值**，字面量里带 `%` ⇒ 永不匹配，
     `audit.rs:75` 写的 `torrent:{id}` 类事件从未进过处置（已改 LIKE）。
   - `corr:`（自报上传超佐证上界）、`xreport_over:`（本轮新增）、
     `nearcap:`（0304 贴边汇报画像）**不在任何处置过滤器里**：台账写了、面板看得见，
     L1/L2 永不触发。0304 那套「网盘挂 NAS 临时挂载型假做种专杀」实测**只产画像不产处置**。
   - 经济侧拉黑名单（`seeding.rs:78-81`）仍只认 `ghost:%/speed:%/reset:%`。
     `seeding.rs` 此刻是另一路在制品，**本轮不代改**，清单见 §十「未改」。
2. **外置存储模式下 HTTP 的 `conn` 读内存表**：`announce.rs` 用
   `state.peers.connectable_of()`，而探测结果在 `FLUX_TRACKER_PEER_STORE=redis`
   时写的是 `flux:swarm:{hash}`（`main.rs:288-296`），**UDP 侧早就读外置**
   （`udp/announce.rs:189`）。多副本下本进程没测过这条 peer ⇒ `conn` 恒未测 ⇒
   `ev.conn != Some(0)` 这条幽灵判据静默放水。已改为与 UDP 同口径。
3. **白名单兜底在 DB 抖动时 fail-close 且写负缓存**：`torrent_registered` 的注释写着
   「查询失败仍 fail-open」，代码却是 `.ok().flatten()` ⇒ Err 与「确实没这颗种」
   同为 None ⇒ 既拒绝本轮，又把它记进 60 s 负缓存（一次抖动把误判自我加固一轮）。
   已按 `Ok(Some)/Ok(None)/Err` 三态分开：抖动 ⇒ 放行、不写负缓存、warn。
4. **`refresh_guard` 无单飞（惊群）**：所有 announce/scrape 都调它，
   60 s 一到全体在途请求同时看到 stale，各自跑一遍**全量**重拉
   （白名单是一次 10 万行 UNION + 20 MB 字符串集合），恰好在流量最高时把 PG 打成放大器；
   passkey 缓存同理（60 s TTL 到点时的 cache stampede）。
   已加进程内单飞（RAII 复位——actix 在客户端断连时会 drop 掉这个 future，
   手写 `store(false)` 一旦漏在某个返回路径上就等于防护缓存从此不再刷新）。
   抢不到这一格的请求直接吃旧快照：60 s 的缓存语义本来就允许短暂陈旧。
5. **查询失败覆盖旧值**：`interval` 与 `announce_pending_policy` 用
   `.unwrap_or(缺省)`，一次 DB 抖动就把站长设定的 interval（连带 TTL 与合并窗长度）
   和准入档位**静默重置回默认**，违反本函数自述的「单项查询失败保留旧值」纪律。
   已改成三态：`Ok(Some)` 按值、`Ok(None)` 用缺省、`Err` 保留旧值 + warn。
6. **passkey 显式重置与改密轮换共用 7 天宽限 ⇒ 撤销不生效**：
   `repo/mod.rs:223`（后台/本人显式「重置密钥」）与 `me_security.rs:65`（改密）
   都写 `passkey_prev + PASSKEY_GRACE_HOURS`。但「重置密钥」在 PT 站是**安全处置动作**
   （密钥泄露了要立刻断），按现在的写法攻击者拿着泄露的旧钥还能继续做种 7 天。
   主流对照（§八-4）：UNIT3D 重置是**立即失效**，且「改密」与「重置 passkey」
   是两个独立动作（`PasswordController` 里没有一处 passkey 字样）。
   **本轮未改**（涉及产品语义取舍，需站长拍板）：建议
   改密⇒给宽限、显式重置⇒`passkey_prev_until` 置 NULL，并在面板写清两者区别。
7. **新开关不在部署面**：`ANN_EVENT_MERGE_PCT`、`ANN_INTERVAL_JITTER_PCT`、
   `ANN_BLACK_PORTS`、`SCR_RATE_IP_PER_MIN`、`SCR_MAX_HASHES`、`TRUST_PROXY_DEPTH`、
   `ALLOW_PRIVATE_PEER_IP`、`FLUX_TRACKER_PIECE_PROBE`、`PASSKEY_GRACE_HOURS`
   此前**只存在于代码默认值与 `_doc` 里**，tracker 服务的 `environment:` 是显式列表
   且没有 `env_file` ⇒ 站长既看不见也要手改 compose 才能配。
   对「可支持任何类型 PT 站的建站系统」这是交付缺口。本轮已全部写进
   `docker-compose.yml`（api 侧的 `PASSKEY_GRACE_HOURS` 也补了），默认值与代码一致。

---

## 七、P3（5 条）

1. `numwant` 双数：入口 `clamp(0, 200)`（`announce.rs:58`）vs 表侧 `MAX_PEERS_RESPONSE = 50`
   （`table.rs:18`）。要 200 永远只拿到 50。对照：UNIT3D 硬上限 25、chihaya 默认 50/最大 100。
   **建议**一个常量管两处，并在部署文档写明本站真实上限。
2. `percent_decode` 不把 `+` 还原成空格 ⇒ 文本参数（`agent`/`event`）口径偏差，
   影响 `snatches.agent` 取证与 `agent_rules` 黑白名单匹配。
   原始字节参数（`info_hash`/`peer_id`）本来就不该转 `+`，**只需要对文本参数走标准解码**。
3. `/metrics` 仍与 announce 同口（公网 :7070）暴露。token 已定长比较、无 token 时 404，
   但「独立 bind 地址」这项上一轮就列在待办里，仍未做。
4. 两处口径不一致的账：`sweep_stale_peers` 的阈值是 `(interval×2).max(7200)`
   （`sweep.rs:9`）而计账容忍窗是 `(interval×2).clamp(3600, 172800)`（`announce_main.rs`），
   两个「陈旧」不同值；外置 `external::upsert` 忽略 Lua 返回的「顶号被拒」码 2
   （Redis 侧正确拒写，但本副本内存表视为成功 ⇒ 顶号不报错、`peer_taken` 计数不出数）。
5. **BEP15 的 UDP scrape 响应第 2/3 格与规范对调**（本轮修 UDP scrape 档位时顺手实测到，
   ）：规范是 `complete, downloaded, incomplete`，
   `udp/scrape.rs` 发的是 `complete, incomplete, downloaded`。
   定级 P3（只有 UDP 这一路、且 `complete` 在第 1 格没受影响），但它是
   客户端可见的协议违规。
   判据不是读码而是把两个量摆成不同值再数一遍：把 `times_completed` 设成 77、
   swarm 里放 1 个做种者与 2 个下载者，回包三格是 `(1, 2, 0)`
   —— `2` 是 leecher 数却坐在 `downloaded` 的位置上，`0`（快照未刷新的完成数）
   坐在 `incomplete` 的位置上。标准客户端由此读到「完成数 = 下载中人数」。
   HTTP scrape 走的是 BEP3 字典（键名自带语义），所以只有 UDP 这一路错。
   **同时纠正探针自己的两处错**：它按 `offset 16` 起算行（BEP15 的响应头只有
   8 字节：action + tid），于是 `(20-16)/12 = 0` 行、C 组的 `udp_scrape` 列
   一直是「读不到」——那一列此前的 PASS/FAIL 全部按不可信处理，本轮的
   C1/C2 `scrape(UDP)` 结论以修复后的复跑为准（见 §十）。

---

## 八、按 PT 惯例的差集（本轮全部拉源码核实，附出处）

> 这一节是把「我以为业界怎么做」换成「业界代码里到底写了什么」。
> 逐条给仓库、文件与原文，方便复核；出处 URL 见文末。

1. **入账增量的钳制**：UNIT3D `app/Jobs/ProcessAnnounce.php`
   ```php
   $uploadedDelta = max($this->queries->uploaded - ($peer?->uploaded ?? 0), 0);
   ```
   Ocelot `worker.cpp:393-410` 同型（`uploaded < p->uploaded` 才当回绕），
   NexusPHP `public/announce.php:431`
   `max(0, $uploaded - $self["uploaded"])`。
   ⇒ **三家都没有「时间 × 速率」的入账钳制，更没有秒窗上界**；
   超额一律**静默丢弃**（`max(0)`），只有 NexusPHP 另跑 `check_cheater` 事后判罚。
   终身天花板也基本没有：只在 NexusPHP 的**种箱专用**设置里
   （`globalfunctions.php:1118` `size × maxUploadedTimes`）。
   ⇒ 结论：我们的速率钳是**超出主流**的加固，正因为它引入了时间因子，
   秒窗不封顶就成了 P1-1；而「事后判罚」这一层（§四 遗留建议）我们反而比主流弱。
2. **`completed` 需要既有 peer 行**：UNIT3D `AnnounceController.php:443-454`
   查 `peers where peer_id=… and user_id=…` 为空 ⇒ `TrackerException(152)`
   （`152 => 'Torrent being announced as complete but no record found.'`）；
   NexusPHP 只在 `elseif(isset($self))` 分支里加 `times_completed`，效果同；
   Ocelot `worker.cpp:321` 是 `completed_torrent = (left == 0)`，**不要求既有行**。
   ⇒ 我们的 `had_baseline` 与 UNIT3D 一致，已复验为绿。
3. **端口校验**：UNIT3D 是**硬编码常量**
   ```php
   private const array BLACK_PORTS = [8080,8081,1214,3389,4662,6346,6347,6699];
   ```
   并在 `port < 1024 && event !== 'stopped'`、`port > 0xFFFF`、`in_array(BLACK_PORTS)`
   三种情形抛 `TrackerException(135)`。NexusPHP `portblacklisted()` 是
   `411-413, 6881-6889, 1214, 6346-6347, 4662, 6699`，且**没有 <1024 规则**。
   ⇒ 本轮按 UNIT3D 补齐黑名单（E 组实测：445 早被 <1024 挡住，
   **5432/6379 此前照收**）；**刻意不跟 NexusPHP 把 6881-6889 拉黑**——
   那是 qBittorrent/Transmission 的默认监听段，照抄会把一大半真做种者判成非法，
   要收的站长用 `ANN_BLACK_PORTS` 自己加。
4. **passkey 撤销**：UNIT3D `User/PasskeyController.php:52-70` 换钥后
   `cache()->forget(旧钥)`、旧钥进 `passkeys` 历史表（仅审计），announce 只解析
   `users.passkey` ⇒ **立即失效，没有宽限**；且 `PasswordController` 里
   **一处 passkey 都没有**（改密与重置密钥是两个动作）。
   ⇒ 我们的 7 天宽限是超出主流的可用性选择（合理，passkey 烤在 .torrent 里），
   但把「显式重置」也一并宽限，等于让安全处置动作失效（P2-6）。
5. **leecher 侧交叉佐证**：**三家都没有**。参数白名单就只有
   `info_hash,peer_id,port,uploaded,downloaded,left,event,numwant,corrupt,key`。
   最接近的类比是 UNIT3D `histories` 里同存三口径
   `'uploaded' => creditedDelta, 'actual_uploaded' => delta, 'client_uploaded' => 原始读数`
   ——即**把「入账量/物理增量/客户端读数」分列存**，比值只用 credited。
   Ocelot/Gazelle 侧：`xbt_snatched` 不带字节数， Gazelle 没有
   `actual_uploaded/actual_downloaded` 这两列（网传口径不成立）。
   ⇒ 我们的 xreport 是自研机制，物理上界必须自己立（P0-2 修法）；
   同时建议吸收 UNIT3D 的**三口径分列**——我们目前 `snatches.uploaded`(credited)、
   `users.*`(倍率后)、原始读数只活在事件流与留痕文案里，取证时拼不回来。
6. **最短间隔强制**：NexusPHP `announce.php` 用
   `warn('There is a minimum announce time of ' . N . ' seconds')`，
   且该分支**不落 peer、不计费**；UNIT3D 用 Redis 锁、阈值为
   `interval × 85–95%`，违规抛 162；Ocelot 无此强制。
   ⇒ 本轮我一度照这个口径在两个入口实现了「低于最短间隔直接拒」，
   **复核后主动撤掉**，理由记在这里以免有人再当成缺陷上报：
   ① 本站 `min_interval = interval/2`，而抖动后的 interval 恒 ≥ `0.9×base > min_interval`
   ⇒ 判据**永不成立**，写下去就是死码；
   ② 关键在于**把 P1-1 的秒窗封顶之后，快发对入账量已经没有任何收益**
   （gap 越小 allowance 越小），而做种时长也受 `min(gap, cap)` 封顶；
   剩下的只是负载面，而那已被批四的合并窗 + 每用户/IP 限流覆盖
   （A6 实测：快发那条 `last_seen_at` 年龄仍是 142 s，事件根本没投）。
   真正值得做的是把「最短间隔」做成**档位**（拒 / warn 不计 / 不干预，
   UNIT3D 与 NexusPHP 各选其一），这是站型策略，留给站长拍板。
7. **connectability**：UNIT3D `config/announce.php` 的
   `connectable_check` **默认 false**，检查由 `ProcessAnnounce::getConnectableStatus()`
   的 `fsockopen(ip,port,1s)` 做、结果存 `peers.connectable`，
   消费者只有 **BON 积分规则**（`BonEarningCondition` 的 `'connectable'` 操作数）；
   NexusPHP 建行时**硬编码 `connectable='yes'`**，真做检查的 peerlist SQL 是注释掉的；
   Gazelle `xbt_files_users.connectable` 默认 1，Ocelot 从不写它。
   ⇒ **三家都只当信息位，没有一家拿它否决 seeding/计费**。
   我们把 `ev.conn != Some(0)` 做进了 `seeding` 必要条件（保种组审计那轮的决策），
   代价是：**NAT 后没有映射入站端口的真做种者会被判不在种**。
   本轮不改语义（属产品取舍），但按 P3/§八 的规矩明确记为待拍板项：
   建议做成 `connectable_gate: off | hard`（缺省 off = 不否决，实测结果仍留在
   `snatches.connectable` 供面板筛），并把「BT 握手失败」与「TCP 连不上」在留痕里
   分开——两者现在都塌成 `conn=0`，版主看不出是「端口没开」还是「开着但不是
   BT 协议」。（同日已按此落地两档；第三档 soft 写过又删，见 §十。）
8. **scrape**：UNIT3D **根本没有 scrape 端点**（`routes/announce.php` 只有
   `{passkey}` 一条），所以「私募站 scrape 该怎么收口」没有上游可抄；
   NexusPHP `public/scrape.php` 复用 announce 的鉴权（passkey/停用/下载位/浏览器块），
   但**种子查询不带任何审核/可见性过滤**，而其 announce 会拒 banned/未过审
   （`announce.php:197-204`）⇒ **NexusPHP 线上就有我们 P1-4 这条泄漏**。
   info_hash 条数上限两家都没有（NexusPHP `checkScrapeFields` 只校验 20 字节长度）。
   ⇒ 本轮修完之后，我们在这一项上**优于两份参照实现**。

---

## 九、本轮复验为绿、请勿重复上报

10-06 八条与 10-07 四批项全部复跑通过（数字见 §十）。本轮新增确认有效的：

- 事件合并窗（批四）在快发场景下确实不投事件（A6 现场 `last_seen_at` 未被刷新）；
- HTTP 侧 `left > size` 拒 + `announce_fake_left` 计数（B1 对照）；
- 顶号归属：UDP 侧沿用同一份 `remove_owned`/`upsert` 语义（本轮进一步合并成 `write_peer`）；
- scrape 在默认档 `self_seed_only` 下与 announce 同收口（C2 两口都 True）；
- port 445 已被 `<1024` 那半挡住（E 组）；6881 正常端口不被误杀（E0）；
- 幽灵做种基座成立：零下载的声称做种者默认判 `seeding=false`（D0）；
- UDP connect 魔数、`connection_id` 绑源端口（BEP15 正解，本轮踩过一次探针坑）；
- 消费者组、XADD MAXLEN、`/metrics` 定长比较、段封禁 CIDR、passkey 视图口径
  等上一轮项：常驻探针全绿。

---

## 十、落地状态与验收

### 已改（本轮，代码 + 单测；未提交部分见文末清单）

| 条目 | 落点 | 验证方式 |
|---|---|---|
| P0-1 锚点判据 | `ledger_guard.rs` 抽 `anchor_for()` 纯函数 + 首报只立基线 | 6 条单测（含反例钉） |
| P0-2 佐证上界 + 豁免门槛 | `xreport.rs`、迁移 `0305`、`process_event/seeding_gate.rs`（新）、`xreport_in.rs`（新）+ `helpers/metrics` 新计数器 | 6 条单测 + 探针 D1/D2/D3 |
| P1-1 秒窗封顶 | `ledger_guard.rs`（`secs ∈ [1, secs_cap]`） | 探针 A5 |
| P1-2 认可重置允许锚点下调 | `process_event.rs` SQL `CASE WHEN $18/$19` | 单测 + 探针 A1-A4 |
| P1-3 UDP 走 gate | `udp/announce.rs`、`udp/scrape.rs`、`guard_store::hides_pending` | 探针 B2/C1/C2 四读口 |
| P1-4 scrape 档位反转 | `http_track/scrape.rs` | 探针 C1 |
| P2-1 处置链断口 | `cheat_enforce.rs`（LIKE 拼写 + `corr:`/`xreport_over:`/`nearcap:`/`xreport:absurd`） | 待部署后按现场 `cheat_events` 复核 |
| P2-2 外置模式 conn | `announce.rs` 改读 `external::connectable_of` | 单机栈（非外置）行为不变，需外置侧容器专测 |
| P2-3/4/5 白名单三态 / 单飞 / 保留旧值 | `guard_whitelist.rs`（新）、`guard_refresh.rs` | 探针 + 编译 |
| P2-7 配置面 | `docker-compose.yml`（tracker 6 项 + api `PASSKEY_GRACE_HOURS`） | `docker compose config -q` |
| 端口黑名单 | `http_track/limits.rs`（新，与 interval 抖动同域） | 2 条单测 + 探针 E |
| interval 稳定抖动 | 同上（`peer_interval` 唯一口径，`reply.rs` 消费） | 单测（同 key 恒定 / ±10% 内 / 60 颗种 >10 个取值） |

### 站长拍板三件（同日追加，「按这个方案推进」之后落地）

| 件 | 落点 | 判据要点 | 验证状态 |
|---|---|---|---|
| ① passkey 显式重置 = 立即撤销 | `repo/passkey.rs`（新，`update_passkey(user_id, grace)`）、`me_security.rs` 自助轮换与 `admin_http/user_passkey.rs` 后台代重置传 `grace=false`、改密顺带轮换保留 `grace=true` | 泄露后按「重置密钥」是**安全处置**，留 7 天窗 = 撤销了但攻击者还能接着做种 7 天；UNIT3D 换钥即 `forget(旧钥)`，且「改密」与「重置 passkey」是两个互不相干的控制器 | **已部署已实测**（探针 ① 组 9/9，含「改密旧钥仍可用」正控） |
| ② connectable 只作信息位 | `site_settings.connectable_gate` = `off`（默认）/ `hard`、`seeding_gate::verdict` 的 `reachable` 只在 hard 档否决、`refresh_conn_gate` 60 s 节流且读失败保留旧档 | 三家主流都把可达性当信息位（§八-7）；硬否决会把 NAT 后无映射入站端口的**真做种者**判成不在种。**只有两档**——曾写的第三档 soft 与 off 行为无差别，是没有后果的假枚举，已删 | **已部署已实测**（探针 ③ 组 14/14：同一份 `conn=0` 在两档下结论相反） |
| ③ 即时分享率闸门 | `ratio_gate.rs` + `ratio_gate/tests.rs`（新，announce 热路径）、迁移 0308 四个键、视图 `user_by_passkey` 补 `uploaded/downloaded/created_at/ratio_watch_until` | 原先 `ratiolimit` 与全部 `user_classes.min_ratio` **零消费者**＝假开关。门槛 `min(max(等级 min_ratio, ratiolimit), ratio_gate_max)`；缺省档 **warn**（只计数不拦）；豁免：员工/`downloaded=0`/新人（`ratio_gate_grace_days`，与等级 `min_age_days` 取大者）/观察期内；只拦 `left>0` 的下载侧，做种永不拦 | **已部署已实测**（探针 ② 组 20/20，含四条豁免各自判别正控） |

线上行为验收（`.workbuddy/_probe_r6_three_switches.py`；api/tracker/worker 均为本轮
工作树出的新镜像，迁移 306/307/308 由 api 启动时 sqlx 应用；每条否定式断言都配正控）：

| 组 | 关键判据 | 结果 |
|---|---|---|
| ① passkey（9 条） | 改密后旧钥**仍可用**（`passkey_prev=旧钥 AND passkey_prev_until>now()`）；`/me/passkey/rotate` 后 `passkey_prev IS NULL`、旧钥 announce 回 `passkey 无效`；上一代宽限窗里的钥一并失效；新钥立即可用 | **9/9** |
| ② ratio（20 条） | `off` 放行；`warn` 放行且 `flux_tracker_ratio_gate_warn_total` 0→1；`block` 拒且 `…_block_total` 0→1；比率 2.0 正控放行；**left=0 做种侧永不拦**；员工(class 91)/零下载/注册 1 天/观察期内四条豁免各自成立，且各自配「换一份输入就必须被拒」的判别正控；`ratio_gate_max=1.0` 把 `ratiolimit=6` 截住（比率 2.0 放行）而 0.50 仍被拒；等级门槛 1.5 赢过站点门槛 0.8（同一份 0.9 在 class 3 被拒、在 class 2 放行）；踩线（比率正好等于执行门槛）放行 | **20/20** |
| ③ conn（14 条） | 每步先打一发 `event=stopped` 把**粘滞的** `seeding` 清成 false（`EXCLUDED.seeding OR snatches.seeding`，不清就判不出档位作用），再喂同一份 `conn=0` 事件：`off` 点亮、`hard` 不点亮；`hard` 下 `conn=1` 仍点亮（不是恒 false）；`port=0` 两档都不点亮；不带 conn 字段（未抽样）按 off 点亮；`snatches.connectable=0` 留痕照写 | **14/14** |

跑完复原并回读：`connectable_gate=off`、`ratio_gate=warn`、`ratio_gate_max=1.0`、
`ratio_gate_grace_days=7`、`ratiolimit=6`；现场 10 个探针账号已墓碑化、5 颗探针种子已删、
`announce_seen`/`cheat_events` 残留 0。

安全轨（本机现状决定的，不是可选项）：`ratiolimit` 现值 **6**、所有 `min_ratio` **0** ⇒
直接把 block 打开等于全站锁死，所以 `ratio_gate_max` 缺省 **1.0** 并会在截断时打
warn，档位缺省 **warn**。这两个缺省值都要站长核对过数字之后再往严里拧。

一处「文档比代码诚实」的反例：`docs/user/07-account.md` 一直写着自助重置后
「旧 key 立即失效」，而代码给的是 7 天宽限窗（`passkey_prev`）——**这条用户可见文案
在 ① 之前是假的**，用户照它操作会以为泄露已经止血。① 落地后文案与实现第一次一致。
`docs/ops/security.md` 原先把「重置」与「改密」混写成同一种宽限语义，也已按两档拆开。

### 本轮没做（明确交代，别当已修）

1. **`seeding.rs` 经济拉黑名单扩类**——该文件是另一路在制品；
   需把 `corr:` / `nearcap:` 一并纳入（与 `cheat_enforce` 的告警面区分开，两个口径）。
2. `numwant` 单常量、`+` 解码、`/metrics` 独立 bind、`sweep` 阈值口径（P3 四条）。
3. **后台面板**：②③ 四个键写进了 `settings_meta`（面板可见可改），但
   `ratio_gate` 的判定结果只有两处出口——`flux_tracker_ratio_gate_{warn,block}_total`
   两个计数器与 `tracing` 日志（**不写 `cheat_events`**：低分享率是准入问题，不是作弊证据，
   混进 cheat_events 会把 `cheat_enforce` 的告警与累进处置一起误伤），
   也没有按人留痕。站长要看「本档拦了谁、拦了多少次」目前只能抓 `/metrics` 的增量
   或读运行日志——待行为探针跑通后一并评估要不要做成面板可读的一页。

### 验收数字（本机 `docker compose up -d tracker worker` + api 重启后实测）

| 判据 | 修复前 | 修复后 |
|---|---|---|
| `_probe_r5_gaps.py`（A/B/C/D/E 五组 24 条） | **15 pass / 14 fail** | **22 pass / 2 fail**（两条都不是产品红，见下） |
| ↳ 残余 A0「首报没落库」 | — | worker 消费是分钟级批处理，本机该轮 > 260 s 预算；同一判据由 `_probe_r5_credit.py` 用 420 s 预算跑到 **5/5**，故按探针预算问题处理（已把默认预算提到 420 s） |
| ↳ 残余 C0「UDP scrape 读不到」 | — | **探针按 offset 16 读格**（BEP15 响应头只有 8 字节）⇒ 由 `_probe_r5_udp_scrape.py` 取代并跑出 **13/13**；顺着这条查出并修掉了产品侧的真 bug（§七-5 格序对调） |
| A 组入账链（1 GiB / 1800 s 正常增量） | `uploaded=0`、无流水 | **入账 1,073,741,824 = 1 GiB 整**，`downloaded=512 MiB`，`last_up` 前移，`_probe_r5_credit.py` **5/5** |
| A5 秒窗封顶（**可判别**那颗 16 GiB 名义种） | 未封顶额度 10.5 TiB 全额放行 | 入账 **450.0 GiB = 128 MiB/s × 3600 s**，正好卡在封顶值；`_probe_r5_secs.py` **4/4** |
| UDP 走 gate | `left>size` 照收、待审种外发 | UDP 回 `left 超过种子大小（fake announce）`；正控同一会话合法包 `ok`（下发的 interval 是 1721 ⇒ 抖动生效） |
| 四读口 × 三档矩阵 | owner_only 下 HTTP scrape 泄漏 | allow_all 四口全可见（含 scrape 的 `(1, 77, 1)` 证明 BEP15 格序），owner_only/self_seed_only 四口全部归零或拒绝 |
| 端口黑名单 | 5432 / 6379 进表并下发他人 | 三口（445/5432/6379）全拒；正控 6881 不被误杀 |
| 佐证铸币 / 1 字节豁免 | `upload_corroborated=9663676416` 而佐证者 credited 下载 0 ⇒ `seeding=t` | **`upload_corroborated=0`**，1 字节与 9 GiB 两档都不再点亮 `seeding`（D1/D2/D3 全绿） |
| 常驻回归 | — | `_verify_batch_1008.py` **24/24**、`_verify_tracker_round.py` **20/20**、`_verify_event_merge.py` **5/5**、`_verify_policy_switch.py` **9/9** |
| 单测 | 58 + 11 | `cargo test -p flux-tracker` **60/60**、`-p flux-worker` **26/26**（本轮新增 12 条，全是判据级） |
| 门禁 | — | `line_limit_guard` 本批文件零红（残留 7 条全属另一路在制品：`main.rs`/`bt_probe.rs`/`piece_cache.rs`/`torrent_parse.rs`，按规矩不代洗）；`check_type_drift.mjs` TYPE_DRIFT_OK；`docker compose config -q` 通过 |
| 迁移 0305 | — | 已在本机库 `psql` 应用（`xreport_credited` 4 列 + `leecher_xreports.over_bytes`）；`_sqlx_migrations` 尚未记 305——api 镜像是 10:29 构建的，不含该文件，**下次 api 出镜像启动时按幂等重放补记**（与 0303 同一处理，非异常） |

## 十三、本轮新增的常驻探针（改 tracker 判据先跑这四支）

| 脚本 | 钉住什么 | 为什么值得常驻 |
|---|---|---|
| `.workbuddy/_probe_r5_gaps.py` | A 入账链 / B UDP 走 gate / C 四读口×三档 / D 佐证物理上界 / E 端口黑名单 | 编码的是**修复后期望**；否定式断言全部配了正控（S0/S1/B2a/C0/D0/E0），正控不过会显式声明「后面的结论按不可信处理」 |
| `.workbuddy/_probe_r5_credit.py` | 1 GiB / 1800 s 的正常增量必须入账 + 锚点必须前移 | **A1 是全站入账停摆那条的反例钉**；只断言「谎报者被拒」的探针查不出它 |
| `.workbuddy/_probe_r5_secs.py` | 速率秒窗封顶 | 教了一课：**判据要可判别**。用 31 MiB 的种跑这条永远绿（终身天花板 30.5 GiB 比封顶额度 450 GiB 先截），必须造名义 > 11.26 GiB 的种让 `size×1000 > ceiling×86400`，脚本里把这条前置做成硬断言 |
| `.workbuddy/_probe_r5_udp_scrape.py` | 四读口×三档 + BEP15 scrape 格序（`complete, downloaded, incomplete`） | 把完成数设成 77 再数第几格——**用可区分的值定位字段**，而不是读码猜 |
| `.workbuddy/_r5_cleanup.py` | 探针账号/种子/关联行清理 + 归零核对 | 归零核对看的是计数而不是「应该没了」 |

### 方法论回执（三条，都付过学费）

1. **否定式断言必须配正控**。上一轮的 G 组只会判「谎报者拿不到 seeding」，
   锚点判据一坏，全体都拿不到 ⇒ 22/22 全绿地把全站计账改死。
   本轮每条「被挡住」前面都压一条「不挡的时候必须看得见」。
2. **判据要能判别**（`_probe_r5_secs.py` 的由来）：一个断言如果在缺陷存在时也只能
   通过，它就是装饰。写断言前先问「修复前的哪一行会让它红」。
3. **同一份数据的每个出口都要各自验一遍**。本轮 C 组那张四口矩阵一次抓出三个洞
   （UDP announce 两道 + scrape 档位一道），而它们各自都藏在「HTTP announce 已经修好了」
   的后面。

---

## 十一、探针自我纠正（三条假绿，全部当场修掉并留档）

本轮我自己造的坑，记下来是为了下次不再踩——**它们的共同形状是「否定式断言在
环境没通的时候也会绿」**：

1. **UDP 会话必须复用同一个 socket**。BEP15 的 `connection_id` 绑**源端口**
   （tracker 这条是刻意实现的，10-07 轮还专门实测过 R12）。我第一版每发一个包
   `socket()` 一次 ⇒ connect 成功、announce 全部「connection_id 无效」⇒
   B2/B3 当时**双双显示 PASS**，看着像「UDP 也拒了假 left」。修法：
   会话 = 一个 socket；并且加 **B2a 正控**（同一会话里合法 announce 必须 `ok`），
   正控不过就整组作废。
2. **待审种子要先建立 swarm**。C 组第一版在 `owner_only`（上一轮残留的配置）下
   让「非发布者」去做种 ⇒ 那条 announce 直接被拒，swarm 是空的 ⇒
   「scrape 不外发」的断言拿着空表比空表，判成 PASS。修法：C0 正控改为
   **allow_all 档下四个口都必须看得见**，C1/C2 只在正控成立时才采信；
   并把做种者改成本身就是发布者的账号（`UPDATE torrents SET owner_id`）。
3. **佐证是异步落账的，读早了会把「还没轮到」当成「挡住了」**。
   worker 的消费是分钟级批处理（实测 `snatches.last_seen_at` 全落在整轮时刻上），
   D 组第一版在 xreport 之后**立刻**读 `upload_corroborated`（还没落）
   再让 A 重发 ⇒ `seeding` 仍是 f ⇒ D1/D2/D3 三条全绿。修法：
   先 poll 到 `upload_corroborated` 真的入账，**再**让 A 重新 announce，
   并把 A0/A1 的轮询预算从 40 s 提到分钟级。顺带这条也给所有
   「事件流 → worker → 表」类断言定了新规矩：**先确认落账，再判拒否**。

（另记一处产品侧自我纠正：§八-6 那条我实现后又撤掉的「最短间隔直接拒」。）

## 十二、清理（已全部归零）

探针资产：18 个账号（`r5******a/b`）、种子 `R5Credit / R5Pending / R5Corr /
R5CorrOne / R5Big`，以及 `snatches / traffic_ledger / upload_corroborated /
xreport_credited / leecher_xreports / cheat_events / torrent_files` 关联行。
`announce_pending_policy` 已复原为 `self_seed_only`；本轮没有新建侧容器
（全部在主栈 7070/6969 上打），`docker ps` 只剩主栈 6 个容器。

**清理本身学到的三条**（已写进 `.workbuddy/_r5_cleanup.py` 与
`_r5_tombstone.py`，别再按 10-07 那份报告的口径重做一遍）：

1. **本站的用户删除是墓碑化，不是删行**。`admin_http/user_del.rs:137` 写着
   「刻意不动 audit_log：actor_id 继续指向该账号」；实测墓碑化会把
   `username` 改成 `deleted-<id>-<8hex>`、`passkey` 换成 `deleted…` 前缀、`status=3`。
   ⇒ 两条看着合理的归零判据其实都是错的：
   · `count(*) FROM users WHERE username ~ '^r5…'` 一定归零，因为名字已经不是
     r5 了——**假通过**；
   · `user_by_passkey` 里还剩 36 行也不是没清干净，视图本来不过滤 status
     （门槛在 tracker 那句 `AND status < 2`）。
   正确的判据是**「还能不能被解析成可用账号」**：
   `count(*) … WHERE status < 2 = 0`，且墓碑行数 = 探针数，两个一起看。
2. **裸 `DELETE FROM users` 走不通**：先被 `messages` 顶回来
   （`cheat_enforce` 的 L1 站内信就是这条外键的来源），清了 messages 又被
   `audit_log` 顶回来——而 audit_log 是产品**有意保留**的。
   所以必须走 API 的 `status=2 → DELETE /admin/users/{id}`，不是 psql 硬删。
   第一版脚本因此在库里留了 18 个半死账号，重跑才对。
3. 顺手清掉一条**上一轮遗留**的 `upload_corroborated` 行
   （`uploader_id=1(root) / torrent_id=42508 / bytes=2 GiB`）——
   10-07 报告写「已全部归零」时，0300 这张表还不在清理清单里。
   教训：**新增一张计费相关表，老的清理脚本不会自动覆盖它**，
   归零核对的清单必须跟着表结构一起长。
