# FluxTorrent 统一扩展契约设计稿（FX Ext Contract）

> 状态：设计稿（不改动代码）。对应结论「把已有的 wasmtime / 插件钩子 / 表达式引擎收敛成一套统一契约」。
> 参照系：PT Studio 的「统一扩展契约 + 能力最小化 + SQL 隔离」原则（仅学原则，不抄其未经验证的全插件平台）。
> 定位：FluxTorrent 已有「功能先落地、架构后收敛」的纪律。本稿是**收敛层**，不是重写，不动单体（Actix + Next）。

---

## 1. 为什么收口

当前 FluxTorrent 有**三套互相独立**的扩展机制，每套都有自己的清单（manifest）、生命周期、安全模型：

| # | 机制 | 代码位置 | 后端形态 | 当前用途 | 安全边界 |
|---|---|---|---|---|---|
| A | wasmtime 适配器宿主 | `adapter_*.rs`（`adapter_runtime` / `adapter_host` / `adapter_sandbox`） | 外部 `.wasm` 沙箱 | `/ptgen` 外部元数据抓取（豆瓣等） | 强：仅 `fetch` ABI + 出网白名单 + 限流 + 缓存；**无 SQL / 文件 / veto** |
| B | 插件钩子 | `plugins.rs`（`Plugin` trait + `DomainEvent` + `PluginManager`） | 进程内 Rust trait object，编译期静态装配 | `auto_pin_official`（官种发布自动置顶） | 弱：**`catch_unwind` 隔离，但允许裸 SQL / 全库读写** |
| C | 受限表达式引擎 | `rules_engine.rs`（`RuleSpec` + `lint`/`eval`） | evalexpr 白名单公式 | 规则包银行利率（`pack_format.rs` / `packs.rs` / `economy.rs`） | 构造安全：纯计算、变量/函数白名单、越域回落 |

**问题**：三者没有统一的「扩展」抽象。再加一种扩展类型（例如反作弊私有规则、私有注册策略），就得重造 manifest + 生命周期 + 安全模型；且安全水位参差不齐——wasm 最严，native 插件最松，公式天然安全。

---

## 2. 现状的不一致与风险（六点）

1. **无统一清单**。A 是 `AdapterManifest`（id/kind/http_allow/secrets_read/rate_limit_per_min）；B 是代码里的 `Vec<Arc<dyn Plugin>>` + `plugin__{name}` 站点开关；C 是编译期 `RuleSpec` 常量。三者形状互不相通。
2. **安全水位不均（最关键）**。A 严格（网络白名单、无 DB）；C 构造安全；**B 没有任何能力约束**——`AutoPinOfficial::on_torrent_upload` 直接 `sqlx::query("UPDATE torrents SET sticky=TRUE …")`，拿的是完整 `&AppState`。第三方若写一个「native 插件」即等于拿到全库读写，正是 NexusPHP 19 个 CVE 的同款弱点。
3. **事件模型碎片化**。`DomainEvent`（`plugins.rs`）只服务 B；A 是按需调用（`/ptgen` 里手动调 `AdapterRuntime::call`）；C 在 `economy.rs` 结算时手动 `eval`。没有统一的「某事件 → 触发扩展」总线。
4. **无能力声明**。三者都没有「以宿主可校验方式声明自己能做什么」。A 的能力隐含在 host 导入函数里；B 无约束；C 的能力隐含在 `vars` 白名单里。宿主无法在边界处做「最小授权」校验。
5. **无版本化 / 冻结**。A 的 ABI 是手写 PBR 布局（线性内存偏移 64 / `out_region` 等魔术数）；B 的 trait 未版本化；C 无版本。没有 WIT 式的「先用真实扩展验证、再定版冻结」。
6. **第三方贡献路径不完整**。只有 A（wasm）真正支持第三方上传安装；B 必须编进 bin；C 是常量。即「站长不写代码改产品」在 B/C 上不成立。

---

## 3. 统一扩展契约：`fx:extension@1.0.0`

**核心思想**：把 A/B/C 都视作「同一个契约的不同后端 runtime」。新增一种扩展类型 = 在契约里加能力，而不是重造一套机制。

### 3.1 一个 `ExtensionManifest`（冻结面）

每个扩展——无论后端是 wasm / native / formula——都声明同一份清单：

| 字段 | 含义 | 来源（当前） |
|---|---|---|
| `id` | 全局唯一 | A: `adapter_id`；B: `name()`；C: `key` |
| `kind` | `wasm` \| `native` \| `formula` | 新增统一枚举 |
| `version` | 语义化版本 | 新增 |
| `display` | 展示名 / 描述 | 新增（后台可见） |
| `capabilities` | **显式请求的能力 allowlist**（见 3.2） | A 部分隐含；B 无；C 隐含 |
| `triggers` | 订阅哪些 `DomainEvent` | B: `on_*`；A/C 按需 |
| `limits` | `rate_per_min` / `timeout_s` / `circuit_breaker` | A 已有；B/C 无 |
| `signature` | minisign 签名槽（见 3.6） | 新增（可选） |

> 冻结纪律：契约字段在 `@1.0.0` 定版前，必须被每个 runtime（wasm/native/formula）至少一种真实扩展用过；**不在纸面上冻结**。

### 3.2 能力模型（最小授权 + 敏感数据默认封闭）

宿主**只授予清单里声明的能力**；未声明的能力在宿主边界直接拒绝。能力清单：

| 能力 | 含义 | 映射现状 |
|---|---|---|
| `net:egress` | 出网（含 `allow` 主机 glob 白名单） | A 的 `http_allow` |
| `kv` | 命名空间缓存（TTL 上限） | A 的 `cache_*` |
| `secret:read` | 读取指定命名 secret | A 的 `secrets_read` |
| `event:on` | 订阅指定 `DomainEvent` 变体 | B 的 `on_*` |
| `db:query` | **作用域 SQL 只读**（表/列 allowlist） | **新增**，替代 B 的裸 SQL |
| `db:write` | **作用域 SQL 写**（表.列 allowlist，需评审） | **新增**，替代 `AutoPinOfficial` 的 `UPDATE` |
| `formula:eval` | 声明 `vars` + 数值域的公式求值 | C 的 `RuleSpec` |
| `log` | 带前缀日志 | A 的 `host_log` |

**关键收敛点**：`native` / `wasm` 扩展若需要改库（如置顶官种），**必须声明 `db:write(torrents.sticky)` 且仅此一列**，不再持有完整 `&AppState`。这直接消除「插件 = 全库读写」的 CVE 类风险。

### 3.3 三个 runtime 收敛到同一契约接口

| Runtime | 当前代码 | 契约映射 | 改造要点 |
|---|---|---|---|
| `WasmRuntime` | `AdapterRuntime` + `adapter_host` + `adapter_sandbox` | `kind=wasm`，映射 `net/kv/secret/log` 到 host 导入 | **隔离强度保持不变**（已最强），仅把清单改为 `ExtensionManifest` |
| `NativeRuntime` | `PluginManager` + `Plugin` trait | `kind=native`，映射 `event/db/kv/log` | **核心加固**：`on_*` hook 入参从 `&AppState` 改为 `&CapabilityHandle`；handle 只暴露 `db.query_scoped(cap)` / `kv` / `log` / `emit`，移除裸 `state.repo.db` 访问 |
| `FormulaRuntime` | `rules_engine` | `kind=formula`，`RuleSpec` 成为 manifest 条目 | 基本不动，仅纳入统一清单与生命周期 |

> `CapabilityHandle`：宿主在分发时按 `manifest.capabilities` 校验；任何越权调用（如未声明 `db:write` 却尝试写）在边界返回错误并记审计。**这是 PT Studio「能力最小化」原则在 FluxTorrent 单体里的落地形态**——不引入 per-plugin schema/角色/Redis 前缀那套尚未被生产验证的复杂度。

### 3.4 统一事件总线

把 `DomainEvent`（`plugins.rs:18`）升级为**版本化、冻结的枚举**，作为唯一事件契约：
- A（适配器）：`/ptgen` 等按需调用改为「事件触发适配器实例」；
- B（插件）：`dispatch_login` / `dispatch_upload`（`login.rs:243` / `upload.rs:398`）+ worker 里程碑，全部走总线；
- C（公式）：规则包公式在对应事件（如结算）经总线触发 `eval`。
- 三种 runtime 都通过 `event:on` 订阅，载荷按契约 schema 校验。

### 3.5 版本化与冻结（WIT 式）

- 契约三件套——`ExtensionManifest` schema + host ABI（host 导入函数签名）+ `DomainEvent` 枚举——统一版本号 `fx:extension@N.M.P`。
- 冻结门槛：每个字段/每个事件变体至少被一个真实扩展（每个 runtime 各一）演练过，才允许 `@1.0.0` 定版。避免「纸面最强、未兑现」。

### 3.6 安全加固（学 PT Studio 的三处强点，且要补它的缺口）

1. **SQL 三层隔离**（任何持 `db` 能力的扩展）：
   - 专用角色 `INHERIT FALSE, SET TRUE`；
   - 撤销 `set_config` / 禁用裸 plpgsql；
   - 用 `pg_query` 做语句检查，强制命中 manifest 的表.列 allowlist。
   - 新增迁移（概念）`0171_ext_sql_isolation.sql` 建角色与撤销项。
2. **补 SQL 逃逸测试**（PT Studio 的缺口 #3——我们不在冻结前留这个洞）：针对 `db:query` / `db:write` 作用域写绕过用例（越列表、越列、试图 `set_config`、试图 DDL）。
3. **签名槽 `signature` 现在就定义，但仅 `prod` 第三方要求**（PT Studio 缺口 #2——我们不重复：定义槽位，启用未签名第三方仅限 `dev`/`test`，`prod` 强制 minisign）。

---

## 4. 迁移路线（分阶段，零代码改动先行）

| 阶段 | 动作 | 行为变化 | 风险 |
|---|---|---|---|
| **P0 设计冻结** | 本稿 + 写明 `ExtensionManifest` schema + `DomainEvent` 枚举 + 能力清单 | 无 | 无 |
| **P1 包装层（零行为）** | 新增 `ExtensionManifest` 结构；`AdapterManifest` / `PluginManager::builtin` / `RuleSpec` 改为**转换自**它 | 无运行时变化 | 低 |
| **P2 native 能力闸门（安全收益）** | `Plugin` trait 入参改 `&CapabilityHandle`；`AutoPinOfficial` 改写为声明 `db:write(torrents.sticky)` + 作用域查询；内置插件走兼容 shim | native 插件不再持有全库；置顶功能等价 | 中（需回归 `upload.rs:398` 路径） |
| **P3 事件总线统一** | `dispatch_*` 与 `/ptgen`、公式结算改走统一总线；扩展 `DomainEvent` | 分发路径收敛，可观测性提升 | 中 |
| **P4 第三方 native + 签名** | 允许通过签名 manifest 装载 native 扩展，不重编 bin；`prod` 强制签名 | 站长可装私有逻辑（反作弊/注册策略） | 中（需 P2 闸门先落地） |
| **P5 契约冻结 @1.0.0** | 所有 runtime 演练过每个字段后定版 | 稳定扩展 API | 低 |

> 每阶段独立可回退；P2 必须在 P4 之前（否则第三方 native = 全库读写）。

---

## 5. 我们刻意**不**做什么（对照 PT Studio 的三个硬伤）

- **不**把单体拆成「每插件独立 schema / 角色 / Redis 前缀 / minisign 全开」的全插件平台——那套正确性尚未被生产实测，运维成本极高。
- **不**在不需要时强制 minisign——仅 `prod` 第三方要求，避免阻塞内部/测试场景（PT Studio 的 `prod` 不可用缺口）。
- **不**把 PT 刚需（反作弊*判定*、注册策略、分享率考核、字幕、论坛、娱乐屋）改写成插件——它们留在核心，扩展只**包裹/增强**核心，不替代核心。保持「功能先落地」。
- wasm 沙箱的**强隔离保持不变**，不因为统一契约而降级它的边界。

---

## 6. 验收 / 冻结门槛（@1.0.0）

- [ ] 三种 runtime（wasm / native / formula）各至少一个真实扩展跑通统一 `ExtensionManifest`；
- [ ] `native` 扩展无法越权访问未声明能力（有失败用例）；
- [ ] SQL 三层隔离迁移落地 + 逃逸测试全绿；
- [ ] `DomainEvent` 覆盖当前所有真实分发点（`login` / `upload` / 里程碑 / 公式结算）；
- [ ] `prod` 模式下未签名第三方扩展被拒；`dev`/`test` 可装载。

---

### 附：现状代码锚点速查

- wasm 宿主 ABI：`adapter_host.rs:15` `build_linker`（host/http_fetch/cache_*/host_log/secret）
- wasm 清单：`adapter_runtime.rs:41` `AdapterManifest`
- wasm 安全：`adapter_runtime.rs:34-36`（epoch 10s / 挂钟 25s）、`:127` `url_allowed` 白名单
- 插件 trait：`plugins.rs:36` `Plugin`、`:18` `DomainEvent`、`:98` `PluginManager`、`:118/146/179` 三个 dispatch
- 裸 SQL 风险点：`plugins.rs:235` `AutoPinOfficial::on_torrent_upload` 的 `UPDATE torrents`
- 公式引擎：`rules_engine.rs:16` `RuleSpec`、`:46` `lint`、`:127` `eval`
- 公式驱动点：`pack_format.rs:48`、`:57`、`packs.rs:175/185`、`economy.rs:39/66`
- 插件分发点：`login.rs:243` `dispatch_login`、`upload.rs:398` `dispatch_upload`
