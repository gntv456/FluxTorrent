// 模块键三方一致性检查（四审 L5：0197 加 invites 时只改了 Rust，TS 清单漏改而无人报错）
//
// 对比三处：
//   1) Rust  apps/api/src/modules.rs 的 `key::ALL`（网关/worker/缺省回落都看它）
//   2) TS    packages/domain-types/src/index.ts 的 `MODULE_KEYS`（前端 mod() 的键空间）
//   3) DB    modules 表（注册表真身；可选——连不上库时跳过并说明，不算失败）
// 用法：node scripts/module_keys_guard.mjs
import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";

const RS = "apps/api/src/modules.rs";
const TS = "packages/domain-types/src/index.ts";

// Rust：key 常量定义 `pub const X: &str = "k";` + ALL 列表引用大写名
const rs = readFileSync(RS, "utf8");
const constMap = new Map();
for (const m of rs.matchAll(/pub const ([A-Z_]+): &str = "([a-z_]+)";/g)) {
  constMap.set(m[1], m[2]);
}
const allBlock = rs.split("pub const ALL")[1] ?? "";
const rustKeys = [...allBlock.matchAll(/^\s*([A-Z_]+),/gm)]
  .map((m) => constMap.get(m[1]))
  .filter(Boolean);

// TS：MODULE_KEYS 数组里的字符串字面量
const ts = readFileSync(TS, "utf8");
const arrBlock = (ts.split("MODULE_KEYS = [")[1] ?? "").split("] as const")[0];
const tsKeys = [...arrBlock.matchAll(/"([a-z_]+)"/g)].map((m) => m[1]);

function diff(a, b) {
  const f = (x, y) => x.filter((k) => !y.includes(k));
  return { onlyA: f(a, b), onlyB: f(b, a) };
}

let bad = [];
const rt = diff(rustKeys, tsKeys);
console.log(
  `Rust ${rustKeys.length} 键 / TS ${tsKeys.length} 键`,
);
if (rt.onlyA.length || rt.onlyB.length) {
  bad.push(`Rust↔TS 漂移：只有 Rust 有 [${rt.onlyA}] / 只有 TS 有 [${rt.onlyB}]`);
}
if (rustKeys.join() !== tsKeys.join() && !rt.onlyA.length && !rt.onlyB.length) {
  bad.push("Rust 与 TS 键集合相同但顺序不同（契约测试按长度、按序比对会误判）");
}

// DB 侧可选
try {
  const out = execFileSync(
    "docker",
    ["exec", "flux-postgres", "psql", "-U", "flux", "-d", "fluxtorrent",
     "-tAc", "SELECT key FROM modules ORDER BY key"],
    { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] },
  );
  const dbKeys = out.split("\n").map((s) => s.trim()).filter(Boolean).sort();
  const d1 = diff(rustKeys.slice().sort(), dbKeys);
  if (d1.onlyA.length || d1.onlyB.length) {
    bad.push(`Rust↔DB modules 表漂移：只有代码有 [${d1.onlyA}] / 只有库里有 [${d1.onlyB}]`);
  } else {
    console.log(`DB modules 表 ${dbKeys.length} 键：与代码一致`);
  }
} catch {
  console.log("DB 侧跳过（连不上 flux-postgres；静态比对仍然生效）");
}

for (const b of bad) console.error("FAIL " + b);
if (bad.length) process.exit(1);
console.log("OK: 模块键清单一致");
