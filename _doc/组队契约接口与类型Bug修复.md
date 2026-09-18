# 组队契约接口实现与类型问题修复

> 承接 `_doc/实现记录-P0代码与二次修正.md`
> 本文记录 P0 之后的下一步：**实现组队接口**（而非埋点），以及过程中挖出的两个类型 bug。

---

## 一、为什么这轮没做埋点

上轮我把"埋点"列为下一步，但按铁律先做了调研，结论是现在做没有意义：

**1. 项目没有通用埋点设施。** 现有表全是专用用途（`audit_log` 管理审计 / `cheat_events` / `leak_events` /
`login_events` / `torrent_operation_logs`），代码里也没有任何 analytics 调用。为一次验证建一套通用埋点，
属于过度投资。

**2. 开发库没有真实用户可统计：**

| 指标 | 值 |
|---|---|
| 种子总数 / 已过审 | 30 / 29 |
| 死亡资源（seeders = 0） | **29** |
| 濒危资源（seeders = 1） | 0 |
| 复活任务记录 | 0 |
| 活跃做种行 | 0 |

埋点是用来**统计真人行为**的，而这个库一个真人都没有。**埋点应后置到"准备上线时"再做。**

**3. 已建的表还是半成品状态** —— `social_team` / `social_team_member` 建好了却没有接口，
表空着不用比不建更糟（会让人以为是遗留物）。所以下一步应该是**让表活起来**。

---

## 二、组队接口（3 个）

设计原则：**复用 `resurrections` 作为任务本体**，组队只是给它挂 `team_id`。

| 接口 | 作用 |
|---|---|
| `POST /api/v1/social/team/create` | 建队 + 认领该资源的复活任务 + 队长入队（记贡献基线） |
| `POST /api/v1/social/team/join` | 加入队伍（记贡献基线） |
| `GET  /api/v1/social/team/mine` | 我的队伍（含成员与各自契约期内的做种增量） |

单人任务（`team_id IS NULL`）走原有的 `/resurrections/claim`，行为完全不变。

### 校验链（create / join 共用口径）

| 校验 | 依据 |
|---|---|
| 种子必须已过审 | `approval_status = 1` |
| **不能拯救自己发布的种** | 与 `resurrections/claim` 同口径（U3D Graveyard） |
| **必须已下载过该资源** | `snatches` 里有行 —— 没下载就做不了种，加入无意义；同时天然防"空手蹭贡献" |
| 一个资源同时只有一个任务 | `resurrections.torrent_id` 唯一约束（`ON CONFLICT DO NOTHING`） |
| 队伍未满 / 未结束 / 未重复加入 | `social_team.status`、`social_team_member` 主键 |

### 贡献口径（关键）

**不使用 `spark_ledger` 反查**——`seeding_reward` 按用户逐小时聚合落库、无 `ref_type/ref_id`，
定位不到单个种子。改为**加入时快照 `snatches` 累计值作基线，结算时算增量**，
与 `seed_preserve` 的 `seed_time_begin` / `uploaded_begin` 同款口径。

好处：精度到单资源，**天然杜绝"挂一堆无关种子刷契约贡献"**。

`/social/team/mine` 直接返回每人的 `delta_seconds = GREATEST(snatches.seeded_seconds - 基线, 0)`。

### 事务

`create` 要跨三张表写（`social_team` → `resurrections` → `social_team_member`），
用显式事务；冲突时 `return Err` 让事务随作用域自动回滚。

---

## 三、过程中挖出的两个类型 bug

两个都是**编译期发现不了、只在运行时报 500** 的类型不匹配。

### Bug 1：`site_settings.value::int` 读成了 `i64`

```
error occurred while decoding column 0: mismatched types;
Rust type `i64` (as SQL type `INT8`) is not compatible with SQL type `INT4`
```

`value::int` 得到的是 **INT4**，必须读 `i32`。全项目有 5 处这个模式：

| 位置 | 读法 | 结论 |
|---|---|---|
| `http.rs:6160` `upload_deny_limit` | `i32` | ✅ 正确 |
| `http.rs:6340` `upload_auto_promo_days` | `i32` | ✅ 正确 |
| `torrents.rs:919` `upload_price_tax` | `i32` | ✅ 正确 |
| **`ops_http.rs:1020` `resurrection_hours`** | **`i64`** | ❌ **既有 bug** |
| `social_http.rs` 新增（抄自上面那处） | `i64` | ❌ 本轮新写，已修 |

**`ops_http.rs` 那处的危害更隐蔽**：它后面跟着 `.unwrap_or(240)`，查询失败被静默吞掉，
表现为「**`resurrection_hours` 配置永远不生效、恒为 240**」——不报错、不崩溃，只是配置改了没反应。

**已一并修复**（改为 `i32`，与其余三处一致）。修复无行为风险：配置表里根本没有这个键，
`COALESCE(..., 240)` 本来就返回 240。

> 这条已写进 `fluxtorrent-change-verify` 技能：**排查任何"配置改了没反应"的问题，先查这里。**

### Bug 2：`snatches.seeded_seconds` 是 INT，不是 BIGINT

```
同上的 i64 / INT4 报错
```

`snatches` 表里 `uploaded BIGINT` 但 **`seeded_seconds INT`**（见 `0001_init.sql`），
我按 BIGINT 读成 i64。修法是在 SQL 里显式 cast：`COALESCE(seeded_seconds, 0)::bigint`。

**教训**：写涉及金额/时长的 SQL 前，**先用 `information_schema.columns` 把所有列的真实类型拉出来核对一遍**，
比"编译 → 重建容器 8 分钟 → 报错 → 再改"便宜得多。本轮就是靠这一步一次性排除了剩余风险：

```sql
SELECT table_name, column_name, data_type FROM information_schema.columns
WHERE table_name IN ('social_team','social_team_member','resurrections','snatches')
ORDER BY table_name, ordinal_position;
```

---

## 四、端到端测试结果（8/8 通过）

用开发库里**真实的死种**（id=1，实际资源名「【官种】2025 人教版高中数学必修一 全套精讲视频 1080P」）
作对象，为两个测试用户构造 `snatches` 记录模拟"下载过"，跑完整链路后**清理全部测试数据并恢复开关**。

| # | 用例 | 结果 |
|---|---|---|
| 1 | user2 发起组队 | ✅ 200，返回 `team_id=1 / torrent_id=1 / required_hours=240 / seed_seconds_begin=0` |
| 2 | 边界：同一资源重复建队 | ✅ 400「该资源已有进行中的复活任务」（`torrent_id` 唯一约束生效） |
| 3 | 边界：队长加入自己的队伍 | ✅ 400「你已经是这支队伍的队长」 |
| 4 | user3 加入队伍 | ✅ 200 |
| 5 | 边界：重复加入 | ✅ 400「你已在这支队伍中」 |
| 6 | user2 查我的队伍 | ✅ 200，1 支队伍 / 2 名成员，队长标记与 `delta_seconds` 正确 |
| 7 | 数据库落库核对 | ✅ `social_team` 1 行 / `resurrections` 1 行（`team_id=1`）/ `social_team_member` 2 行 |
| 8 | 清理与恢复 | ✅ `social_team=0`、`resurrections=0`、`module_social=no` |

顺带验证了 Bug 1 的修复：返回的 `required_hours = 240` 是配置默认值（`site_settings` 里本无该键），
说明类型修复后 `COALESCE` 分支能正常走通。

测试脚本：项目根 `_team_e2e.py`（可重复执行，自带清理）。

---

## 五、下一步

| # | 任务 | 说明 |
|---|---|---|
| 1 | 队伍的前端页面 | 招募列表 / 我的队伍 / 加入按钮 |
| 2 | worker 契约结算 job | 幂等键 `social:rescue:{team_id}`；按 `social_team_member` 的增量分账 |
| 3 | 埋点 | **等准备上线时再做**（现在没有真实用户可统计） |
| 4 | 赛季结算 job | 幂等键 `social:season:{season_id}:{uid}` |
