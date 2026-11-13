/** 把 Prettier 展开的 3 行 <th> 压回单行（内容短，单行 <80 列）。 */
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
const ROOT = "D:/FluxTorrent";
const files = process.argv.slice(2);
let n = 0;
for (const rel of files) {
  const p = path.join(ROOT, rel);
  const src = readFileSync(p, "utf8");
  const out = src.replace(
    /<(th|td)(\s+[^>]*?)>\s*\n\s*(\{[^{}\n]*\})\s*\n\s*<\/\1>/g,
    (m, tag, attrs, body) => {
      const one = `<${tag}${attrs}>${body}</${tag}>`;
      if (one.length > 80) return m;
      n++;
      return one;
    },
  );
  if (out !== src) writeFileSync(p, out);
}
console.log("collapsed=" + n);
