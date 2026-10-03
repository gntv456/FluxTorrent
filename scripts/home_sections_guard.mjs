// 首页板块单源防漂移（四审 L6）：
// 后端清单（apps/api/src/http/home_layout.rs 的 HOME_SECTIONS）
// 必须与前端渲染分支（home-sections.tsx renderSection 的 case 集）一一对应。
// 漂移的历史形态：`latest` 早在 0089 就进了白名单，但前端 switch 一直没有分支
// ⇒ 后台存得进、首页不认，排序与占宽对它完全无效，且没有任何地方报错。
import fs from "fs";
import path from "node:path";

// 相对脚本位置解析（CI 在 apps/web 下调用本脚本，见 module_keys_guard 同款说明）
const ROOT = path.resolve(import.meta.dirname, "..");
const RS = path.join(ROOT, "apps/api/src/http/home_layout.rs");
const TSX = path.join(ROOT, "apps/web/components/home-sections.tsx");

const rs = fs.readFileSync(RS, "utf8");
const block = rs.split("pub const HOME_SECTIONS")[1] ?? "";
const backend = [...block.matchAll(/key:\s*"([a-z_]+)"/g)].map((m) => m[1]);
// 截到清单数组结束，避免吃到文件里其它 `key: "..."` 字面量
const backendSet = new Set(backend.slice(0, 32));

const tsx = fs.readFileSync(TSX, "utf8");
const body = tsx.split("function renderSection")[1] ?? "";
const front = [...body.matchAll(/case "([a-z_]+)":/g)].map((m) => m[1]);
const frontSet = new Set(front);

const onlyBackend = [...backendSet].filter((k) => !frontSet.has(k));
const onlyFront = [...frontSet].filter((k) => !backendSet.has(k));

console.log(
  `后端清单 ${backendSet.size} 键 / 前端渲染分支 ${frontSet.size} 键`,
);
if (onlyBackend.length)
  console.log("  后端有、前端不渲染（存进去首页不认）: " + onlyBackend.join(", "));
if (onlyFront.length)
  console.log("  前端能渲染、后端不认（写不进去的死分支）: " + onlyFront.join(", "));
if (onlyBackend.length || onlyFront.length) {
  console.error("FAIL: 首页板块键集漂移");
  process.exit(1);
}
console.log("OK: 首页板块键集一致");
