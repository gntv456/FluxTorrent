# 质量基建（覆盖率与浏览器 E2E，E14）

> 基线先行、不设阈值：先让数字可见且可复跑，禁阈值（防「为凑数写无断言测试」）。
> 阈值提案待数字稳定 2-3 个迭代后再议。

## 覆盖率

### web（vitest + v8）

```bash
cd apps/web && npx vitest run --coverage
```

- 口径：共享层全目录（components/i18n/lib，含未被测试触达的文件按 0 计——诚实口径）；
- 产物：终端表 + `coverage/`（HTML 可点开逐文件看未覆盖行）+ lcov（供 CI 服务消费）；
- **基线（2026-09-29，144 测）**：共享层全目录 ~2.4%——数字低是口径诚实（全量分母），
  不代表测试少；被测文件内覆盖率见 HTML 报告；
- 配置：`apps/web/vitest.config.ts` coverage 段。

### api/worker/tracker（cargo-llvm-cov）

```bash
cargo install cargo-llvm-cov --locked   # 一次性
cd apps/api && cargo llvm-cov           # debug profile，首次编译较慢
```

- 产物：终端逐 crate 报告 + `target/llvm-cov-html/`（`cargo llvm-cov --open` 直接看）；
- 解读：单测集中在纯函数域（i18n/promo/sections/gacha-math），HTTP 层由 31 个
  e2e python 脚本覆盖——llvm-cov 数字只反映 `#[test]` 触达，两套口径并列看。
- **基线（2026-09-29，144 测）**：api crate region 7.91%（HTTP handler 文件
  0% 属预期——由 e2e 承担）。

## 浏览器 E2E（Playwright）

```bash
cd apps/web
npx playwright test            # 需本地栈在跑（docker compose up）+ root 账号
```

- 定位：**浏览器层冒烟**——真实 Chromium/Edge 里登录走通、关键页非白屏；
  深链路（发种/支付/H&R）仍由 `scripts/*_e2e.py` 的 API 级 e2e 承担，不重复造用例；
- 首套用例（e2e/smoke.spec.ts）：登录跳转 / 资源库渲染 / 等级页（旧镜像自动 skip）；
- 浏览器：默认系统 Edge（`channel: msedge`）——本机 chromium 修订号与包版本不齐时
  免下载；可用 `E2E_CHANNEL=chromium` 覆盖；
- 失败产物：`test-results/`（截图+trace.zip，`npx playwright show-trace` 回放）；
- CI：暂不阻断（本地手动跑）；纳入 CI 的前置是 CI runner 有浏览器与栈编排。

## 纪律

1. 新增纯函数/叶子组件 → 配 vitest 单测（进覆盖率分母且拉数字）；
2. 新增页面级功能 → 冒烟用例补一条 Playwright（登录态可观察信号）；
3. 覆盖率数字只升不降（评审参考项，非硬门禁——降了要说明原因）。
