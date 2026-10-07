#!/usr/bin/env node
/** 任务目录三向一致门禁（0295，运营轮 P1-1 / P1-6）。
 *
 *  一个 job 的名字同时活在三个地方，历来靠人肉同步（catalog.rs 顶部
 *  那句注释就是承认这一点）：
 *    1) apps/worker/src/jobs/catalog.rs  的 JOBS —— 面板可见清单
 *    2) apps/worker/src/jobs/manual.rs   的 run_named —— 手触分派
 *    3) apps/worker/src/jobs/run.rs      的 shard_lock!("job:X") —— 定时调度
 *  三边任何一对不齐，表现都不一样，而且都不报错：
 *    缺 2 ⇒ 站长点「执行」永远「未知任务」（实测 usage_stats：入队回 200，
 *           job_status.ok=false，看起来像跑过了）；
 *    缺 1 ⇒ 任务照跑但面板看不见、不能手触（实测 request_expire）；
 *    缺 3 ⇒ 只有手触入口，从来没自动跑过（本轮不判，需要人看节奏）。
 *
 *  用法：node scripts/job_catalog_guard.mjs
 *  exit 1 = 不一致。无基线、无豁免：这三份清单本来就该同源。 */
import { readFileSync } from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const JOBS = "apps/worker/src/jobs";

function src(rel) {
  return readFileSync(path.join(ROOT, rel), "utf-8");
}

/** 从源码里抽名字：re 的第 1 捕获组即 job 名 */
function names(text, re) {
  const out = new Set();
  for (const m of text.matchAll(re)) out.add(m[1]);
  return out;
}

const catalog = names(src(`${JOBS}/catalog.rs`), /name:\s*"([a-z0-9_]+)"/g);
const manual = names(src(`${JOBS}/manual.rs`), /^\s{8}"([a-z0-9_]+)"\s*=>/gm);
const scheduled = names(src(`${JOBS}/run.rs`), /"job:([a-z0-9_]+)"/g);

const diff = (a, b) => [...a].filter((x) => !b.has(x)).sort();

const problems = [];
for (const j of diff(catalog, manual)) {
  problems.push(
    `面板可见但手触必失败：catalog 有 ${j}，run_named 无对应分支 ` +
      `（站长点「执行」只会得到「未知任务 ${j}」）`,
  );
}
for (const j of diff(manual, catalog)) {
  problems.push(
    `能手触却不在目录里：run_named 有 ${j}，catalog 未登记 ` +
      `（面板看不见，等于死代码入口）`,
  );
}
for (const j of diff(scheduled, catalog)) {
  problems.push(
    `定时在跑却不在目录里：run.rs 调度 ${j}，catalog 未登记 ` +
      `（运营看不见积压、也不能手动补跑）`,
  );
}

if (problems.length) {
  console.log(
    `任务目录三向不一致（catalog ${catalog.size} / run_named ${manual.size}` +
      ` / 调度 ${scheduled.size}）：`,
  );
  for (const p of problems) console.log(`  - ${p}`);
  process.exit(1);
}
console.log(
  `OK: 任务清单三向一致（catalog ${catalog.size} · run_named ${manual.size}` +
    ` · 调度 ${scheduled.size}）`,
);
