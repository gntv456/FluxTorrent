# 贡献指南（CONTRIBUTING）

感谢关注 FluxTorrent！本文说明开发环境、提交规范与质量门禁。

## 开发环境

```bash
# 基础设施
docker compose -f docker/docker-compose.yml up -d postgres redis

# API（自动执行迁移）
DATABASE_URL="postgres://flux:<密码>@127.0.0.1:5432/fluxtorrent" \
REDIS_URL="redis://:<密码>@127.0.0.1:6379" \
JWT_SECRET="dev_secret_change_me_at_least_32_bytes!" \
cargo run -p flux-api

# Web
pnpm install && pnpm --filter @fluxtorrent/web dev
```

## 质量门禁（PR 必须全绿，与 CI 一致）

```bash
cargo fmt --all --check     # 格式
cargo check --workspace --all-targets   # 零警告基线
cargo test --workspace      # 单测
pnpm lint                   # ESLint（error 阻断；warning 存量 7 处不阻断）
node scripts/line_limit_guard.mjs    # 行数（≤300）与行宽（≤80）门禁（见纪律第 9 条）
pnpm --filter @fluxtorrent/web exec tsc --noEmit   # 前端类型
pnpm --filter @fluxtorrent/web exec next build     # 前端构建
node scripts/i18n_guard.mjs # i18n 防新增硬编码中文
```

## 关键工程纪律（违反会被 review 拒绝）

1. **迁移文件不可变**：`apps/api/migrations/` 中已合并的迁移禁止改名/改号/改内容——
   sqlx 按版本号 + checksum 记账，任何改动都会让存量部署启动失败。撞号时把自己的
   迁移挪到更大的号（如 0143+），永远不要让已存在的编号让位。
1a. **建号前先查目录、建号后立刻提交占位**（防撞号，2026-10-09 事故教训）：
   - 新建迁移前**必须**先 `ls apps/api/migrations/ | sort | tail` 取当前最大号，
     号 = 最大号 + 1。**并行开发时"我看到的最大号"可能已经过时**。
   - 建号后**立即 `git add` 该文件并提交占位**（哪怕内容是 `SELECT 1;` 后补）。
     未提交的新号文件最容易被另一方在不知情下复用同一号。
   - 撞号表现：`ls migrations/ | awk -F_ '{print $1}' | uniq -d` 有输出。
     此时**让号的一方改名到更大的空号**，并把 `_sqlx_migrations` 里该 version 的
     `description` 修正为实际归属（sqlx 只按 version 记账，同号文件会互相覆盖）。
   - 已应用过的迁移改名后，需在目标库**删除 `_sqlx_migrations` 旧记录**（checksum
     不符会启动报错），再 rebuild api 让新号以幂等 SQL 重跑。
   - **⚠️ 归属只看 checksum，别看 description**（2026-10-09 第三次撞号实测）：
     sqlx 的 `description` 取自「最后一次扫描到的文件名」，`checksum` 也同批覆盖，
     二者可能**分属不同文件**。诊断：
     `python -c "import hashlib;print(hashlib.sha384(open('migrations/0330_x.sql','rb').read()).hexdigest())"`
     与 `SELECT encode(checksum,'hex') FROM _sqlx_migrations WHERE version=N` 比对，
     相等那个文件才是该号的真实归属。
   - **撞号覆盖发生在 build 时、不只在建号时**：本批 330 号我先应用（03:25），
     对方 20 分钟后才建同号文件，但**下次 `build api` 扫目录时才把 330 的
     checksum 换成对方的**，api 随后 crash-loop。⇒ 光「建号前查目录」不够；
     **每次 build 前重跑一次** `ls migrations/ | awk -F_ '{print $1}' | uniq -d`。
     已应用方能不改内容、直接让号到更大空号（SQL 幂等可重跑）。
   - **⚠️ 让号/重排必须过三视角**（2026-10-09 第四次撞号让号 0330→0333
     实测漏了第二视角，空库装机第一步即崩，见 f3afef2）：
     ① 存量库——checksum 对齐（该提交做了）；
     ② **空库——文件序依赖**：让号把建表挪到更小号之后，所有 `REFERENCES`
     它的迁移就断链（0332 引用 0333 才建的表）。让号后必须 grep 一遍
     「谁引用被挪的表」并确认引用方新序号在被挪文件**之前**；
     ③ CI/本地空库闸门——`install_e2e.py` 真跑一遍（`sed ROLLBACK`
     干跑只验语法不验顺序）。另：让号若伴随**内容修改**（如本次 0332
     前置建表），存量库 checksum 必失配 ⇒ 配套
     `scripts/align_migration_checksums_*.sql` 对账脚本 + upgrade 文档
     写明执行时机，缺一不可。
1b. **改迁移后必须 rebuild api 镜像，不能只 restart**（2026-10-09 事故教训）：
   容器内 `apps/api/src/main.rs:145` 用 `Migrator::new(Path)` **运行时读
   `./migrations`**，但该目录在容器里是 **build 时的镜像拷贝**。只 `restart`
   读到的仍是旧文件（现象：迁移 `success=t` 但数据没变）。改 .sql 后必须
   `docker compose -f docker/docker-compose.yml build api` +
   `up -d --no-deps api`。迁移必须幂等（rebuild 后会重跑已改动的号）。
1c. **JSONB 只读一次构造、别链式累加**：`jsonb_set(obj,'{a,b}',v,true)` 的第 4 参
   `true` **只控制"值已存在时是否替换"，父路径不存在时不会创建中间层级**（直接返回
   原对象）；`jsonb_agg(...)` 过滤后**空集返回 NULL**，`NULL || x` 仍 NULL。
   两者叠加会把整段 JSONB 抹成 NULL/`{}`。构造多层 JSONB 用
   `jsonb_build_object` 一次性建；聚合前 `COALESCE(agg, '[]'::jsonb)`。
2. **资金动账必须走统一管线**：`spend_spark`/`earn_spark`（或 `_tx` 事务版），
   禁止直接 UPDATE 余额；`SpendOutcome` 是 `#[must_use]`——丢弃返回值继续执行
   副作用等于打开幂等重放印钞口。
3. **扣款与副作用同事务**：参照 `bank_deposit` 现版实现；禁止「先扣款提交、
   再写业务行、失败靠补偿」的两段提交。
4. **快照与流水双写**：users 上的余额/流量是快照，权威在 `spark_ledger`/
   `traffic_ledger`；只改快照不落流水的变更会被 6h 对账静默回滚。
5. **handler 不写裸 SQL 拼接**：参数一律 bind；动态表名/排序字段走白名单 match。
6. **新端点必须有守卫**：受保护资源第一步 `require_auth`；staff 端点用 `staff()`
   （authz.rs）。e2e-smoke 的匿名列矩阵会拦住漏守卫的端点。
7. **错误走信封**：所有响应 `{code, message, data, request_id}`；不要在 handler
   里手搓 HttpResponse 绕过 `ok()`/`DomainError`。
8. **组件内禁止裸 fetch**：前端统一走 `lib/api-client.ts`；凭证是 HttpOnly cookie，
   不要把 token 放进 localStorage 或 JS 可读的 cookie。
8a. **首屏拉后端的页面必须退避重试 + 透传业务码**（2026-10-09 事故教训）：
   api 进程**先跑完全部 sqlx 迁移才 `bind` 端口**（`main.rs` 迁移在
   `HttpServer::bind` 之前），330+ 个迁移意味着全新装机 / 重建卷后有几十秒到
   几分钟「进程活着但不监听任何端口」的窗口，而 web 容器此时已 healthy、页面能
   渲染。因此：① 禁止 `useEffect` 里一次性 fetch 完就 `catch(() => setError("…
   请确认 API 可达"))`——那既不自愈（站长只能手动刷页面）又销毁诊断线索
   （`ApiError.code` 被丢掉，`1003` web→api 网关不通与浏览器网络故障渲染成同一句
   话，处置方式完全不同）。② 正确形态见 `app/setup/_parts/use-setup-status.ts`：
   退避重试（1→2→4→8→15→30s，上限约 3 分钟）+ 按 `code` 分类给处置建议 +
   手动重试兜底 + 加载态渲染（**别在 loading 时 `return null`**，否则冷启动期是
   纯白页，站长连标题都看不到，只会觉得站点坏了）。
9. **文件行数与行宽只许瘦不许胖**：源码软上限 Rust/TS/TSX/JS/MJS 300 行、
   Python 500 行、CSS 3000 行；行宽 ≤80 字符（`scripts/line_limit_guard.mjs`
   门禁；例外=迁移/i18n 字典/锁/生成物/domain-types 契约）。新文件超限或
   出现超宽行直接拒；存量超限文件（见 `scripts/line_limit_baseline.json`）
   可以改、鼓励拆分变短，但行数或超宽行数比基线更多即拒——拆分落地后跑
   `node scripts/line_limit_guard.mjs --update` 收缩基线，基线只许变短
   （重新 --update 前先 review diff）。Rust 侧 `rustfmt.toml` 已对齐
   `max_width = 80`，`cargo fmt` 后即行宽合规。

## 提交规范

- 格式：`type(scope): 摘要`（中文摘要可），type ∈ feat/fix/docs/refactor/test/chore/ci，
  scope 参考：api/web/worker/tracker/db/ci/ops/docs。
- 一次提交做一件事；修复与重构分开。
- 涉及迁移的提交必须在 commit message 里说明幂等性（重跑是否安全）。

## 测试期望

- 纯函数（概率/计费/解析）必须有单测；经济类改动补 EV 或幂等断言。
- 新端点在 e2e-smoke 的冒烟清单里加一行 probe。
- 修 bug 先写会失败的测试（能自动化时），再修。
