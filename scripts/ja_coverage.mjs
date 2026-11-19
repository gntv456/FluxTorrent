import { readFileSync, existsSync } from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..", "apps", "web", "i18n");
const read = (f) => readFileSync(path.join(root, f), "utf8");
const segs = (src) =>
  [...src.matchAll(/^  ([A-Za-z][A-Za-z0-9_]*)\s*:\s*\{/gm)].map(
    (m) => m[1],
  );

const zh = new Set(segs(read("zh-CN.ts")));
const jaSrc =
  read("ja.ts") +
  Array.from({ length: 8 }, (_, i) => "ja-part-" + (i + 1) + ".ts")
    .filter((f) => existsSync(path.join(root, f)))
    .map(read)
    .join("\n");
const ja = new Set(segs(jaSrc));
const missing = [...zh].filter((k) => !ja.has(k));
const extra = [...ja].filter((k) => !zh.has(k));
console.log(
  "JA_COVERAGE",
  JSON.stringify({
    zhSegments: zh.size,
    jaSegments: ja.size,
    coverage: Math.round((ja.size / zh.size) * 1000) / 10,
    missing,
    extra,
  }),
);
if (missing.length) process.exitCode = 1;
