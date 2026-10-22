// 运行时字典里含 {magic}（站点货币名占位）的叶子导成平铺 JSON，
// 供 scripts/audit_magic_placeholder.py 判「读它的那句有没有补货币名」。
//
// 为什么要走运行时而不是正则扫 ts 文件：行级正则把 131 条读成 264 条
// （值换行的、隔壁条目的都混进来），手写展平器又只读出 5 条——
// 只有 `import { zhCN }` 拿到的对象才是页面真正在用的那本。
// 基数断言（>2000 叶、>50 条含占位）是防「导错对象静默产出空表」。
import { writeFileSync } from "node:fs";
import path from "node:path";
import { expect, it } from "vitest";
import { zhCN } from "../i18n/zh-CN";

function walk(node: unknown, prefix: string, out: Record<string, string>) {
  if (typeof node === "string") {
    out[prefix] = node;
    return;
  }
  if (node && typeof node === "object" && !Array.isArray(node)) {
    for (const [k, v] of Object.entries(node as Record<string, unknown>)) {
      walk(v, prefix ? `${prefix}.${k}` : k, out);
    }
  }
}

it("导出 {magic} 文案表并守住基数", () => {
  const flat: Record<string, string> = {};
  walk(zhCN, "", flat);
  const magic: Record<string, string> = {};
  for (const [k, v] of Object.entries(flat)) {
    if (v.includes("{magic}")) magic[k] = v;
  }
  expect(Object.keys(flat).length).toBeGreaterThan(2000);
  expect(Object.keys(magic).length).toBeGreaterThan(50);
  // 两本一起导：{magic} 那一族只是「占位符没喂值」的一个特例，
  // audit_placeholder_fill.py 要拿整本叶子去比每个 fmt(...) 的实参键集。
  writeFileSync(
    // 落到仓库根：`.gitignore` 里 `/_*.json` 是「跑完即弃」的调试产物位
    path.resolve(__dirname, "../../../_tmp_dict_leaves.json"),
    JSON.stringify({ total: Object.keys(flat).length, magic, flat }, null, 0),
    "utf-8",
  );
});
