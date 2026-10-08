# 安全姿态

> 一页纸说清 FluxTorrent 的安全设计：站长在做安全评估/等保自查/收录申报时，从这里引用。漏洞披露见仓库根 [SECURITY.md](../../SECURITY.md)。

## 认证与会话

| 项 | 实现 |
|---|---|
| 密码哈希 | Argon2id（`domain/mod.rs`；NP 导入用户走 bcrypt 兼容验证另议） |
| 会话 | JWT（HS256），签发密钥 `JWT_SECRET` ≥32 字节，生产模式启动即拒绝弱值 |
| 2FA | TOTP（RFC 6238，开/关/管理员清除）+ WebAuthn passkey |
| 登录防护 | Redis 滑窗限流（60s/5 次）+ 账户级失败锁定 |
| passkey（tracker） | users.passkey CHAR(32)，泄漏可自助重置；重置/改密后旧 key 进入宽限窗（`PASSKEY_GRACE_HOURS`，默认 7 天，迁移 0302）后失效 |
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

## 注入与输入

- SQL：全程 sqlx 参数绑定（历史 SQL 注入点已修，见 CHANGELOG）；LIKE 通配符转义防全表扫描 DoS。
- 富文本：论坛/公告走 ammonia 白名单净化；附件 mime 白名单（下载侧校验）。
- HTML：React 默认转义；仅两处 `dangerouslySetInnerHTML`（主题注入/自定义页面 body），前者只接受 `#rrggbb` 白名单值，后者为站长后台富文本（信任级等同模板）。

## 限流与防滥用

- 登录/敏感写：Redis 滑窗（`rl:` 键）；开放 API 独立 token 限流 60 req/min。
- announce：tracker 侧防护缓存（Redis 3s 轮询版本号 bump），不打 PG。
- 待审种子准入：`site_settings.announce_pending_policy`（迁移 0303，后台「反作弊」卡片可选）——
  `self_seed_only`（默认）照常接受发布者的 announce 与计费，但对非发布者/非员工清空 peer 列表与计数；
  `allow_all` 为旧行为，`owner_only` 直接拒绝非发布者。站点详情页本就隐藏待审种（visibility.rs），
  此项补齐 tracker 数据面的同一口径。
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
6. 备份 cron 已配（`scripts/backup.sh`，见 webmaster/launch-checklist）。
