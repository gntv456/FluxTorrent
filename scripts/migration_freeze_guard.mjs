#!/usr/bin/env node
// 迁移冻结守卫（2026-11-20 百灵站宕机复盘产物）：
// 已应用过的迁移文件（版本号 <= 基线最高版本）一律禁止改动——sqlx 启动时
// 逐行校验 sha384，不符即拒绝启动（"migration N was previously applied but
// has been modified"）。f06c024a 改了已应用的 0499，dev.38 webhook 自动发版
// 时百灵库 flux-api 崩溃循环六小时，全站 API 500。
//
// 规则：
//   1. 已应用版本（<= 冻结线）的文件：内容或文件名都不许变（改名=换版本号，
//      同样触发 sqlx 校验失败）。
//   2. 只允许新增更大版本号的文件（迁移只增不改）。
//   3. 需要修正已应用迁移的数据效果 → 新增幂等迁移（见 0509 善后先例）。
//
// 基线 migration_freeze_baseline.json：
// {"frozen_version": N, "sha384": {file: hex}}
// 首次生成/合法新增版本后重算：node scripts/migration_freeze_guard.mjs --update
// （--update 只追加新版本并推进冻结线，绝不改写已冻结条目）。

import { readFileSync, writeFileSync, readdirSync } from "node:fs";
import { createHash } from "node:crypto";
import { join } from "node:path";

const MIG_DIR = "apps/api/migrations";
const BASELINE = "scripts/migration_freeze_baseline.json";

const sha384 = (p) =>
  createHash("sha384").update(readFileSync(p)).digest("hex");

// 文件名 → 版本号（NNNN_slug.sql）
const versionOf = (f) => parseInt(f.split("_")[0], 10);

const UPDATE = process.argv.includes("--update");

let baseline;
try {
  baseline = JSON.parse(readFileSync(BASELINE, "utf8"));
} catch {
  if (!UPDATE) {
    console.error(
      `[migration_freeze] 基线缺失：${BASELINE}。首跑请用 --update 生成
      ` +
        `（生成前先确认工作树里迁移目录是「已发布状态」，没有在制改动）。`,
    );
    process.exit(1);
  }
  baseline = { frozen_version: 0, sha384: {} };
}

const files = readdirSync(MIG_DIR)
  .filter((f) => f.endsWith(".sql"))
  .sort();

const problems = [];

for (const f of files) {
  const v = versionOf(f);
  const frozenSha = baseline.sha384[f];
  const currentSha = sha384(join(MIG_DIR, f));

  if (frozenSha !== undefined) {
    // 已冻结：文件名（隐含版本号）与内容都不得变
    if (frozenSha !== currentSha) {
      problems.push(
        `${f}（已应用迁移 v${v} ≤ 冻结线 ${baseline.frozen_version}）内容被改动：` +
          `基线 ${frozenSha.slice(0, 16)}… → 现值 ${currentSha.slice(0, 16)}…。` +
          `改已应用迁移会让 sqlx 拒绝启动（2026-11-20 百灵站宕机根因）。` +
          `数据修正请新增幂等迁移；本文件必须恢复原样。`,
      );
    }
  } else if (v <= baseline.frozen_version) {
    // 冻结线以下的陌生文件名：改名/挪号同样炸校验
    problems.push(
      `${f}（v${v} ≤ 冻结线 ${baseline.frozen_version}）是基线外的陌生文件名——` +
        `疑似对已应用迁移改了名/挪了号。恢复原名或走新增版本。`,
    );
  } else if (!process.argv.includes("--update")) {
    // 冻结线以上的新文件：非 --update 模式下视为「未登记」，提示而非报红——
    // CI 里对新增文件用严格模式（见下），本地宽松。
  }
}

// 严格模式（CI）：冻结线以上出现任何新文件而未带 --update 重算基线 → 红。
// 这样新迁移必须显式 --update 登记（推冻结线），登记动作本身留下审计痕迹。
if (!process.argv.includes("--update")) {
  const unregistered = files.filter(
    (f) =>
      versionOf(f) > baseline.frozen_version &&
      baseline.sha384[f] === undefined,
  );
  if (unregistered.length > 0) {
    problems.push(
      `发现未登记的新迁移：${unregistered.join(", ")}。` +
        `确认无误后运行 node scripts/migration_freeze_guard.mjs --update 登记并推进冻结线。`,
    );
  }
}

if (problems.length > 0) {
  console.error(`[migration_freeze] ${problems.length} 处违规：`);
  for (const p of problems) console.error(`  ✗ ${p}`);
  process.exit(1);
}

if (UPDATE) {
  const prev = baseline.frozen_version;
  baseline.frozen_version = Math.max(
    prev,
    ...files.map((f) => versionOf(f)),
  );
  baseline.sha384 = Object.fromEntries(
    files.map((f) => [f, sha384(join(MIG_DIR, f))]),
  );
  // 稳定排序输出，避免键顺序抖动造成无谓 diff
  writeFileSync(BASELINE, JSON.stringify(baseline, null, 2) + "\n");
  console.log(
    `[migration_freeze] 基线已更新：冻结线 ${prev} → ${baseline.frozen_version}，` +
      `共 ${files.length} 个迁移文件。`,
  );
} else {
  console.log(
    `[migration_freeze] 通过：${files.length} 个迁移文件，` +
      `冻结线 v${baseline.frozen_version}，全部与基线一致。`,
  );
}
