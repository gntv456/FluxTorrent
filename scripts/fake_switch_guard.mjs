/**
 * 假开关门禁（站点运营审计 2026-10-07）。
 *
 * 现象：`settings_meta` 里登记的键会在 `/admin/settings` 渲染成可编辑控件，
 * 但**没有任何代码读取**它 —— 站长改完保存成功、界面回显正常，行为一字不变。
 * 这类缺陷此前已被清过两轮（0193 fake_switch_cleanup、Round C 的 L5），
 * 而它一定会复发：因为"加一个设置键"只要写迁移 + 元数据，
 * "让它真生效"要另写消费者，两步从来不是同一个提交。
 *
 * 判据（与 0193 同口径）：键名出现在 `settings_meta` 的 INSERT 里 = 界面上可写；
 * 若整个源码树（排除迁移与 i18n/基线文件）里再没有第二处引用 → 判为假开关。
 * 首次运行落基线（现存 289 条一并登记，只拦新增），此后 CI 只红不谎。
 *
 * **读数字时要知道的两个偏差**（都往"少报"方向，不会误伤别人）：
 *  1. 动态拼出来的键名读不到，例如 `module_<key>`（modules 网关）与
 *     `site_settings` 按 group 批读的地方——存量里 4 条 `module_*` 属此类；
 *  2. 只做子串匹配，某键若恰好是别处更长的字符串的一部分，会被算成"有引用"。
 * 所以 289 是"确定无人静态引用"的下界，逐条定性前不要当结案数字用。
 *
 * 用法：
 *   node scripts/fake_switch_guard.mjs            # 校验
 *   node scripts/fake_switch_guard.mjs --update   # 重算基线（改完真代码后再跑）
 */
import {
  existsSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const BASELINE = path.join(ROOT, "scripts", "fake_switch_baseline.json");
const MIGRATIONS = path.join(ROOT, "apps", "api", "migrations");
// 消费者扫描面必须含 tracker：probe_*/ratio_gate*/announce_pending_policy
// 的读取方在 apps/tracker/src（guard_refresh 从 site_settings 拉取后经
// Redis/静态量喂给探测循环与闸门）。漏扫 tracker 会把这些真消费者误判
// 成假开关（2026-10-08 实炸：8 键全红，其中 6 键消费方就在 tracker）。
const SCAN_DIRS = [
  "apps/api/src",
  "apps/worker/src",
  "apps/tracker/src",
  "apps/web",
  "packages",
];
const SKIP_DIR =
  /(^|[/\\])(node_modules|\.next|coverage|target|dist|\.git)([/\\]|$)/;
const EXT = new Set([".rs", ".ts", ".tsx", ".js", ".mjs", ".sql", ".json"]);

/** 收集所有在 settings_meta 里登记过的键（INSERT 语句的字符串字面量首列）。 */
function metaKeys() {
  const keys = new Map(); // key -> 首次登记的迁移文件
  const sqls = readdirSync(MIGRATIONS)
    .filter((n) => n.endsWith(".sql"))
    .sort();
  for (const f of sqls) {
    const text = readFileSync(path.join(MIGRATIONS, f), "utf-8");
    if (!/settings_meta/i.test(text)) continue;
    // VALUES ('key', …) / ('key','group',… ：只取每行第一个字面量，
    // 这与 0193 的清理判据一致（键名就是第一列）
    const re = /\(\s*'([a-zA-Z][a-zA-Z0-9_]*)'\s*,/g;
    let m;
    while ((m = re.exec(text))) {
      if (!keys.has(m[1])) keys.set(m[1], f);
    }
  }
  return keys;
}

/** 源码全文（不含迁移）拼成一个 haystack，逐键判引用。 */
function sourceCorpus() {
  const files = [];
  const walk = (dir) => {
    for (const e of readdirSync(dir)) {
      const p = path.join(dir, e);
      if (SKIP_DIR.test(p)) continue;
      const st = statSync(p);
      if (st.isDirectory()) walk(p);
      else if (EXT.has(path.extname(e))) files.push(p);
    }
  };
  for (const d of SCAN_DIRS) {
    const p = path.join(ROOT, d);
    if (existsSync(p)) walk(p);
  }
  const parts = [];
  const self = path.join(ROOT, "scripts", "fake_switch_guard.mjs");
  for (const f of files) {
    if (path.resolve(f) === self) continue;
    if (f.endsWith("fake_switch_baseline.json")) continue;
    parts.push(readFileSync(f, "utf-8"));
  }
  return parts.join("\n");
}

const keys = metaKeys();
const corpus = sourceCorpus();
const dead = [];
for (const [k, mig] of keys) {
  if (!corpus.includes(k)) dead.push({ key: k, migration: mig });
}

if (process.argv.includes("--update")) {
  const content =
    JSON.stringify(
      { generated: "manual", dead: dead.map((d) => d.key) },
      null,
      2,
    ) + "\n";
  writeFileSync(BASELINE, content);
  console.log(`基线已写入：登记 ${dead.length} 条现存假开关（只拦新增）`);
  process.exit(0);
}

if (!existsSync(BASELINE)) {
  console.error("缺 scripts/fake_switch_baseline.json —— 先跑 --update 落基线");
  process.exit(1);
}
const base = new Set(JSON.parse(readFileSync(BASELINE, "utf-8")).dead);
const fresh = dead.filter((d) => !base.has(d.key));
if (fresh.length) {
  console.error(
    `假开关门禁失败：${fresh.length} 个设置键在后台可写但全仓无人读取`,
  );
  for (const d of fresh.slice(0, 40)) {
    console.error(`  ${d.key}  (登记于 ${d.migration})`);
  }
  console.error("要么写消费者，要么在迁移里把它从 settings_meta 删掉。");
  process.exit(1);
}
console.log(
  `OK: settings_meta 登记 ${keys.size} 个键，存量基线 ${base.size} 条，无新增假开关`,
);
