# 安全姿态

> 一页纸说清 FluxTorrent 的安全设计：站长在做安全评估/等保自查/收录申报时，从这里引用。漏洞披露见仓库根 [SECURITY.md](../../SECURITY.md)。

## 认证与会话

| 项 | 实现 |
|---|---|
| 密码哈希 | Argon2id（`domain/mod.rs`；NP 导入用户走 bcrypt 兼容验证另议） |
| 会话 | JWT（HS256），签发密钥 `JWT_SECRET` ≥32 字节，生产模式启动即拒绝弱值 |
| 2FA | TOTP（RFC 6238，开/关/管理员清除）+ WebAuthn passkey |
| 登录防护 | Redis 滑窗限流（60s/5 次）+ 账户级失败锁定 |
| passkey（tracker） | users.passkey CHAR(32)。两种轮换语义分开：**显式「重置密钥」**（自助 usercp 与后台代为重置）是安全处置动作，旧钥不再进宽限窗、当场作废（tracker 侧随 `flux:guard:ver` 轮询清快照，≤3s）；**改密顺带轮换**才给宽限窗（`PASSKEY_GRACE_HOURS`，默认 7 天，迁移 0302/0308）——passkey 烤在用户已下载的每一个 .torrent 里，改密不该让手上种子集体停种 |
| 注册防护 | 验证码四驱动（none/turnstile/recaptcha/hcaptcha）+ 一次性邮箱域名黑名单 + 邀请码邮箱绑定校验 |

## 传输与响应头

api（actix `DefaultHeaders`）与 web（`next.config.ts` headers）双侧注入：

| 头 | api 值 | web 值 |
|---|---|---|
| X-Content-Type-Options | nosniff | nosniff |
| X-Frame-Options | DENY | DENY |
| Referrer-Policy | strict-origin-when-cross-origin | strict-origin-when-cross-origin |
| Permissions-Policy | — | camera=(), microphone=(), geolocation=() |
| Strict-Transport-Security | 反代注入 | max-age=31536000; includeSubDomains |
| Content-Security-Policy | `default-src 'none'; frame-ancestors 'none'` | 见下 |

web 侧 CSP 基线（E1）：

```
default-src 'self'; script-src 'self' 'unsafe-inline';
style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: http: https:;
font-src 'self' data:; connect-src 'self'; media-src 'self' blob:;
frame-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self';
frame-ancestors 'none'
```

口径说明：

- api 只出 JSON 与文件流，无脚本执行场景，故 CSP 取最严形态 `default-src 'none'`——即便某响应被错误嗅探成 HTML 也无法引用任何资源。
- web 侧 `'unsafe-inline'`（script）来自 Next 内联引导脚本（主题 no-flash / SW 注册 / 主题令牌注入），全部在本仓 `layout.tsx` 内、无用户输入插值；升级 nonce 基建前以此基线斩断**外域**脚本/对象/框架注入面。
- `img-src` 放开 http/https 是因为附件/封面/头像支持站长配置的绝对 URL（含外域图床）。
- 外域视频 embed（论坛/公告）默认被 `frame-src 'self'` 收口；如站长启用了外域 embed 白名单，需在反向代理层为对应路径放宽 frame-src（文档见 [论坛视频内嵌] 章节）。
- 访问日志不落凭据：actix Logger 用 `%m %U`（不含 query），防止 passkey/token/apikey 进日志。

## 威胁模型：开源引擎下的反作弊（0309）

本引擎 MIT 开源，**检测算法与出厂默认值人人可读**——这是战略选择（对
GPL/AGPL 竞品的差异化 + 站长零法律摩擦二开），不是疏漏。应对模型是
「**算法公开、密钥私有**」：代码里公开的是检测的形状，每个站实际的
钥匙（参数/时机/比例）私有且可变。

分层与对策：

| 层 | 对策 | 落点 |
|---|---|---|
| 出厂默认值 = 公开知识 | 反作弊组全部键 hint 带「开源提示」；上线必改非默认值 | 后台「反作弊」卡片（迁移 0309） |
| 固定节拍可被「卡表」 | 探测循环每轮加 0~`probe_jitter_secs` 随机延迟（缺省 90s，可后台改） | tracker 探测任务（0309） |
| piece 抽查位置可预测 | 按 `probe_piece_ratio`（缺省 0.25）随机抽人，与采样序无关 | 同上 |
| 规则可读码绕过 | 重心压在**物理验证**：BT 握手（`bt_probe.rs`）→ bitfield → piece SHA-1 比对——对抗的是现实不是规则，读源码也绕不过 | tracker `peers/` |
| 服务端数据不对称 | traffic_ledger 全量流水、xreport 交叉佐证、贴边节奏画像（0304）——站方知道作弊者不知道的 | worker `jobs/` |
| 引擎层不够用 | **私有检测层**：适配器沙箱（wasmtime）+ 规则包体系允许站长部署私有反作弊规则，MIT 下完全合法且官方支持，见 [适配器](../customize/adapters.md) | `apps/api/src/adapter_runtime.rs` |

给站长的三条纪律：

1. **上线后把反作弊组的值全改成非出厂值**——出厂默认随源码公开，留着
   等于把调参插在门上。重点：`traffic_credit_max_bps`（速率钳制）、
   `ratio_watch_threshold`、`probe_jitter_secs`、`probe_piece_ratio`。
2. 私有调参不要外传——它就是你的「密钥」。代建/交接场景传 fork 合法
   （MIT），但传出去就不再是私有参数。
3. 升级引擎不会覆盖你的调参（迁移只补缺行 `ON CONFLICT DO NOTHING`），
   但新增检测键的出厂值仍要按第 1 条处理。

红队自查：`scripts/redteam_probe.py` 用作弊者视角对本站做黑盒探测
（幽灵做种/伪造握手/伪 piece/速率超窗），大版本发布前跑一轮。

## 注入与输入

- SQL：全程 sqlx 参数绑定（历史 SQL 注入点已修，见 CHANGELOG）；LIKE 通配符转义防全表扫描 DoS。
- 富文本：论坛/公告走 ammonia 白名单净化；附件 mime 白名单（下载侧校验）。
- HTML：React 默认转义；仅两处 `dangerouslySetInnerHTML`（主题注入/自定义页面 body），前者只接受 `#rrggbb` 白名单值，后者为站长后台富文本（信任级等同模板）。

## 限流与防滥用

- 登录/敏感写：Redis 滑窗（`rl:` 键）；开放 API 独立 token 限流 60 req/min。
- announce：tracker 侧防护缓存（Redis 3s 轮询版本号 bump），不打 PG。
- announce 事件合并（P2-1）：同一 `(账号, 种子)` 的周期 announce 在 `interval × ANN_EVENT_MERGE_PCT%`（默认 40%，60s–900s 夹取）内只回 peer 列表、不投事件——一条事件是一个 PG 事务 + 行锁，而差值计账/做种时长按间隔累计，合并无损。`started`/`completed`/`stopped` 永不合并；Redis 挂了照常投（fail-open）。
- 待审种子准入：`site_settings.announce_pending_policy`（迁移 0303，后台「反作弊」卡片可选）——
  `self_seed_only`（默认）照常接受发布者的 announce 与计费，但对非发布者/非员工清空 peer 列表与计数；
  `allow_all` 为旧行为，`owner_only` 直接拒绝非发布者。站点详情页本就隐藏待审种（visibility.rs），
  此项补齐 tracker 数据面的同一口径。
- 回连可达性档位：`site_settings.connectable_gate`（迁移 0308）只有两档——`off`（默认）
  tracker 的 TCP 回连 + BT 握手实测结果只写进 `snatches.connectable` 供版主筛，**不否决在种**；
  `hard` 才把实测不可达判成不在种（本站旧行为）。缺省选 off 是按主流口径来的：UNIT3D 的
  `connectable_check` 默认 false 且唯一消费者是 BON 条件、NexusPHP 建行硬编码 `'yes'`、
  Ocelot 从不写该列——拿它一票否决会把 NAT 后没有映射入站端口的真做种者静默判成不在种
  （在种数、保种考核、濒危种救援一起塌）。机房/公网可达的站想要硬口径再显式开 `hard`。
- 即时分享率闸门：`site_settings.ratio_gate`（`off`/`warn` 默认/`block`，迁移 0308）。
  拦的是**下载**（announce 的 `left > 0`），做种永不拦——拦做种等于把要补比率的人赶出
  swarm，比率只会更差。门槛 = `min(max(user_classes.min_ratio, ratiolimit), ratio_gate_max)`，
  豁免四条例外：员工（class ≥ 90）、`downloaded = 0`（比率无从计算）、注册未过
  `ratio_gate_grace_days`（与等级 `min_age_days` 取较大者）、已在 `ratio_watch` 观察期内
  （那条异步链已处置，不重复罚）。HTTP 与 UDP 两条通道共用同一个 `decide()`。
  **开站前必查**：`ratiolimit` 是历史值（本仓库出厂样例是 6，等级 `min_ratio` 全 0），
  照它直接开 `block` 等于把全站下载锁死；先设 `warn` 看
  `flux_tracker_ratio_gate_warn_total` 的量，确认门槛数字合理再拧 `block`。
- IP：ip_bans 封禁（0302 起支持 CIDR 段，`/0` 拒收）+ testip 工具。取信分两档，别再混称：
  `TRUST_PROXY=1` 信 X-Forwarded-For（右数第 `TRUST_PROXY_DEPTH` 段）；
  `TRUST_PROXY_IP=1` 信 announce 的 `?ip=` —— 那是**客户端自报**，比 XFF 更宽，只供调试。
  两档取值都必须解析成合法地址且不属于保留/内网段（`ALLOW_PRIVATE_PEER_IP=1` 才放行 RFC1918/CGNAT/ULA），否则回落 socket 对端。
- **XFF 取信语义（2026-10-07 三轮审计后）**：`TRUST_PROXY=1` 时 api/tracker 取 XFF **右值**（链尾）——即「直连我的那台反代追加的值」。两条部署红线：
  1. 反代必须用 `$proxy_add_x_forwarded_for`（追加语义）。若配成 `proxy_set_header X-Forwarded-For $http_x_forwarded_for`（透传客户端自带值），右值=攻击者伪造值，限流/ip_bans 整体失效。
  2. api 容器端口不得直接对外（compose 默认绑 127.0.0.1）——直连暴露时客户端可自带「干净尾值」伪造来源 IP。
  web 容器的 `/api` 同源代理只**原样透传** XFF（不自造首位，2026-10-07 修复），站长外层 nginx 的追加语义直接贯通到 api。闸门自检：`FLUX_API_BASE=... python scripts/pt_audit_f_entitlement.py sec_g_xff_chain`（G4：伪造 XFF 不得污染 login_events）。

## 审计与监控

- audit_log 后台可查（管理动作全量）；runtime_logs（WARN+）落库后台「运行日志」页。
- **审计防篡改链（0266 起）**：每行 `self_hash = SHA256(prev_hash ‖ 行内容)` 串成哈希链，触发器同时拒绝 UPDATE/DELETE；`GET /admin/audit/chain-verify` 一键全链校验（返回断链位置，null = 完整）。0266 之前的存量行已按同一公式回填串链。
- Prometheus 告警 5 条：DLQ 积压 / 5xx 率 / tracker Redis 降级等（`docker/monitoring/`）。
- 周度 `cargo audit`（CI security-audit workflow）。

## 自查清单（开站前）

1. `JWT_SECRET`/`DB_PASSWORD`/`REDIS_PASSWORD` 均为强随机值（.env 不入库）；
2. TLS 已在反代终结且 HSTS 生效（`curl -I https://站名` 应见 `Strict-Transport-Security`）;
3. `CORS_ORIGINS` 已显式配置为站点域名（生产必填）；
4. root 已改密 + 已建日常管理账号；
5. 验证码驱动已选（生产不建议 none）；
6. 备份 cron 已配（`scripts/backup.sh`，见 webmaster/launch-checklist）；
7. 反作弊组的键已全部改为非出厂值（见上方「威胁模型」一节——出厂默认随源码公开）。
