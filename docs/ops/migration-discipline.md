# 迁移纪律：只增不改（2026-11-20 百灵站宕机复盘）

## 铁律

**已应用的迁移文件，内容与文件名一律冻结。新变更 = 新版本号的幂等迁移。**

sqlx 启动时逐行校验 `_sqlx_migrations` 里记录的 sha384 与镜像内迁移文件是否一致，
不符即拒绝启动：

```
Error: migration 499 was previously applied but has been modified
```

API 挂 → compose 依赖链把 tracker/worker 一起卡死 → 全站 500。

## 事故链（2026-11-20，记档防忘）

1. f06c024a（11-19）修改了**已应用**的 `0499_massmail_audience_retain.sql`
   （min_class 99→98）。
2. 热修只 UPDATE 了**好学库**的 499 checksum，**百灵库（sub2api-server
   独立栈）漏了**——当时误信「百灵与好学同栈同源」（11-15 假阳性结论，
   验证手法不合格：`curl --resolve` 打到的是默认 vhost，非品牌级判据）。
3. dev.38 tag → webhook 自动发版到百灵机 → sqlx 校验炸 → 崩溃循环 6 小时，
   期间站点品牌回落「FluxTorrent · PT」、API 全 500、announce 死。
4. 部署失败只写 webhook 本地日志，无告警，无人知晓。

## 正确姿势

| 需求 | 做法 |
|---|---|
| 修正已应用迁移的数据效果 | 新增幂等迁移（先例：`0509_contract_panel_minclass_fix.sql`） |
| 新功能需要 schema 变更 | 新增 `NNNN_slug.sql`（编号顺延） |
| 已应用迁移有笔误想改注释 | **不改**。注释错了就让它错着，或新增迁移里补注释性 INSERT/UPDATE |
| 各站点库 checksum 对账 | 见下「对账卡」 |

## 守卫

`scripts/migrations_frozen_guard.mjs` + `scripts/migrations_frozen_baseline.json`
（2026-11-04 立的冻结门禁：基线全量 sha384，CRLF→LF 归一后与 sqlx 同口径，
已接 CI `build-test.yml` 与 `scripts/ci_local.sh`）。

- 改冻结文件（含注释） / 删除已登记文件 → 红
- 新增迁移未登记 → WARN 放行（入库时跑
  `node scripts/migrations_frozen_guard.mjs --touch <文件名>` 显式登记）
- 2026-11-20 百灵宕机复盘时重复建设的 `migration_freeze_guard` 已退役
  （功能被本守卫覆盖）；0509 事故善后时已把基线对齐方案 A（0499=e277）
  并补登记全部 496 个迁移。

## 站点清单（checksum 对账的对象）

**多站部署，迁移对账必须逐库做，不依赖任何「同栈」结论**（拓扑结论会过期，
2026-11-15 的「百灵=好学同栈」结论 11-20 被证伪）：

| 站点 | 宿主 | 栈路径 | DB |
|---|---|---|---|
| 好学 www.hxpt.org | hxpt-prod (159.195.247.176) | `/opt/fluxtorrent/docker` | flux-postgres/fluxtorrent |
| 百灵 ft.ptang.top | sub2api-server (37.120.176.130) | `/root/FluxTorrent/docker` | flux-postgres/fluxtorrent |

验证拓扑用**品牌级判据**：站点 title / `/api/v1/site-profile` 的 `brand` 字段，
不是「返回了 flux 页面」。

## checksum 对账卡（发版后跑一遍）

```bash
# 1) 镜像内期望值（N = 最高迁移版本）
docker run --rm --entrypoint sh ghcr.io/gntv456/fluxtorrent/api:<tag> \
  -c 'sha384sum /app/migrations/*.sql' > /tmp/expected.txt

# 2) 各库实际值
ssh <host> "docker exec flux-postgres psql -U flux -d fluxtorrent -tAc \
  \"SELECT version, encode(checksum,'hex') FROM _sqlx_migrations ORDER BY version;\""

# 3) 不一致（仅限「文件改过但语义幂等」的对账场景，正常发版不该出现）：
ssh <host> "docker exec flux-postgres psql -U flux -d fluxtorrent -c \
  \"UPDATE _sqlx_migrations SET checksum=decode('<hex>','hex') WHERE version=<N>;\""
```

## 部署失败告警

webhook 部署失败会静默（只写本地日志）。两台服务器装有
`/usr/local/bin/flux-stack-watchdog`（cron 每 5 分钟）：检测 api 容器
restart-loop / 非 healthy 即发信（Brevo relay）并附 `docker logs` 尾部。
安装/测试方法见 `scripts/flux-stack-watchdog.sh` 头注。
