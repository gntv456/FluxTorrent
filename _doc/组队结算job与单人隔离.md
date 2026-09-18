# 组队结算 job 与单人任务隔离

> 承接 `_doc/组队契约接口与类型Bug修复.md`
> 组队接口做完后，队伍能建、能做种，但**没有结算**——闭环差最后一环。本文记录实现过程，
> 以及动手前发现的一处**会重复发奖的冲突**。

---

## 一、动手前的发现：与现有单人结算冲突（必须先隔离）

`apps/worker/src/jobs.rs:920` 的 `resurrection_settle`（0073，每小时跑）：

```sql
UPDATE resurrections r SET status = 'done', finished_at = now()
WHERE r.status = 'open'
  AND EXISTS (SELECT 1 FROM snatches s
              WHERE s.user_id = r.user_id AND s.torrent_id = r.torrent_id
                AND s.seeded_seconds >= r.required_hours * 3600 AND s.seeding)
RETURNING r.id, r.user_id, r.torrent_id, r.reward_sparks
```

判据是「**领取者本人**做到 `required_hours` 且仍在做种」，奖励**全额**给 `r.user_id`：
火花 + 1 枚免费券 + 该种 7 天 free bump + 站内信。

**冲突**：组队任务的 `resurrections.user_id` 是**队长**。于是：

1. 队长个人做种达标 → 单人结算给他发**全额 5000**
2. 我的组队结算再给全队分一次 5000
→ **队长被双发，其他队员被绕开。**

**处置**：给 `resurrection_settle` 加一行 `AND r.team_id IS NULL`——单人任务（`team_id IS NULL`）
走原路径完全不变，组队任务交给新 job。改动 1 行，向后兼容。

---

## 二、组队结算 job（`social_team_settle`）

挂在 hourly tick，紧跟单人结算之后：

```rust
with_lock(&db, "job:resurrection_settle", resurrection_settle(&db)).await;
with_lock(&db, "job:social_team_settle", social_team_settle(&db)).await;
```

### 结算判据

| 条件 | 说明 |
|---|---|
| `social_team.status IN (0,1)` 且 `settled_at IS NULL` | 未处理过的活跃队伍 |
| 关联的 `resurrections.status = 'open'` | 任务未结束 |
| **至少一名成员当前仍在做种** | 防止"做够了就撤"后资源再次死掉 |
| **团队增量合计 ≥ `required_hours × 3600`** | 采纳"团队总和"而非"每人各自达标" |

最后一条是刻意的：单人任务要求"一个人做 240 小时"，组队任务的语义应该是
「**让这个资源被持续保种 240 小时**」——5 个人分摊各 48 小时同样达成了目标。
组队的好处是**分摊**，不是拔高总量。

### 分账

按每个成员的契约期增量 `delta = seeded_seconds - 基线` 占比分配：

```
amount_i = reward × delta_i / Σdelta        （整数除法）
余数补给贡献最多的人                          （保证分配总额恰好 = reward）
delta = 0 的成员不分账，但仍回填 contributed_sec
```

写 `spark_ledger`（`kind = 'social_team_reward'`，幂等键 `social:team:{team_id}:{uid}`）
并同步 `users.spark_balance`——与现有 `resurrection_settle` 同款两段式写法。

### 其余动作

| 动作 | 落点 |
|---|---|
| 拯救荣誉 | `rescue_honor`（`uids` 数组、`total_sec`、`seeders_after`），**永久留存** |
| 状态推进 | `social_team.status = 2` + `settled_at`；`resurrections.status = 'done'` |
| 成员回填 | `contributed_sec` / `settled_amount`；`join_status = 4`（完成） |
| 站内信 | 逐个通知成员 |

---

## 三、可靠性：比现有实现更稳一点

现有 `resurrection_settle` 的注释说「CAS 已置 done 后崩溃，重启重跑此循环仍能凭幂等键补发」——
但实际只在**同一次循环内**成立：`status` 一旦置 `done`，下次 job 运行的 `WHERE status='open'`
就匹配不到，奖励会永久丢失。

组队结算换了个更稳的写法：

> **以 `social_team.settled_at IS NULL` 作为「未处理」标记，而不是靠状态 CAS。**

- 发奖、写荣誉、置 `settled_at` **全在同一事务**内
- 中途崩溃 → 事务回滚 → `settled_at` 仍为 NULL → 下一轮**重新处理**
- 幂等键 `social:team:{team_id}:{uid}` 兜底，保证重跑不会重复发放

这样"崩溃丢奖"和"重跑双发"两个方向都堵住了。

---

## 四、端到端测试结果（全通过）

**造数**：队伍（队长 user2 + 队员 user3），团队做种增量 900000s（阈值 `240h × 3600 = 864000s`），两人均 `seeding = true`。
**触发**：`docker restart flux-worker` → 首轮 hourly tick 立即执行。

| 校验项 | 结果 |
|---|---|
| `resurrections.status` | ✅ `done` |
| `social_team` | ✅ `status = 2`，`settled_at` 已置位 |
| **成员分账** | ✅ user2: 500000s → **2777**；user3: 400000s → **2223**（合计恰好 **5000**） |
| 奖励流水 | ✅ 两条 `kind = social_team_reward` |
| 用户余额 | ✅ 两人分别到账 |
| 拯救荣誉 | ✅ `uids = {2,3}`、`total_sec = 900000` |
| 站内信 | ✅ 2 条 |
| **幂等复测** | ✅ 再重启 worker 一次：流水仍 2 条、总额仍 **5000**、荣誉仍 1 条 —— **无重复发放** |
| 清理与回滚 | ✅ 各表归零，余额还原到初始值 |

**可靠性设计得到验证**：`settled_at IS NULL` 作「未处理」标记 + 发奖同事务，重跑既不丢奖也不重发。

### 测试过程中踩的坑（脚本层，非后端）

`psql -c` 执行 INSERT / UPDATE / DELETE 时，**结果是第一行、命令标签（`INSERT 0 1`）在最后一行**。
我一开始直接 `stdout.strip()` 取整段，`team_id` 变成了 `"3\nINSERT 0 1"`，后续 SQL 全部错位，
表现为"接口没报错但数据没写进去"。正确做法是取**第一个非空行**。

---

## 五、下一步

| # | 任务 | 说明 |
|---|---|---|
| 1 | 队伍的前端页面 | 招募列表 / 我的队伍 / 加入按钮 |
| 2 | 契约失败与超时的处理 | 目前只实现了"成功结算"，未实现"到期未达标"的失败流转与信誉扣减 |
| 3 | 赛季结算 job | 幂等键 `social:season:{season_id}:{uid}` |
| 4 | 埋点 | 准备上线时再做 |

> 注：`social_reputation` 表已建但尚无读写逻辑——信誉的加减应随"契约完成/失败/中途退出"落地，
> 与第 2 项一起做更合适。
