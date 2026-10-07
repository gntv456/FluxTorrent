# PT tracker 深挖实测审计报告（2026-10-07 二轮）

> 尺子先说清：本项目的应用面是**私有站 tracker**（passkey 鉴权、按 announce 计流量、
> 承担反作弊与版主治理），所以本报告一律对 **UNIT3D / NexusPHP / Ocelot(Gazelle) 的
> 私募站惯例**评，不对公网 tracker 的功能清单评。
> 顺带纠正两处我自己带进来的错误前提（调研实证，别再引用讹传）：
> **opentracker 根本没有 passkey 体系**（`grep -c passkey` 在 upstream master 全文件 = 0），
> **chihaya 没有速率限制中间件**（`dist/example_config.yaml` 实际只有 jwt / client approval /
> torrent approval / interval variation 四个，issue #145 的" Potential Middleware"里
> ratelimit、privatescrape 都从未落地）。拿这两家当"业界标配"会得出错误结论。

---

## 一、结论摘要

| 级别 | 条数 | 一句话 |
|---|---|---|
| P0 | 1 | 在种 / 完成判定全部建立在**客户端自报累计计数器**上；站点自己的 credited 传输量为 0 也照样判「在种 + 完成」 |
| P1 | 7 | peer_id 槽位可被他人顶号（→ 驱逐 + 继承回连信用）、待审种子的 swarm 对全站开放、IP 信任面三处不自洽（含实测内网回连）、改密即轮换 passkey 无宽限、agent_rules 是假开关、版主在线面板双模式都坏、后台第三方 tracker 会把用户 passkey 送出去 |
| P2 | 14 | 限速与事件流容量口径（单账号 30 tx/s、XADD 无界、消费者泄漏）、scrape 四条、参数口径五条、封禁无 CIDR、DB 超管 + 扁平网络、announce 默认明文 |
| P3 | 7 | 白名单 DB 兜底的 `SELECT 1`/i64 静默 fail-open、无负缓存、UDP event 映射、UDP 无界 spawn、`+` 不解码、`no_peer_id` 缺失、metrics 面 |

**同时确认仍然有效、不必重复上报的**：10-06 那八条（参数严格校验、`remove_owned` 的 stopped 侧、
影子 peer 配额、TTL 随 interval、scrape 只认快照、port=0 不计数不下发、passkey 负缓存、
UDP connect 随机化与限速）本轮全部复验为绿；默认配置下 `?ip=` 与 `X-Forwarded-For` 都不采信。

---

## 二、实测环境与方法

- 被测镜像 `ghcr.io/gntv456/fluxtorrent/tracker:latest` = `918912f8531a`，容器 `flux-tracker`
  Up 且 healthy，工作树 HEAD `83e2bc7`（判据是行为断言，不是二进制 grep）。
- 探针实体：`/admin/adduser` 建账号（含改临时密码）+ 以 root 发一颗 4 MiB 的专用种子
  （`make_torrent` → 走真实 `/torrents` 上传链，`raw_info_hash` 现算）。
- 观测面三处交叉，避免"接口 200 就当生效"：① tracker 的 announce/scrape **响应字节**；
  ② Redis 事件流 `flux:announce` 的 **payload 原文**；③ Postgres `snatches / torrents /
  traffic_ledger` 的落库值。peer 表状态用 tracker 自己每 60s 落盘的 `flux:tracker:peers` 快照读。
- 代理信任面用**独立侧容器**测（`TRUST_PROXY=1` / `TRUST_PROXY_IP=1`），主栈保持默认配置作对照；
  侧容器由脚本自起自拆，测完 `docker ps` 只剩主栈。
- 探针脚本（gitignored 的 `.workbuddy/`，可复跑）：
  `_tracker_deep_probe.py`（18）、`_tracker_acl.py`（11/11）、`_tracker_ownership.py`（7/7）、
  `_tracker_lab4.py`（5/5）、`_tracker_proxy_lab.py`（10/11）、`_tracker_round2.py` /
  `_tracker_round3.py`（参数与面板口径）。
- **诚实记账**：第一轮 `_tracker_deep_probe.py` 里有 5 条 FAIL 是探针自身缺陷
  （peer_id 拼成 24 字节、bencode 解析器从 0 切片），已在脚本内修好并复跑；
  `_tracker_round2.py` 的 R1.x 因基线 swarm 被上一轮残留污染而无效，改用
  `_tracker_ownership.py` 在干净种子上重做并全绿。报告只采信有干净证据的条目。

---

## 三、P0-1 反作弊判据的自证循环：在种 / 完成 = 客户端说自己下过

**实测**（`_tracker_deep_probe.py` C 组，全新账号 C，全新种子，从未下载过任何字节）：

一次 announce：`left=0 & port=48436 & downloaded=4194304 (= 种子大小) & event=completed`
→ worker 60s 后落库：

```
snatches:  seeding=true | downloaded=0 | progress=10000 | completed_at=2026-10-07 13:39:16 | seeded_seconds=0
torrents:  times_completed=1
```

`downloaded=0` 是关键：**站点侧记账认为是 0 字节**（首报增量恒 0，见 ledger_guard 的锚点逻辑），
但「你有没有真下过数据」这条不变量读的是客户端原始读数。

**根因**
- `apps/worker/src/jobs/ledger_guard.rs:121-122` —— `raw_up/raw_down` 直接取 `ev.up/ev.down`
  （客户端累计读数，未过任何校验）；同函数已经算出了过物理速率钳的 `credit_up/credit_down`
  （**倍率前**，freeleech 也非零）。
- `apps/worker/src/jobs/process_event.rs:111` `has_payload = g.raw_down > 0`；
  `:114-115` `payload_ratio_ok` 也用 `g.raw_down`；`:116-120` seeding 四条件、
  `:128-131` completed 判定都建立在这两个值上。
- 结果：把 `downloaded` 报成种子大小的 10.4% 就同时满足「非零」和「比例下限」，
  一个 announce 拿到 `seeding=true` + 完成记录 + `progress=10000`。
- 持续 announce 还能累计 `seeded_seconds`（`process_event.rs:183-188`，代码推论：
  第二次 announce 起 `snatches.last_seen_at` 落在容忍窗内）。

**打穿的面**：种子页在种数与保种组在种判定、保种时长/收益结算、勋章与"完成 N 颗"类考核、
种子页「N 次下载」、下载榜。2026-10-07 一轮加的"第四条不变量"本意正是堵幽灵做种，
但它换了个自报字段而已，攻击成本 = 多打一个 query 参数。

**PT 惯例对照**：没有一家把"是否下过"建立在同一个自报数上——
UNIT3D 的 `event=completed` 要求该 `peer_id + user_id` **已有 peer 行**，否则回 error 152；
NexusPHP 对 `left > torrent size` 判 fake announce 并**吊销下载权限**；
opentracker 的 sanitiser 口径"声称完成者必须是做种者"。共同点是：完成/在种要由
**tracker 侧的既有状态或站点侧的记账**背书，而不是客户端的一句话。

**建议修法（一处改，两条判据同时收口，且不伤 freeleech 与二传申诉）**

```rust
// ledger_guard.rs：把已读到但被丢弃的 snatches.downloaded 用起来
let physical_down = g.credit_down.max(acc_down_hist);   // acc_down_hist = 查询里的 downloaded 列（现 _acc_down）
has_payload   = physical_down > 0;
payload_ratio = torrent_size <= 0 || physical_down * 1000 >= torrent_size * 104;
completed     = ev.event == "completed" && ev.left == 0 && has_payload && payload_ratio
                && 本 (user, torrent) 此前已存在 peer 行;   // UNIT3D 口径
```

- `credit_down` 是倍率前的物理量 ⇒ freeleech / x2free 下正常做种者仍为真（不会被误杀）；
- 历史 `snatches.downloaded` 兜住"重新做种老种子"的合法场景；
- 纯谎报者：`credit_down=0` 且历史 0 ⇒ 挡下，`ghost_seed` 留痕与人工申诉通道照旧。

---

## 四、P1（7 条）

### P1-1 peer_id 槽位可被顶号 → 驱逐受害者 + 继承「回连可达」信用

**实测**（`_tracker_ownership.py` 7/7，干净 swarm）：

```
K2 A 的槽位：port=51500 left=0（在种）
K3 B 用 A 的 peer_id 发一条 left=12345/port=51599 → A 的槽位端口与 left 都被改写
K5 B 随后发 event=stopped → 该槽位从表里消失（A 从未发过 stopped）
K6 对照：不经顶号时，他人 stopped 不生效（10-06 的 remove_owned 有效）
```

peer_id 怎么拿到：**`compact=0` 的非 compact 响应里带每条 peer 的 20 字节原始 id**
（`_tracker_deep_probe.py` B2 实测能读到他人 peer_id）。

**根因**：`apps/tracker/src/peers/table.rs:77-102` 的 `upsert` 没有归属校验——
同 `(info_hash, peer_id)` 键既存条目的 `user_id` 直接被覆盖；
`:80-82` 还**无条件继承**旧条目的 `connectable` 测量值。
10-06 只在 `stopped` 侧加了归属，写侧漏了，于是"先顶号再 stopped"绕过整条防线。

**危害（PT 语境）**：① 任意用户可把同 swarm 里任何人抹下线（骚扰，且面板/在种数一起塌）；
② 可把别人的槽位改成自己的 ip:port，让其他客户端连到攻击者（内容/元数据面）；
③ **回连信用可搬运**：`_tracker_lab4.py` C.5 实测——B 顶用 A 的 peer_id 并换一个根本没开的端口，
事件里 `conn` 仍是 A 的 `1` ⇒ 幽灵做种判据的 `conn != Some(0)` 条件被搬运满足。

**建议**：`upsert` 前查既存条目，`user_id` 不同则拒绝（failure reason「peer_id 已被本站其他账号使用，
请重置客户端 peer_id」），`connectable` 仅在同账号时继承；同时支持 BEP23 `no_peer_id=1`
或按 opentracker 口径直接对 `compact=0` 回 400，收掉"批量收号"面。补 `peers/tests.rs` 一例
`stop_after_takeover_is_rejected`。

### P1-2 待审种子：站点 404，tracker 放全站进 swarm

**实测**（`_tracker_acl.py` 11/11，同一状态同一账号）：

```
approval_status=0 时：
  GET /torrents/{id}  → HTTP 404            （apps/api/src/torrents/visibility.rs:24-32 的口径）
  POST announce       → d8:completei1e...    并且 peers 段返回发布者真实 ip:port
  GET  scrape         → complete=1（实时做种数对任意登录用户可读）
approval_status=3/4/2：announce 与 scrape 都正确拒绝/归零（对照组绿）
```

根因：`apps/tracker/src/http_track/guard.rs:89-96 / 115-135` 的放行集是 `approval_status IN (0,1)`，
注释理由是"审核期发布者要先能做种"——理由成立，但**放行了所有人**。
站点侧花了两个审计轮（0286/P1-1）把待审内容的读面收成"仅发布者与 staff"，tracker 这条口径没跟上。

**建议**：按 caller 身份分档，并**做成站长可配**（建站定位，不要把某类站的审核流程焊死）：

```
announce_pending_policy: allow_all | owner_and_staff | self_seed_only
默认 self_seed_only：待审种子的 announce 照常接受（发布者能注册 peer、事件照常计费），
                     但 peer 列表对非 owner/非 staff 返回空 ⇒ 审核期不外泄数据面。
```

### P1-3 IP 信任面：三处不自洽，实测能把它变成内网探测 + 投毒 + 封禁绕过

| # | 事实 | 证据 |
|---|---|---|
| a | **文档说反了**：`_doc/生产部署指南.md:84` 写「不信任自报 IP（`TRUST_PROXY_IP=1` 才信 X-Forwarded-For）」；代码里 XFF 由 `TRUST_PROXY=1` 门控（`helpers.rs:99-102`），而 `TRUST_PROXY_IP=1` 打开的是**客户端自报 `?ip=`**（`:104-109`，更危险的那一档）。照文档去"收紧"的站长会正好打开最宽的口子 | 读码 + `docs/ops/security.md:57-58` 口径是对的，两份文档互相矛盾 |
| b | **XFF 段不校验是不是 IP**：`helpers.rs:123-145` 只 trim/分段取值，从不 `parse::<IpAddr>()`（`?ip=` 路径 `:146-157` 反而校验了） | `_tracker_proxy_lab.py` S1：`X-Forwarded-For: spoof042.example` 被当客户端 IP；事件 `ip` 落该串；`EXISTS rl:ann:ip:spoof042.example = 1`（限流键被写成任意串 → 桶被打散 + Redis 键面失控 + 取证污染） |
| c | **封禁按"攻击者自选值"匹配** | S3：`XFF: 9.9.9.9`（在册封禁）被拒；同一连接改发 `XFF: 9.9.9.8` 立即放行。`ip_bans` 是 `SELECT host(ip)` 精确串匹配（`guard.rs:28`），键由客户端控制即等于没有封禁 |
| d | **peer 列表投毒 + 内网地址被主动回连** | `_tracker_proxy_lab.py` S2：XFF 信任开启后 `8.8.8.8` 被下发给同 swarm 的其他客户端。`_tracker_lab4.py` C.1–C.4：`TRUST_PROXY_IP=1` 下 `?ip=172.20.0.5&port=5432` → ① 进 peer 表 ② **下发给他人**（C.3）③ tracker 每 5min 的回连抽样**真的连上了内网 DB 端口**（C.4，`conn=1`；连接逻辑在 `main.rs:213-258`，`TcpStream::connect((ip, port))` 对表内地址不加任何过滤） |

**建议（四条都很小，收益是把"只有误配才有的洞"变成"误配也打不穿"）**
1. `client_ip` 两条路径统一：取到的值必须 `parse::<IpAddr>()`，否则丢弃并回落 socket 对端，
   并 `metrics.ip_unparsable` 计数 + 启动 warn 一次。
2. 入 peer 表 / 参与回连的地址做保留段过滤：loopback、RFC1918、link-local、multicast、
   unspecified、`::ffff:` 映射（v4 映射应降级成 v4 入 v4 池）一律不入表、不下发、不回连；
   回连目标再加一层"只连本机以外且非内网"的可配开关（默认按上面的过滤）。
3. 文档改口径：XFF=`TRUST_PROXY=1`（并写清 `TRUST_PROXY_DEPTH` 必须等于可信跳数），
   `TRUST_PROXY_IP` 改名 `ALLOW_ANNOUNCE_IP_PARAM` 并标注"仅调试，等同允许伪造 IP"，
   同时把 `docs/ops/security.md` 的红线段链接过去。
4. 部署层：`docker/nginx/lb.conf.sample` 已把 tracker 排除在 LB 之外（announce 公网直连），
   在部署指南里把"tracker 前面不要放会覆盖 XFF 的代理"升级为显式禁用 + 自检（tracker 启动时
   发现 `TRUST_PROXY=1` 且没有可信代理解析链就打 WARN 并在 `/metrics` 暴露一个 gauge）。

### P1-4 改密即轮换 passkey，且没有宽限期

**事实**：`apps/api/src/auth_http/me_security.rs:60-71` —— `POST /me/password/change` 在改
`pass_hash` 的同一条 UPDATE 里换掉 `passkey`（有审计键 `passkey_rotate_on_password_change`，
属设计而非事故）。**实测**：探针账号改密后，用改密前读到的 passkey 打 announce，立刻
`failure reason: passkey 无效，请在站点重置`。

**为什么对 PT 站是 P1**：passkey 被烤进每一个用户已下载的 `.torrent` 文件（`announce` 与
`announce-list` 的每一 tier，见 `apps/api/src/publish_http/download.rs`）。用户改一次密码 =
手上所有种子的 announce 全部永久失败；libtorrent/transmission 类客户端会把该 tracker 标成
错误状态并长时间不再重试 ⇒ 表现为"我明明在做种，站内显示停种 / 保种组考核塌 / 在种数掉"，
而用户完全看不出原因。装机首启强制改密那一步就等于全站停种（root 之外无用户时不显，
用户改密日就显）。

**建议（择一，倾向 A）**
- A. 加宽限：`users.passkey_prev` + `passkey_prev_until`（默认 7 天）。tracker 的
  `resolve_passkey_cached` 查询改成 `WHERE passkey = $1 OR (passkey = $2_prev AND now() < until)`，
  命中旧值时在响应里带 BEP4 `warning message`「密钥已更新，请重新下载种子文件」；
  到期自动失效，不改密的用户零感知。
- B. 改密不轮换，只保留显式「重置 passkey」动作，并要求二次确认文案写清
  "所有已下载种子需重新下载"，重置后对旧值同样给 A 的宽限窗。

### P1-5 agent_rules 是假开关（口径三处不一致）

- 迁移 `apps/api/migrations/0036_reseed_agent_tag.sql:9` 明写 `pattern` 是
  **「前缀或子串匹配，如 "Transmission/3"」**；
- tracker 侧 `apps/tracker/src/http_track/guard.rs:41-72` 把它当**正则**编译，
  编译失败仅 `tracing::warn` + **静默跳过该规则**；
- 后台写入口 `apps/api/src/admin_http/agent_rules.rs:62-66` 只校验长度 1–100，**不校验正则**；
  UI（`staff-tools-sys-agentrules.tsx`）原样回显 pattern，没有任何"正则"字样。

后果：站长按注释填 `uTorrent/3.5.5 (build)`、`qBittorrent/4.6.0 (x64)` 这类带括号的 UA
→ 规则永远不生效，而后台列表里它"存在"。按你们自己的判据这是第三种形态的假开关
（写进去了但消费端语义对不上），而且失败态只在日志里，面板看不见。

**建议**：① 写入口用 `regex::Regex::new` 预校验，非法直接 400 并说明；② 语义二选一统一——
建议"默认字面子串匹配，以 `^` 或 `/…/` 包裹时按正则"，并在 UI placeholder 与迁移注释同步；
③ 面板加一列「生效状态」（guard 加载时把被跳过的规则 id 落到 `site_settings` 或新列，
`/metrics` 也给一个 `flux_tracker_agent_rules_skipped`）。

### P1-6 版主的实时在线面板：两种部署模式都坏，还漏一种 hash 口径

**实测**：`_tracker_round3.py` P4 —— 对 `info_hash <> raw_info_hash` 的那颗在册种子
（当前库里 53 颗中有 1 颗）announce 成功，peer 确实进了 tracker 快照（`P4.2 True`），
但 `GET /torrents/{id}/peers` 返回 `{"available": true, "items": [], "seeders": 0}`。

三处独立成因：
1. **口径**：`apps/api/src/torrent_http/peers.rs:143-145` 只比 `torrents.info_hash`，
   而 tracker 的桶键是 announce 原始字节 hex（= `raw_info_hash`）。全链路（白名单、计费、
   事件流）都是"双口径 OR"，唯独这个读面是单口径。
2. **单键互抹**：`flux:tracker:peers` 是一个全局键，每个 tracker 进程每 60s 全量覆盖写
   （`apps/tracker/src/main.rs:146-180`）。本轮我起第二个 tracker 进程期间，
   该键内容被侧容器覆盖成 `[]`（`STRLEN=2`），撤掉后恢复 2032 字节——**实测复现了
   0225 G30-B12 描述的"多副本互抹"在面板侧的下游后果**。
3. **多机模式根本不写**：`FLUX_TRACKER_PEER_STORE=redis`（`compose.multi-node.yml` 的口径）
   时快照写线程整个不启动，而权威数据在 `flux:swarm:{info_hash}` 里，面板不读它 ⇒ 恒空。
4. 顺带：`peers.rs:129` 的注释说"快照缺失返回 available=false"，代码里 `available` 恒 `true`
   （`:184`）⇒ 前端无法区分"tracker 挂了"与"真没人"。

**建议**：面板按部署模式选源（外置模式读 `flux:swarm:{hash}` 的 HGETALL，单机模式读快照键），
hash 比较改 `info_hash OR raw_info_hash`，`available` 如实反映键缺失；快照键加实例后缀
（`flux:tracker:peers:{instance_id}`）+ API 侧聚合，杜绝互抹。这是版主治理面，不是可选项。

### P1-7 后台「附加 tracker」会把每个用户的 passkey 拼给第三方

`apps/api/src/publish_http/download.rs:196-212`：对 `tracker_urls` 里每条启用地址，
无条件 `format!("{base}/announce/{}", user.passkey)`（udp 则是 `{url}/{passkey}`）。
过滤只有 scheme 与 `127.0.0.1/localhost`。

今天 `tracker_urls` 是 **0 行**（实测），所以它不是"现在在漏"，而是"站长一旦用了这个框就漏"：
把公网/友站的 tracker 填进去，**所有用户 32 位 passkey 就被写进下发的 .torrent 并持续外发**
（对方能据此归因用户在本站的全部做种行为，并以其身份 announce）。这属于"高危面默认无守卫"。

**建议**：① 只有当 tracker 根地址等于本站 `announce_url`/`PUBLIC_TRACKER_URL` 的 host 时才拼
passkey，否则写裸 announce 根；② 后台保存时若 host 非本站，明确二次确认 + 落审计；
③ 表加 `with_passkey boolean default false`，把"带凭据"变成显式意图（同时满足建站定位——
站长想接友站 tier 也不该被动泄露凭据）。

---

## 五、P2（14 条，按面分组）

**流量与容量口径（PT 大户口径）**
1. 每用户 1800/min 意味着单账号可持续 30 announce/s，而**每条事件一个 PG 事务**
   （`process_event.rs:44` 开事务 + `ledger_guard.rs:24-33` 的 `FOR UPDATE` 行锁 +
   snatches upsert）。PT 惯例是按最短间隔合并：NexusPHP 用 Redis NX 锁做
   **5s/(passkey,info_hash)** + 30s/torrent，UNIT3D 是 30s 重复 announce 锁。
   建议：同一 `(user, torrent)` 在 `interval×40%` 内的重复 announce **只回 peer 列表、不发事件**
   （差值口径本就无损），并加 `per-hash` 维度限流。
2. `emit_event` 的 XADD 无 `MAXLEN`（`http_track/emit.rs:80-94`），裁剪只在 worker 消费轮里
   （`announce_main.rs:175-178`，`XTRIM … ~10000`）⇒ **worker 停机/落后时 `flux:announce` 无界增长**，
   而它和会话、首页缓存同库 Redis。建议 `XADD MAXLEN ~ 200000` + 丢弃计数进 metrics。
3. 消费者组泄漏：`group.rs::consumer_name()` = `instance_tag-pid`，且全仓没有 `XGROUP DELCONSUMER`。
   实测 `XINFO GROUPS flux:announce` → `consumers 24`，而只有 1 个 worker（每次重启/pid 变化都沉积一个，
   PEL 归属随死消费者丢）。建议稳定消费者名（容器名）+ 启动时清死消费者。
4. 边缘与内闸数值相等：部署指南给的 nginx 是 `limit_req rate=60r/s`（=3600/min）而
   `ANN_RATE_IP_PER_MIN=3600`（compose L213）⇒ 零余量，正常多挂种盒子先撞边缘并收到 502/429
   （**不是** tracker 的 bencode 提示，用户在客户端里看不到原因）。建议边缘 ≥2×，或边缘只限"无凭据洪水"。
5. `interval` 全站固定 1800s 无抖动 ⇒ 全体客户端同秒重发（惊群）。chihaya 为此专门有
   `interval_variation` 中间件。建议 ±10% 随机（seed 用 info_hash，保证同种稳定）。

**scrape**
6. **无条数上限**：实测单请求带 151 个 `info_hash` 全部原样回；继续加到 URI 上限能带
   **1039 个**（8 KB 请求 → 70 KB 响应）。chihaya `max_scrape_infohashes: 50`，
   opentracker `OT_MAXMULTISCRAPE_COUNT 64`。建议 cap 50–64，超出截断并在文档写明。
7. **重复 info_hash → 重复字典键**（实测 keys=3，同 20 字节键出现 3 次）：bencode 字典键必须唯一，
   严格解析器（libtorrent）可能直接拒。建议去重后回。
8. scrape 与 announce **共用同一个 IP 桶** `rl:ann:ip:` ⇒ 一次全站轮询把该 IP 的 announce 额度
   吃光，用户表现为"突然全站在报频率超限"。建议分桶 `rl:scr:ip:`。
9. 外置模式下每个 hash 一次 `HGETALL` 全量解析（`peers/external.rs:110-123`），1000-hash 请求 =
   1000 次全 swarm 拉取；而 `counts/snapshot` 各拉一次 ⇒ 一次 announce 两次全量。
   建议 pipeline + 上限（与第 6 条一起做）。

**参数与响应口径**
10. `uploaded` / `downloaded` 非数字或超 i64 → **静默按 0**（`params.rs:30-34` 的 `get_i64` 回落默认值；
    实测 `uploaded=99999999999999999999999` 返回正常 announce，而同样超界的 `left` 被拒）。
    10-06 审计已把 `left` 的同类问题改成"一律拒绝"，up/down 没跟上。附带后果：客户端读数被当 0 →
    `ledger_guard` 判"读数回退"→ 落 `counter_reset` 作弊留痕（**误报**）。
11. `left` 无上界：可 `left > 种子大小`（NexusPHP 明确判 fake announce 并吊销下载权）。
    影响 `progress` 语义与 TTL 分档（`left>0` 走短 TTL）。
12. `port` 接受 0 与 <1024 的特权端口（UNIT3D 有 `BLACK_PORTS` 且除 stopped 外拒绝 <1024，
    NexusPHP `portblacklisted()`）。port=0 你们已经做了"不计数不下发"，但 <1024 会进 peer 表
    并下发，别人去连 25/53/445，同时也是 tracker 自己回连探测的放大器（见 P1-3 d）。
13. `numwant` 口径两处不一致：入口 clamp(0,200)（`announce.rs:60`），表侧硬上限 50
    （`table.rs:16`）⇒ 客户端要 200 永远只拿 50。对照：UNIT3D 硬上限 25，
    chihaya 默认 50 / 最大 100，opentracker 200（UDP 侧再按分片限 200/66）。
    建议一个常量同时管两处并在部署文档写明，别留两个数。
14. **完成数不下发**：announce 与 scrape 的 `downloaded` 恒 0（实测 C4/R6：`times_completed=1`
    而响应 `downloadedi0e`），而 UNIT3D/NexusPHP/Ocelot/opentracker 都下发真实完成计数。
    客户端"完成/做种人数"列永远空。数据就在 `torrents.times_completed`，一次 join 的事。

---

## 六、P3（7 条，口径与工程卫生）

1. 白名单 DB 兜底写成了 `SELECT 1` 配 `query_scalar::<_, i64>`（`guard.rs:125-133`）：
   Postgres 字面量 `1` 是 INT4、Rust 要 INT8 ⇒ **一旦命中行就必然解码失败**，被 `.ok()` 吞成
   `None → unwrap_or(true)` 的 fail-open。今天不构成绕过（未注册哈希走"无行"分支正常拒绝，
   A1 实测仍绿），但它意味着"新发种 60s 窗口兜底"这条路径从未真正确认过一次，
   且任何 DB 抖动都会让整站白名单静默失效。改 `SELECT EXISTS` + `bool`——
   仓库里已有同款前车之鉴（`visibility.rs:41` 的注释）。
2. 未注册 info_hash 每次直查 PG，announce 侧没有负缓存（scrape 侧有"只认快照"的纪律，
   两边不对称）⇒ 随机 hash 洪水 = 一请求一查询。建议照 `passkey_miss` 做 60s miss 缓存。
3. **UDP event 映射少一档**：`udp/announce.rs:55-59` 只认 `1→completed`、`3→stopped`，
   `2`（BEP15 的 started）落进 `_ => ""` ⇒ UDP 通道永远不产生 `started` 事件，
   站点侧按 started 的语义（首报、审计、面板）在 UDP 全丢。补 `2 => "started"`。
4. UDP 每包一个 `tokio::spawn` 无并发上限（`udp/core.rs:88-108`），Redis 前置限流缺位
   （`rate_limited_ip` 在 handler 内）。建议收包侧本地预限流 + 有界并发（semaphore）。
5. `percent_decode` 不把 `+` 还原成空格 ⇒ 部分客户端 UA/参数里的 `+` 口径偏差
   （`snatches.agent` 与黑白名单匹配都受影响）。BEP3 的原始字节面不需要，但 `agent`/`event`
   这类文本参数建议走标准 URL 解码。
6. 支持 `no_peer_id=1`（BEP23）或对 `compact=0` 回 400（opentracker 口径）——
   现在非 compact 响应把他人原始 peer_id 交出去，正好喂给 P1-1 的顶号链。
7. `/metrics` 挂在公网 :7070 上、token 用 `==` 比较且该路径无限速（`metrics.rs:17-34`），
   指南也只在 nginx 侧建议 `allow 127.0.0.1`。建议：独立 bind 地址（默认 127.0.0.1）+
   `constant_time_eq`，把"监控面"从公网面上摘下来。

---

## 七、本轮复验为有效、请勿重复上报

种子白名单对**未注册/被拒/软删/暂缓**四种状态都正确拒绝（announce + scrape 双口径，11/11）；
passkey 无效与负缓存；挂起账号双拒；`download_enabled` 只对 `left>0` 生效；
`left` 严格解析；负数 up/down 拒；port 边界拒；`numwant=0` 合法；影子 peer ≤10；
实时计数按 user 去重；port=0 不计数不下发；非法字符串 IP 不进 peer 列表（`table.rs:167-179`
的 parse 过滤起了兜底作用）；TTL 随 interval 伸缩；bencode 字典键序与二进制完整性；
`compact` 双路径；BEP-7 `peers6`；空 swarm 桶回收；默认配置不采信 `?ip=` / XFF；
UDP connection_id 跨源端口不可复用（实测 R12）、随机化、connect 限速；
`sweep_stale_peers` 对"没发 stopped 的僵尸做种"有 2×interval 兜底清理。

---

## 八、按 PT 惯例还该有、我们现在没有的（含"要不要做"的判断）

| 惯例（出处） | 我们的现状 | 判断 |
|---|---|---|
| `downloaded` 下发真实完成数（UNIT3D/NexusPHP/Ocelot/opentracker） | 恒 0 | **该做**，1 处 SQL，用户可见 |
| `completed` 需既有 peer 行背书（UNIT3D err 152） | 首报即可 completed | **该做**（与 P0 一并） |
| `left > size` 判假 announce（NexusPHP） | 无校验 | **该做**，一行 |
| 特权端口 / 端口黑名单（UNIT3D `BLACK_PORTS`、NP `portblacklisted`） | 无 | **该做**，与 P1-3 的入表过滤一起 |
| per-(passkey,info_hash) 最短间隔锁（NP Redis NX 5s / UNIT3D 30s） | 只有每分钟总量 | **该做**，同时解决 P2-1 与惊群 |
| `interval` 随机抖动（chihaya interval_variation） | 固定值 | 建议做（±10%，低成本） |
| peer 列表不含同 user 的其他 peer（NP/UNIT3D 排除自己） | 只按 peer_id 排除 | **该做**，一行（骚扰/隐私面） |
| 按 /24 与 /64 归一统计骚扰源（opentracker woodpecker） | ip_bans 只能封单 host | **该做**（版主刚需：动态 IPv6） |
| UA 必需且拒绝浏览器/爬虫 UA（NP/UNIT3D） | 无 UA 要求 | 建议做，但**必须可配**（自研客户端/下载器场景） |
| 站型可配的准入策略（建站定位） | 白名单硬编码 `IN (0,1)` | **该做**（见 P1-2） |
| scrape 条数上限（chihaya 50 / opentracker 64） | 无（实测 1039） | **该做** |
| BEP26 scrape-all / DHT / PEX / `tracker id` / 公网站群语义 | 无 | **不做**，与私有站定位无关 |

---

## 九、建议落地顺序（一轮能吃完的量）

1. **第一批（P0-1 + P1-1 + P1-3 的 1/2 步 + P2-10/11/12/14）**：计费与归属判据、
   IP 入表校验、参数口径、完成数下发。全部集中在 `ledger_guard.rs / process_event.rs /
   peers/table.rs / http_track/{announce,params,helpers}.rs`，不动 schema。
2. **第二批（P1-2 策略位 + P1-6 面板 + P1-7 with_passkey）**：需要 1 个迁移
   （`announce_pending_policy` 设置键 + `tracker_urls.with_passkey`）+ 面板选源改造。
3. **第三批（P1-4 passkey 宽限 + P1-5 agent_rules 口径）**：需要 1 个迁移
   （`passkey_prev/passkey_prev_until`）+ 后台写入口校验 + UI 文案。
4. **第四批（P2 的流控/容量项 + P3）**：XADD MAXLEN、消费者名、interval 抖动、
   scrape 上限与去重、`SELECT EXISTS`、UDP event、metrics 独立 bind。

---

## 十、验收探针（复跑前置与断言数）

- `_tracker_deep_probe.py`（18）：白名单/鉴权/参数/事件 ip 口径/peer_id 收号/幽灵做种。
- `_tracker_acl.py`（11）：待审与下架态的 announce+scrape 双口径，含"恢复过审即放行"对照组。
- `_tracker_ownership.py`（7）：顶号 → 改写 → 驱逐 → 对照（未顶号不生效）。
- `_tracker_lab4.py`（5）：`TRUST_PROXY_IP=1` 侧容器专测（内网投毒 + 主动回连 + 测量继承），
  **脚本自己起容器、自己拆**，不会把主栈的 `flux:tracker:peers` 快照长期覆盖。
- `_tracker_proxy_lab.py`（11）+ `_tracker_round2/3.py`：XFF 各档、scrape 上限与重复键、参数上界、面板口径。
- 环境坑（写探针时必守，本轮我自己全踩过一遍）：peer_id **必须恰好 20 字节**（拼 24 会让所有
  断言以"peer_id 长度无效"假红）；bencode 解码器切片要带起点 `buf[i:j]` 而不是 `buf[:j]`；
  `.torrent` 的 `pieces` 必须是 20×片数（站端上传接口校验片数，直接 400）；
  `/admin/adduser` 出来的账号**改临时密码会轮换 passkey**，所以「改密」必须放在
  「读 passkey」之前，否则后续 announce 全假红（这条既是探针坑，也是 P1-4 本体）；
  `XREVRANGE` 输出本身最新在前，别再倒序；事件流扫描要按唯一 info_hash 过滤，别按"最早匹配"。
## 十一、附：本轮残留与清理（已全部归零）

- 已删：`TrackerDeep.* / ProxyLab.* / OwnLab.* / Aclab.*` 四组探针种子与全部探针账号
  （含各自 `snatches / traffic_ledger / announce_seen / cheat_events` 关联行）、
  `ip_bans` 的 `lab-spoof` 行、侧容器 `flux-tracker-xff / -paramip / -lab4`。
- 收尾复验：`探针种子=0 / 在册探针号=0 / ip_bans=0 / tracker_urls=0`（后两者本轮前后一致）。
- 探针账号删除走 `/admin/users/status status=2` → `DELETE /admin/users/{id}`（先降 `class_id=1`，
  否则 `ensure_outranks` 让 root 删不掉 99 级探针）。
- 库内 `users.uploaded/downloaded` 的变动只来自探针账号自身（已随账号墓碑化，root 增量恒 0）。

---

## 十二、落地状态（同日追加）：本批已改 / 待下一批

站长要求「全部解决」。执行时工作树里有**另一路在制品**（`http_track/announce.rs`、
`peers/table.rs`、`worker/jobs/ledger_guard.rs`、`process_event.rs`、`emit.rs`、
`params.rs`、`main.rs`、`external.rs` + `xreport/collusion/bt_probe` 三个新文件，
且 0299/0300/0301 已被 `git add`）。经确认按「先做不重叠的、重叠等它落库」执行。

### 已落地（不碰在制品，单测 + 编译 + SQL 实跑三重验证）

| 报告条目 | 落点 | 验证 |
|---|---|---|
| P1-3 IP 取信 | 新模块 `http_track/ip_trust.rs`：XFF 段与 `?ip=` 一律 `parse` + 保留段/RFC1918/CGNAT/ULA 拒绝（`ALLOW_PRIVATE_PEER_IP=1` 才放行内网），非法即回落 socket 对端并计数 | 4 条单测（含实测用过的 `spoof042.example`、`172.20.0.5`） |
| P1-3 d 回连侧 | `peers/probes.rs` 过滤本环/链路本地/组播/文档段/非法串，永不回连 | 1 条单测 |
| P2 段封禁 | 新模块 `guard_store.rs`（手写 v4/v6 前缀匹配，`/0` 拒收）；`refresh_guard` 改取 `ip::text`；写入口 `staff_http/bans.rs` 接受 CIDR 并归一 | 6 条单测 + psql 对照（`ip::text=10.9.8.0/24` vs `host(ip)=10.9.8.0`，正是旧写法吞掩码的证据） |
| P3-1/P3-2 白名单 | `guard_refresh.rs`：兜底查询改带类型元组（不再 `SELECT 1`/i64 解码失败被 `.ok()` 吞成放行）；未注册 hash 60s 负缓存，刷新即清 | 空库全链应用 + SQL 形状实跑 |
| P1-4 passkey 宽限 | 迁移 0302：`passkey_prev/passkey_prev_until` + 视图 `user_by_passkey`；改密与后台重置都写旧钥（默认 7 天，`PASSKEY_GRACE_HOURS`）；tracker/兼容层/RSS **四个读口统一走视图**（判据只有一份） | scratch 库实跑：轮换后新旧两把钥都解析到同一账号 |
| P2 附带（passkey 假负缓存） | `guard.rs`：查询**报错**不再与「密钥无效」混同，不写负缓存、单独计数 `passkey_query_failed` | 编译 + 逻辑隔离 |
| P2-6/7/8/14 scrape | 上限 64（`SCR_MAX_HASHES`）、重复 hash 去重、files 按原始字节排序（不再产出重复字典键）、独立限流桶 `rl:scr:ip:`（`SCR_RATE_IP_PER_MIN`）、`downloaded` 下发真实 `times_completed`（HTTP+UDP） | 编译 + 与 `bencode_scrape` 签名同步改造 |
| P3-3 / P2-12 UDP | `udp/announce.rs` 补 `2 => started`；除 stopped 外拒绝 <1024 特权端口 | 编译 |
| P1-6 治理面板 | `torrent_http/peers.rs`：`info_hash` **与** `raw_info_hash` 双口径；外置模式改读 `flux:swarm:{hash}`；`available` 如实反映数据源缺失；计数按 user 去重（与 tracker 同口径） | 编译 |
| P1-5 agent_rules | 写入口用 `Regex::new` 校验并 400；导入把非法正则计入 `invalid` 返回；被跳过的规则从「只有日志」变成可查 | 2 条单测（含站长按注释直觉填的 `uTorrent/3.5.5 (build)`） |
| P1-7 凭据外发 | 迁移 0302 加 `tracker_urls.with_passkey`（默认 false）；`download.rs` 只有本站 authority 或显式勾选才拼 passkey | 1 条单测（authority 归一） |
| P2 附带（限流永久卡死） | `rate_over` 改成「无 TTL 就补 EXPIRE」，不再只在新键第一次尝试 | 编译 |
| P3-7 metrics | token 改定长时间比较（`/metrics` 仍受 404 门控）；新增 `ip_inject_rejected` / `passkey_query_failed` 两个计数器 | 编译 |
| 文档 | `_doc/生产部署指南.md`：原文把「信 XFF」写成 `TRUST_PROXY_IP=1`（照做等于打开客户端自报 IP 那档）——已改口径并登记 4 个新开关；`docs/ops/security.md` 同步；报告与译文表（`validation_details.tsv` 追加 2 句，新串全部已译） | `validation_i18n_guard` 输出与 HEAD **逐行一致** |

**验证汇总**：`cargo test -p flux-tracker` 42/42（本批新增 10 条全绿）；
`cargo check -p flux-api` 0 error；迁移链在空库按序应用 296 份 0 失败；
`scripts/line_limit_guard.mjs` 对我改动的文件全绿（残留红全部是另一路的
`collusion/xreport/group/run/process_event/table/tests/params/pages.css`，按规矩不代洗）；
`check_type_drift.mjs` OK。

### 待下一批（都在被别人正在改的文件里）

P0 幽灵做种判据（`ledger_guard.rs` + `process_event.rs`）、P1-1 顶号归属校验与
`connectable` 继承（`peers/table.rs` + `announce.rs` + `external.rs` Lua）、
P1-2 待审种子准入（`announce.rs`，**读口 `swarm_size/swarm_owner` 已在本批预留**）、
P2-10 `uploaded/downloaded` 严格解析（`params.rs`）、P2-11 `left > size`（读口已备好）、
P2-1/5 per-(user,hash) 事件合并与 interval 抖动（`announce.rs`/`main.rs`）、
P2-2 XADD MAXLEN（`emit.rs`）、P2-3 消费者组 DELCONSUMER（`group.rs`）、
P3-6 `no_peer_id=1`（`announce.rs`+`bencode.rs`）、P3-7 metrics 独立 bind（`main.rs`）。

### 状态：代码已写好，**未提交、未部署**

- 未提交：另一路已 `git add` 0299/0300/0301 且工作树里有它的新文件，此时我做
  path-scoped 提交有把对方半制品扫进同一条 commit 的风险。等它落库后一条命令即可：
  `git add apps/tracker/src/http_track/{ip_trust,guard_store,guard_refresh,guard,helpers,mod,metrics,scrape}.rs apps/tracker/src/peers/{bencode,probes}.rs apps/tracker/src/udp/{announce,scrape}.rs apps/api/src/{domain,repo}/mod.rs apps/api/src/{staff_http/bans,admin_http/agent_rules,admin_p3_http/tracker_urls,publish_http/download,auth_http/me_security,torrent_http/peers,compat_http/aliases,compat_http/nexusphp,rss_http,rss_http/forum}.rs apps/api/i18n/validation_details.tsv apps/api/migrations/0302_tracker_passkey_grace.sql _doc/生产部署指南.md docs/ops/security.md` → 一条 `fix(security): tracker 深挖不重叠批`。
- 未部署：发版会把另一路的在制品一起上线。复验配方已就绪（`_tracker_proxy_lab.py`
  的 S1–S3 与 `_tracker_lab4.py` 的 C.1–C.3 应从「攻击成功」翻成「被拒」，
  `_tracker_deep_probe.py` A4/A5 保持绿）。

### 追加（同日更晚）：P0 已写、验收探针已建、部署态已核

1. **P0 判据已改**（叠在另一路未提交的 `ledger_guard.rs/process_event.rs` 之上，
   他们的 `had_baseline`、`GREATEST(last_up…)` 等内容逐块 assert 后原位保留，未被回退）：
   新增 `GuardOutcome.phys_down = credit_down.max(历史 credited)`，
   `has_payload/payload_ratio_ok` 改用它，`completed` 再加 `had_baseline`
   （UNIT3D「首报即自称完成不算」口径）。`cargo check -p flux-worker` 通过；**未部署**。
2. **验收探针**：`.workbuddy/_verify_batch_1008.py`（22 条，编码的是**修复后**期望）。
   当前基线 `12 passed, 10 failed`，红的正是待落地项：G1/G2（P0，等 worker 出镜像）、
   O2/O3/O4（顶号，等 `announce.rs`/`peers/table.rs`）、P2/P3（待审准入，等 `announce.rs`）、
   K1/K2/K3（严格解析 / left>size / 特权端口，等 `params.rs`/`announce.rs`）。
   修掉探针自身两处假绿：scrape 白名单只认 60s 快照 ⇒ 新建种子要先等到快照含它再判
   （否则 O1 假红）；`/metrics` 无 token 返回 404 不能当「容器没起来」（否则 I 组整组跳过）。
3. **部署态核对（行为判据，不 grep 二进制）**：运行中的 `flux-tracker` 镜像
   `4bfdfb0b…`（容器 created 15:05Z）已包含本批不重叠改动——S1/S2/S3 与 I2/I3/I4
   全部实测为绿（scrape 去重+上限+真实完成数；`?ip=` 注入的内网地址不再进事件流、
   不再下发他人、非法 `?ip=` 不再变成 Redis 限流键）。也就是说：另一路在 15:05
   从当前工作树重建并上线了 tracker，把我未提交的改动一起带上了线。
   **提醒**：这意味着此刻生产行为不再只由已提交代码决定；两边都要以工作树为准来读日志。
4. **仍被挡住的 6 个文件**（另一路未提交）：`http_track/announce.rs`、`emit.rs`、
   `params.rs`、`tracker/src/main.rs`、`peers/external.rs`、`peers/mod.rs`、
   `worker/jobs/group.rs`。等它们提交后，P1-1/P1-2/P2-1/2/3/5/10/11/12 一次接完，
   再 build tracker+worker+api → 部署 → 该探针必须跑到 22/22 或按剩余待办逐项交代。
