// glob 兼容垫片：优先用 node:fs 的 globSync（Node 22+），回落到 fast-glob
// （pnpm 依赖树里常驻，next/tailwind 都靠它跑）——CI 的 Node 20 没有 globSync。
// 语义对齐调用点所需的子集：{ cwd, nodir } + 返回相对路径数组。
//
// 用法（.mjs 脚本）：
//   import { globCompat } from "./lib/glob_compat.mjs";
//   globSync(pat, { cwd }) → globCompat(pat, { cwd })
import { createRequire } from "node:module";
import { existsSync, readdirSync, statSync } from "node:fs";
import path from "node:path";

const require = createRequire(import.meta.url);
let fg = null;

export function globCompat(pattern, opts = {}) {
  const g = loadGlob();
  return g(pattern, opts);
}

function loadGlob() {
  if (fg) return fg;
  // ① node:fs globSync（Node 22+）
  try {
    const m = require("node:fs");
    if (typeof m.globSync === "function") {
      fg = m.globSync;
      return fg;
    }
  } catch {
    /* 老版本 node:fs 无此导出，走 ② */
  }
  // ② 依赖树里的 fast-glob：从根 node_modules 及 pnpm 虚拟店解析
  const here = path.resolve(import.meta.dirname, "..", "..");
  const candidates = [
    path.join(here, "node_modules", "fast-glob"),
    ...readdirSync(path.join(here, "node_modules", ".pnpm"))
      .filter((d) => /^fast-glob@/.test(d))
      .map((d) =>
        path.join(
          here,
          "node_modules",
          ".pnpm",
          d,
          "node_modules",
          "fast-glob",
        ),
      ),
  ];
  for (const c of candidates) {
    if (!existsSync(c)) continue;
    try {
      const mod = require(c);
      const sync = (pat, o = {}) =>
        mod.sync(pat, {
          cwd: o.cwd,
          onlyFiles: o.nodir !== false,
          dot: true,
        });
      fg = sync;
      return fg;
    } catch {
      /* 试下一个 */
    }
  }
  throw new Error(
    "glob 不可用：需要 Node 22+（node:fs globSync）或依赖树里有 fast-glob",
  );
}
