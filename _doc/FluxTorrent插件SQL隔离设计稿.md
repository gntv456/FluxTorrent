# FluxTorrent 插件 SQL 隔离设计稿（安全地基，不改代码）

> 目标：消除 `apps/api/src/plugins.rs:235` 的 CVE 类风险——native 插件持完整 `&AppState`，可对任意表跑裸 SQL（`AutoPinOfficial` 直接 `UPDATE torrents`）。借鉴 PT Studio 的「能力最小化 + 插件 SQL 隔离」，但**按 FluxTorrent 单体共享 schema 的现实裁剪**，不照搬它的 per-plugin schema。  
> 状态：设计稿。零代码改动。所有建号/依赖动作标注了落点与风险。

---

## 1. 为什么不能照搬 PT Studio 的 per-plugin schema

PT Studio 给每个插件建独立 schema + 独立 DB role + `SET LOCAL search_path`（`storage/migrate.rs`）。它的前提是「微内核 + 一切皆不可信插件 + 每插件数据物理隔离」。

FluxTorrent 现实：

- 单体、**共享 schema**（359 个迁移全在 public），拆 per-plugin schema = 数据模型大改，不可接受。
- native 插件**编译期装配、全内置**（`PluginManager::builtin()` 只有 `auto_pin_official`），目前没有第三方 native 路径——**没有不可信第三方**。
- 单 `PgPool` 单登录角色（`Repo{db, read_db}`）。

⇒ **结论**：当前风险不是「不可信第三方逃逸」，而是「任何插件（哪怕写砸的内置插件）能越权/误操作任意表」。所以第一阶段用**能力声明 + 语句校验**关门即可，DB role 留到开放第三方时再做纵深。

---

## 2. 设计：`PluginCtx` 能力句柄 + 声明式 DB 能力 + 语句校验

### 2.1 三层结构

| 层  | 组件                                    | 作用                                   |
| -- | ------------------------------------- | ------------------------------------ |
| 契约 | `DbCapability`（插件声明能碰哪些表/列/读写）        | 最小授权清单                               |
| 句柄 | `PluginCtx`（替代 `&AppState` 传给 hook）   | 插件**只能**经 `ctx.db_query_scoped()` 碰库 |
| 校验 | `sql_guard` 语句校验器（Rust 侧，fail-closed） | 每次查询前校验：语句类型/表/列/WHERE               |

### 2.2 能力声明（`DbCapability`）

```rust
// plugins.rs 或新模块 plugin_ctx.rs
pub struct DbCapability {
    pub table: String,          // 允许的表，如 "torrents"
    pub columns: Vec<String>,   // 允许的列，如 ["sticky","id","official_tag"]
    pub mode: DbMode,           // Read | Write
    pub allow_unbounded: bool,  // 是否允许无 WHERE 的整表写（默认 false）
}
pub enum DbMode { Read, Write }
```

每个插件在清单里声明自己的能力。`auto_pin_official` 现声明白名单：

```rust
// 只能写 torrents.sticky，读 WHERE 用到的 id/official_tag
DbCapability { table: "torrents".into(),
               columns: ["sticky","id","official_tag"].into(),
               mode: DbMode::Write, allow_unbounded: false }
```

`PluginManager` 改持 `(Arc<dyn Plugin>, Vec<DbCapability>)` 对（`builtin()` 里声明）。

### 2.3 `PluginCtx`（hook 新入参）

```rust
pub struct PluginCtx<'a> {
    pool: &'a PgPool,
    caps: &'a [DbCapability],
    plugin: &'a str,
    redis: &'a redis::aio::ConnectionManager, // 可选，按需暴露
}
impl PluginCtx<'_> {
    /// 插件碰库的唯一入口：先过 sql_guard，再 sqlx 参数化执行
    pub async fn db_query_scoped(&self, sql: &str, binds: &[Bind]) -> DomainResult<PgQueryResult>;
    pub fn log(&self, ...);        // 带 plugin 前缀（复用现有 host_log 思路）
    pub fn audit(&self, ...);      // 走 repo.audit_detail，actor=None、action 带 plugin 前缀
    // kv/redis 按需再加，默认不给（能力最小化）
}
```

- `Plugin` trait 三个 hook 的 `&AppState` 全改为 `&PluginCtx`（`plugins.rs:36-48`）。
- 分发点（`auth_http/login.rs:243`、`publish_http/upload.rs:398`、worker 里程碑）从 `&AppState` 构造 `PluginCtx`（`PluginCtx::new(&state.repo.db, &caps, name, &state.redis)`）。**分发点调用方式不变**，只是内部多包一层。
- 失去的能力：插件不再能直接读 `state.repo`/`state.redis`/`state.cfg`。需要的能力显式加进 `PluginCtx`（当前内置插件只用得到 db + log + audit）。

### 2.4 `sql_guard` 语句校验器（核心，fail-closed）

新增依赖 **`sqlparser`**（纯 Rust，Postgres 方言，无原生编译；备选 `pg_query.rs` 是 libpg_query 的 C 绑定，跨平台/沙箱成本高，**不推荐**）。校验只作用于**插件查询**（低频），不碰主应用，解析器覆盖面只需满足插件写的 SQL，且**解析失败即拒**。

校验规则（按序，任一不过 → `DomainError::Validation` + 审计）：

1. **解析**：`sqlparser` Postgres 方言解析；失败 → 拒（fail-closed，插件必须写可解析 SQL）。
2. **单语句**：拒绝 `;` 分隔的多语句（`SELECT 1; DROP TABLE users`）。
3. **语句类型**：仅允许 `SELECT / INSERT / UPDATE / DELETE`；拒绝 `CREATE/DROP/ALTER/TRUNCATE/GRANT/REVOKE/SET/RESET/COPY/DO/CALL/LOCK/EXPLAIN`。
4. **表白名单**：收集所有表引用（含子查询、CTE、JOIN），每个表必须 ∈ 该插件声明的 `caps.table`。
5. **列白名单**：
   - `SELECT *` → 只有当该表声明的 `columns` 覆盖「插件查询实际需要的全部列」时才放行；保守做法：**默认拒绝 `*`**，插件显式列列名（内置插件好改）。
   - 投影列 + WHERE/JOIN/ORDER/GROUP 里的列，必须 ∈ 所引用的表的声明 `columns`。
   - `RETURNING` 列同样校验。
6. **写保护**：`UPDATE/DELETE` 必须有 `WHERE`（拒绝整表操作），除非该 capability `allow_unbounded=true`。
7. **危险函数/构造**：拒绝 `set_config`、`current_setting`、`pg_sleep`、`pg_read_file`、`pg_read_server_file`、`dblink`、`lo_import/lo_export`、`COPY`（已被 3 覆盖，双保险）。
8. **占位符**：`$1..$n` 允许（sqlx 绑定值）；字面量是数据不是标识符，结构校验（表/列/类型）才是闸门。

> 关键点：**闸门管「结构」（能碰哪些表/列、能做什么语句），不管「值」**。绑定参数始终走 sqlx 参数化（`repo/mod.rs` 已是全线 `.bind()`），注入面早已封死；sql_guard 封的是「越权访问未授权表/列」和「危险语句类型」。

### 2.5 执行路径

`db_query_scoped(sql, binds)`：

```
sql_guard::validate(sql, caps)?      // 结构校验，不过即拒 + 审计
sqlx::query(sql).bind(...).execute(pool).await   // 现有参数化执行
```

插件 hook 在 `spawn_blocking` + `Handle::block_on` 里跑（沿用 `plugins.rs:130` 现有模式），`db_query_scoped` 的异步体照常。

---

## 3. 迁移路线（分阶段，每阶段可回退）

| 阶段                        | 动作                                                                                                                                                                                           | 行为变化                                 | 风险                                                          |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ | ----------------------------------------------------------- |
| **P0 本文**                 | 设计定稿                                                                                                                                                                                         | 无                                    | 无                                                           |
| **P1 闸门（核心收益）**           | ①加 `sqlparser` 依赖 ②新模块 `plugin_ctx.rs`（`DbCapability`/`PluginCtx`/`sql_guard`）③`Plugin` trait 改 `&PluginCtx` ④`builtin()` 声明能力 ⑤重写 `AutoPinOfficial` 用 `db_query_scoped` ⑥分发点包一层 `PluginCtx` | native 插件不再持 `&AppState`；置顶功能等价；越权即拒 | 中（改 trait 签名，需回归 login/upload/里程碑三处分发）                      |
| **P2 可见性**                | 迁移 **0367**（建号前重查最大号）：`plugin_capabilities(plugin, table_name, columns, mode, granted_at)`，启动期从清单回填，后台只读展示「每个插件能干什么」                                                                         | 无运行行为，纯可观测 + 为 P3 铺路                 | 低                                                           |
| **P3 纵深（开放第三方 native 时）** | 每插件 DB role + `SET LOCAL ROLE` +  registry 驱动 GRANT；补逃逸测试（`RESET ROLE`/`set_config`/plpgsql 动态 SQL）                                                                                          | 不可信插件也关进 role 沙箱                     | 高（PT Studio 自己也把「连接池下 SET ROLE 正确性」列为待验证，须实测 pgbouncer/池交互） |
| **P4 收口**                 | 并入统一扩展契约（见《FluxTorrent统一扩展契约设计稿》）                                                                                                                                                            | —                                    | —                                                           |



> **P1 是本次要做的**：零 DB 变更、零行为变化（就一个内置插件）、直接把 `UPDATE torrents` 的越权面砍成「只能碰 torrents.sticky」。P3 等真开第三方再做。

---

## 4. 测试（PT Studio 欠的逃逸测试，我们补）

`plugin_ctx` 单测（不需要真库，校验器是纯函数）：

- [ ] 越表：声明仅 `torrents`，`SELECT ... FROM users` → 拒
- [ ] 越列：声明仅 `torrents.sticky`，`SELECT pass_hash FROM users`/`SELECT email FROM torrents` → 拒
- [ ] DDL：`CREATE TABLE` / `DROP TABLE torrents` / `ALTER TABLE` → 拒
- [ ] 提权：`SET ROLE` / `RESET ROLE` / `SELECT set_config('role',...)` / `GRANT` → 拒
- [ ] 多语句：`SELECT 1; DROP TABLE users` → 拒
- [ ] 无 WHERE 写：`UPDATE torrents SET sticky=TRUE`（无 WHERE）→ 拒
- [ ] `SELECT *` 未全覆盖声明列 → 拒
- [ ] CTE/子query 引用未声明表 → 拒
- [ ] **合法用例仍放行**：`UPDATE torrents SET sticky=TRUE WHERE id=$1 AND official_tag`（auto_pin_official 原句）→ 过
- [ ] 解析失败 → 拒（fail-closed）
- [ ] 分发回归：`dispatch_login/upload/milestone` 三处仍正常触发、panic 仍被 `catch_unwind` 吞（沿用 `plugins.rs` 现有语义）

---

## 5. 风险与注意

1. **sqlparser 覆盖度**：对插件写的、合法但解析器不认的 SQL 会误拒。可接受（fail-closed，插件改写法）；但要把误拒报错写清楚（返回解析器原文，便于插件作者修）。
2. **trait 签名变更波及面**：`&AppState` → `&PluginCtx` 影响 3 个分发点 + 示例插件 + 测试。全是内部 API，无外部契约。改完 `cargo check -p flux-api` 兜底。
3. **不要让 `PluginCtx` 重新变成「万能句柄」**：只加内置插件真需要的方法（现在 = db + log + audit）。kv/redis/cfg 需要时再逐个加，每个加的东西就是新的攻击面。
4. **不要退化成「只校验不执行隔离」就以为万事大吉**：P1 是「关门」（越权即拒），不是「沙箱」（不可信代码）。真正跑不可信第三方必须 P3 的 DB role。文档里写清这个边界，别让后来者误判。
5. **迁移纪律**：真要建 0367 前，按仓内纪律 `ls apps/api/migrations/*.sql | 查最大号 +1`（本文写作时 0366），建号后立刻 build api；新 SQL 幂等。

---

## 6. 与两份其它设计稿的关系

- 本稿是**安全前置**：反作弊运营层 / 注册治理 / 商城这些「持库能力的高权限插件」要上，先过这一关。
- P4 并入《FluxTorrent统一扩展契约设计稿》——`DbCapability` 就是统一契约里 `db:query`/`db:write` 能力的落地形态。
- 与 crossseed-api 无耦合，可并行。
