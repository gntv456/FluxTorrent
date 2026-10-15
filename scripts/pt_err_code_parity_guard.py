# -*- coding: utf-8 -*-
"""错误码三方对拍闸门（FRAME §9 验收清单 #2）

错误码的真源是 `apps/api/src/errors.rs` 的 `code()` 映射，另有两条派生
链路各管一份集合：

  1. `apps/api/src/i18n.rs` 的 `MESSAGES` 表 —— 信封 message 的三语文案
  2. `packages/domain-types/src/index.ts` 的 `ErrorCode` 表 —— 前端消费端

**为什么必须有这道闸门**：三者此前静默漂移，且每一处都只有一侧知道：
  - 4102（PoolGuard）在 `code()` 里有，但 i18n 表漏登记 ⇒ 出口回落到
    1000「内部错误」，把「池子保底返还率 >100%」这件运营侧该修的事
    说成了服务端故障；
  - 1001 / 2008 / 3001 / 3002 / 5001 在手写 TS 里躺着，Rust 侧从没有
    这么几个变体 ⇒ 零消费点的死声明，任何按它们写的分支都是死代码
    （这 5 条已于 2026-10-16 清掉）。

判据（CI 一步，exit 1 = 漂移）：
  A. **TS ⊆ Rust**：前端表里每个码都必须在 `code()` 里真实存在。死声明
     会被这条当场抓住。
  B. **Rust ⊆ i18n**：`code()` 返回的每个码都必须有三语文案。漏登记的
     会回落到 1000「内部错误」，等于把可读错误说成服务端故障。
  C. **生成物新鲜度**：`error-codes.generated.ts` 必须与 `code()` 同步。

i18n 表允许有 Rust 之外的码（`localized_message(code)` 可被分支直接
调用，如 2012 登录 PoW 的预留文案），故 A 方向不查 i18n。

红测：把 TS 表里塞一个 Rust 没有的码（如 `BOGUS: 9999`）必须被 A 拦。
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary, FAILURES  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ERRORS = "apps/api/src/errors.rs"
I18N = "apps/api/src/i18n.rs"
TS = "packages/domain-types/src/index.ts"
GEN = "packages/domain-types/src/error-codes.generated.ts"


def read(rel):
    p = os.path.join(ROOT, rel)
    if not os.path.exists(p):
        FAILURES.append("ERRC 取文失败 %s" % rel)
        return ""
    return open(p, encoding="utf-8", errors="replace").read()


def rust_codes():
    """`code()` 映射里真实存在的码集合（真源）。"""
    src = read(ERRORS)
    m = re.search(r"fn code\(&self\)[^{]*\{(.*?)\n    \}", src, re.S)
    if not m:
        FAILURES.append("ERRC 解析不到 code() 映射")
        return set()
    return set(re.findall(
        r"DomainError::\w+\s*(?:\([^)]*\))?\s*=>\s*(\d{4})", m.group(1)))


def i18n_codes():
    src = read(I18N)
    m = re.search(r"const MESSAGES[^=]*=\s*&?\[(.*?)\n\];", src, re.S)
    if not m:
        FAILURES.append("ERRC 解析不到 MESSAGES 表")
        return set()
    return set(re.findall(r"\(\s*(\d{4})\s*,", m.group(1)))


def ts_codes():
    src = read(TS)
    m = re.search(r"export const ErrorCode = \{(.*?)\n\}", src, re.S)
    if not m:
        FAILURES.append("ERRC 解析不到 ErrorCode 表")
        return set()
    return set(re.findall(r":\s*(\d{4}),", m.group(1)))


def gen_codes():
    src = read(GEN)
    return set(re.findall(r"^\s+[A-Z_0-9]+:\s*(\d{4}),", src, re.M))


def sorted_nums(xs):
    return sorted(xs, key=int)


print("== 错误码三方对拍（真源 errors.rs::code()） ==")

R = rust_codes()
I = i18n_codes()
T = ts_codes()
G = gen_codes()

ok("ERRC0 真源可解析（code() 映射非空）", len(R) > 0, "R=%d" % len(R))

# A. TS ⊆ Rust（死声明）
ts_ghost = T - R
ok("ERRC1 前端 ErrorCode 表无幽灵码（TS ⊆ Rust）",
   not ts_ghost,
   "后端从无此码: %s" % sorted_nums(ts_ghost))

# 反向：Rust 有而 TS 无 = 前端少声明（不是错，允许子集）
# A2. 明确提示规模，避免「表空了也算过」
ok("ERRC2 前端表规模合理（≥后端 80%%）",
   len(T) >= len(R) * 8 // 10,
   "TS=%d / Rust=%d" % (len(T), len(R)))

# B. Rust ⊆ i18n（漏文案 ⇒ 回落 1000「内部错误」）
i18n_gap = R - I
ok("ERRC3 每个后端码都有三语文案（Rust ⊆ i18n）",
   not i18n_gap,
   "回落成「内部错误」的码: %s" % sorted_nums(i18n_gap))

# C. 生成物新鲜度
ok("ERRC4 生成物与真源同步（error-codes.generated.ts）",
   G == R,
   "生成物多: %s / 少: %s"
   % (sorted_nums(G - R), sorted_nums(R - G)))

# D. i18n 孤儿（允许存在但登记在案，避免无声堆积）
orphans = I - R
if orphans:
    print("  NOTE i18n 预留给非 code() 直调的码: %s" % sorted_nums(orphans))
ok("ERRC5 i18n 孤儿码不超过 5 个（有登记即不算漂移）",
   len(orphans) <= 5, "孤儿 %d 个" % len(orphans))

summary()
