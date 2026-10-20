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

/** 收集所有在 settings_meta 里登记过的键（登记语句首列的字符串字面量）。
 *
 * 扫描范围必须锁在 `INSERT INTO settings_meta …;` 这一段里。此前的写法是
 * 「整份迁移只要提到 settings_meta 就通吃」，于是同一份迁移里给别的表
 * （section_kinds 之类）INSERT 的首列字面量也被当成设置键——
 * 0340 的 `season_label` 假红就是这么来的（门禁自己假红，比漏检更伤：
 * 人们从此不信这个门）。
 */
function metaKeys() {
  const keys = new Map(); // key -> 首次登记的迁移文件
  const sqls = readdirSync(MIGRATIONS)
    .filter((n) => n.endsWith(".sql"))
    .sort();
  const BLOCK = /INSERT\s+INTO\s+["']?settings_meta["']?\b[^;]*;/gi;
  for (const f of sqls) {
    const text = readFileSync(path.join(MIGRATIONS, f), "utf-8");
    // 两种登记形态都要收到，缺一种就是漏检：
    //   VALUES ('key', …)            —— 元组首列
    //   SELECT 'key', 'yesno', …     —— 配 WHERE NOT EXISTS 的补录写法
    // 键名都是第一列（与 0193 的清理判据一致）。
    const reTuple = /\(\s*'([a-zA-Z][a-zA-Z0-9_]*)'\s*[,)]/g;
    const reSelect = /\bSELECT\s+'([a-zA-Z][a-zA-Z0-9_]*)'\s*,/gi;
    let block;
    while ((block = BLOCK.exec(text))) {
      for (const re of [reTuple, reSelect]) {
        re.lastIndex = 0;
        let m;
        while ((m = re.exec(block[0]))) {
          if (!keys.has(m[1])) keys.set(m[1], f);
        }
      }
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
  // SQL 侧读者（2026-10-21 补）：迁移里的函数/视图体。缺这一段就会把
  // 「只有数据库函数读的键」判成假开关——见 sqlObjectBodies() 的说明。
  parts.push(sqlObjectBodies());
  return parts.join("\n");
}

/** 迁移里 SQL 对象（函数 / 视图）的**函数体**文本。
 *
 *  为什么必须单独取：本门禁的证据面一直只有 Rust/TS，于是 0136 之后
 *  做种火花的全部参数键（`seeding_base_hourly / seeding_cap_hourly /
 *  seeding_bonus_scale / seeding_donor_mult / seeding_curve_k /
 *  seeding_rarity_k / seeding_rarity_exp / seeding_vol_log_base_gb`）
 *  被登记成「无人读取的假开关」——读它们的是迁移 0136/0385/0394 里建的
 *  数据库函数 `seeding_params()`，worker 的做种收益 SQL 直接调它。
 *  基线是「清理清单」，照它删这些键会当场拆掉做种公式（P-460 的姊妹面：
 *  同一族里，旧键是真死、这八把是被误判为死）。
 *
 *  只收 dollar-quoted 区域（函数 / DO 块体）且**必须含 `FROM site_settings`**：
 *  注册语句是 `INSERT INTO settings_meta …`，不含这个读法，所以不会被算成
 *  引用——否则每个键都「有读者」，门禁直接失明（这就是它原来整份排除迁移
 *  文件的原因）。实测现网只有 9 段函数体读 site_settings，无一例视图读它。
 */
function sqlObjectBodies() {
  const bodies = [];
  // 配对的 dollar-quote（$$…$$ 与 $tag$…$tag$）。非贪婪，避免跨函数吞并。
  const QUOTED = /\$([A-Za-z_0-9]*)\$([\s\S]*?)\$\1\$/g;
  for (const f of readdirSync(MIGRATIONS)
    .filter((n) => n.endsWith(".sql"))
    .sort()) {
    const text = readFileSync(path.join(MIGRATIONS, f), "utf-8");
    let m;
    QUOTED.lastIndex = 0;
    while ((m = QUOTED.exec(text))) {
      if (m[2].includes("FROM site_settings")) bodies.push(m[2]);
    }
  }
  return bodies.join("\n");
}

/** 动态拼出来的键名（门禁的老盲区，2026-10-20 补上）。
 *
 *  自动审种的规则开关就是这么写的：
 *      for id in ["arule_title_garbled", ...] {
 *          let enabled = gb(&format!("{id}_enabled"), true);
 *          let action  = Action::parse(&g(&format!("{id}_action")));
 *      }
 *  —— 完整键名 arule_title_garbled_enabled 在源码里一次都不出现，
 *  静态子串匹配判它「无人读取」，于是 12 个真开关被判成假开关。
 *  （门禁自己的注释早就写了「动态拼出来的键名读不到」，只是没说怎么补。）
 *
 *  判据（两条件同时成立才算数，宁可放过不误伤）：
 *    1) 键名的基名（去掉后缀）在源码里出现；
 *    2) 源码里存在「占位符 + 同一后缀」的动态拼接。
 */
const DYN_SUFFIXES = [
  "_enabled",
  "_action",
  "_level",
  "_min_chars",
  "_threshold",
  "_pct",
  "_days",
  "_secs",
  "_limit",
  "_max",
  "_min",
  "_mode",
  "_note",
  "_hint",
];

function dynamicHit(corpus, key) {
  for (const suf of DYN_SUFFIXES) {
    if (!key.endsWith(suf)) continue;
    const base = key.slice(0, -suf.length);
    if (!base || !corpus.includes(base)) continue;
    const re = new RegExp(
      "\\{[a-zA-Z_][a-zA-Z0-9_]*\\}" + suf,
    );
    if (re.test(corpus)) return true;
  }
  return false;
}

const keys = metaKeys();
const corpus = sourceCorpus();
const dead = [];
for (const [k, mig] of keys) {
  if (corpus.includes(k)) continue;
  if (dynamicHit(corpus, k)) continue;
  dead.push({ key: k, migration: mig });
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
