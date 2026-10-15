# FluxTorrent crossseed-api 辅种接口设计稿（不改代码）

> 目标：给 cross-seed 类工具一个**鉴权的批量指纹查询 + 下载**接口，直接对标 PT Studio 规划的 crossseed-api（FR-XSEED-001~009）。结论先行：**FluxTorrent 数据侧已完全就绪，这是现有数据上的一层薄封装**，是全套借鉴里成本最低、感知最强的一个。  
> 状态：设计稿。零代码改动。

---

## 1. 为什么说「已就绪」（代码证据）

| 需要的东西                | FluxTorrent 现状                                                                                               | 证据                                        |
| -------------------- | ------------------------------------------------------------------------------------------------------------ | ----------------------------------------- |
| v1 指纹 `info_hash`    | ✅ `torrents.info_hash CHAR(40) NOT NULL UNIQUE`（已索引）                                                         | `migrations/0001_init.sql:88`             |
| 辅种二级指纹 `pieces_hash` | ✅ `torrents.pieces_hash CHAR(40)`，注释原话「**跨站辅种二级指纹**：SHA1(info.pieces)，YemaPT piecesHash 口径」——**建的时候就是为辅种建的** | `migrations/0069_eco_p0.sql:2-3,23`       |
| 长期凭证                 | ✅ `api_tokens`（`scopes[]`/`rate_per_min`/180 天 + 滚动续期），注释原话「NAS 上长期运行的工具（Prowlarr / **cross-seed** / 自写脚本）」  | `openapi_http/tokens.rs:159-160,205-208`  |
| scope 强制             | ✅ `require_scope` + 白名单校验（`read`/`upload`）                                                                   | `tokens.rs:119-128`                       |
| 限流原语                 | ✅ `rate_window(key, n, window)`                                                                              | `tokens.rs:103`                           |
| 双指纹透出 + download 模板  | ✅ NP 兼容层列表/详情已输出 `info_hash`+`pieces_hash`+download 模板                                                       | `compat_http/nexusphp.rs:248-249,352-353` |
| v2 指纹（btih v2，hex64） | ❌ 无列（FluxTorrent 是 v1 站）                                                                                     | 见 §5 风险                                   |

⇒ 缺的只有：**一个新 scope + 两个路由 + 一条批量查询**。

---

## 2. 设计

### 2.1 新 scope：`crossseed`

- `openapi_http.rs` 加 `pub const SCOPE_CROSSSEED: &str = "crossseed";`（与 `SCOPE_READ/SCOPE_UPLOAD` 并列）。
- `tokens.rs:122-128` 的 scope 白名单校验加入 `crossseed`；`read` 仍是隐含基础（查询要能看种子）。
- 用户签发：`POST /api/v1/me/tokens`，`scopes:["crossseed"]`（仍要走密码/2FA 复核，复用 `step_up`，零改动）。

### 2.2 端点（新模块 `apps/api/src/crossseed_http.rs`，守 300 行门禁）

#### `POST /api/v1/crossseed/lookup`（Token 鉴权，scope `crossseed`）

请求：

```json
{ "info_hash_v1": ["<hex40>", ...], "pieces_sha1": ["<hex40>", ...] }
```

- 两数组合计 ≤ 500 条（超限 400）。
- 响应：

```json
{ "results": [
  { "match": "info_hash_v1", "query": "<hex40>",
    "torrent_id": 123, "name": "...", "size": 456,
    "download_url": "/api/v1/crossseed/dl/123" }
] }
```

- 未命中的 query 不出现在 results（客户端按 query 集合差集判断）。
- **可见性**：只返回「已审核 + 当前用户有权下载」的种子——WHERE 判据**照抄 `nexusphp.rs:308` 详情查询的可见性条件**，不另造口径（避免辅种接口比详情口松）。
- 实现：单条 SQL，`WHERE info_hash = ANY($1::char(40)[]) OR pieces_hash = ANY($2::char(40)[])` + 可见性条件；Rust 侧把每条结果按「哪个入参数命中」回标 `match`/`query`。`info_hash` 已 UNIQUE 索引；`pieces_hash` 索引见 §2.4。

#### `GET /api/v1/crossseed/dl/{id}`（Token 鉴权，scope `crossseed`）

- 返回该种子 `.torrent` 文件（`application/x-bittorrent`），下载计数 +1。
- 落地方式二选一（以现有实现为准，设计稿不锁死）：
  - A. 302 到现有 torrent 下载端点并透传凭据；
  - B. 新开 token 直下（读现有 `/downloads/keys` 或对象存储预签名逻辑）。
- `download_url` 用相对路径，客户端带同一枚 `fxo_` token 请求即可——cross-seed 工具本来就是「一个 token 走全程」。

#### IYUU 兼容路由（`POST /api/v1/crossseed/iyuu`）

- ⚠️ **不在本设计稿编造字段**：IYUU 的请求/响应格式需对照其当前公开对接文档 + IYUUPlus 客户端实际请求核实（PT Studio 也把这条列为「部署前核实」，roadmap §2 R1 F-11）。落法是：先抓包/查文档定型，再包一层格式转换转调 §2.2 的 lookup 内核。
- 设计上预留：IYUU 路由只做「协议适配 → 调同一 lookup 内核」，不碰业务。

### 2.3 限流与配额

- 频率：`rate_window(&state, &format!("xseed:{}", token_row.id), 10, 60)` —— 10 次/分钟（PT Studio 同值）。
- 日配额：Redis `INCR xseed:day:{token_id}:{yyyymmdd}` + `EXPIRE 48h`，> 50000 个指纹/日返回 429。
- 最低等级（可选 v2）：site setting `crossseed_min_class`，默认 0（不限制）；PT Studio 用权限点默认角色，我们后期可对齐。

### 2.4 迁移（大概率零迁移或极简）

- 先查 `migrations/0270_scale_indexes.sql` 是否已给 `pieces_hash` 建索引：
  - 已有 → **零迁移**，纯 API 层。
  - 没有 → 建 **0367**（建号前重查最大号，本文写作时 0366）：`CREATE INDEX IF NOT EXISTS idx_torrents_pieces_hash ON torrents(pieces_hash) WHERE pieces_hash IS NOT NULL;`（幂等）。
- 若加 `crossseed_min_class` 设置键，同批进 settings seed。

### 2.5 审计与隐私

- token 鉴权路径本就更新 `api_tokens.last_used_at`（复用，不新增）。
- 配额 Redis 计数器兼任「每 token 查询量」审计；**不记录具体指纹**（指纹是公开信息，量才有意义；也避免日志膨胀）。
- 不走 `repo.audit`（那是人工操作审计）；辅种查询是机器流量，用量指标走 Prometheus/Redis 即可。

---

## 3. 落地清单（改哪些文件）

| 文件                                  | 改动                                                       |
| ----------------------------------- | -------------------------------------------------------- |
| `openapi_http.rs`                   | 加 `SCOPE_CROSSSEED` 常量                                   |
| `openapi_http/tokens.rs`            | scope 白名单（`:122-128`）加 `crossseed`                       |
| `apps/api/src/crossseed_http.rs`（新） | lookup / dl 两个 handler + IYUU 占位                         |
| `http/mod.rs` 或 `main.rs`           | 挂载 `/api/v1/crossseed` scope（Token 鉴权中间件，与 `/open/*` 同栈） |
| `openapi_http/spec*.rs`             | utoipa path + schema 同步（否则 `check_type_drift` 红）         |
| `migrations/0367_*.sql`（条件）         | pieces_hash 索引（若缺）                                       |



---

## 4. 测试

- [ ] `info_hash_v1` 命中 / 未命中
- [ ] `pieces_sha1` 命中（核心：跨站重打包后 info_hash 变、pieces_hash 不变的场景）
- [ ] 混合批量：一次请求两类指纹，部分命中
- [ ] 可见性过滤：未审核 / 被禁下载用户的种子不出现在 results
- [ ] \> 500 条 → 400
- [ ] scope 强制：无 `crossseed` scope 的 token → 403（复用 `require_scope` 用例模式）
- [ ] 限流：第 11 次/分钟 → 429
- [ ] 日配额：> 50000 指纹 → 429
- [ ] `dl/{id}`：带 token 可下、不带 401、计数 +1
- [ ] 重复 hash 在同一请求里去重

---

## 5. 风险与注意

1. **无 v2**：本站是 v1（`info_hash CHAR(40)`）。若未来支持 BEP v2 混合种子，需加 `info_hash_v2`/`pieces_hash_v2` 列 + 迁移；v1 + pieces_hash 已覆盖主流 cross-seed 客户端（主要用 v1 btih + piece 层匹配）。
2. **指纹即存在性披露**：lookup 会告诉客户端「这种子本站有没有」——这与种子列表/详情同样是公开信息，可见性判据必须与详情一致（§2.2），别比详情松。
3. **IYUU 格式未定型前不要硬编码**：先核实再写转换层；此前 `/crossseed/iyuu` 返回 501 好过返回错格式。
4. **下载鉴权链路**：`download_url` 走 token 直下还是 302 现有端点，动手前看一眼现有 torrent 文件服务怎么发的（`/downloads/keys` / rss feed_key），选复用多的那条。
5. **契约门禁**：加端点必须同步 utoipa，跑 `node scripts/check_type_drift.mjs`；新模块守 300 行，超了按仓内模式拆 `_parts`。

---

## 6. 与其它设计稿的关系

- 与《插件 SQL 隔离》无耦合，可并行；crossseed 是**纯读 + token 鉴权**，不碰插件系统。
- 未来并入统一扩展契约时，crossseed-api 可作为一个「只读数据服务」插件的样板（`net`/`db:read` 能力的最小实践）。
