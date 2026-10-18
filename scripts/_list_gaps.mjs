import fs from "node:fs";
import path from "node:path";
const walk = (d, acc = []) => {
  for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    if (e.name === ".next" || e.name === "node_modules") continue;
    const p = d + "/" + e.name;
    if (e.isDirectory()) walk(p, acc);
    else if (e.name.endsWith(".tsx")) acc.push(p.split("\\").join("/"));
  }
  return acc;
};
const tsx = walk("apps/web/app");
const read = (f) => fs.readFileSync(f, "utf8");
const pages = tsx.filter((f) => f.endsWith("/page.tsx"));
const DATA_HINTS = /await api\.|api\.(get|post|put|delete)|searchParams|force-dynamic|<Suspense|StatePanel|Skeleton/;
const gaps = pages.filter((f) => {
  if (!DATA_HINTS.test(read(f))) return false;
  const dir = path.posix.dirname(f);
  const hasLoading = tsx.some((g) => path.posix.dirname(g) === dir && g.endsWith("loading.tsx"));
  return !(hasLoading || /Skeleton|StatePanel/.test(read(f)));
});
console.log(gaps.join("\n"));
console.error("total gaps:", gaps.length);
