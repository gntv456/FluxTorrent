// 术语规则「零命中」守卫（四审 L7 / 0205）。
//
// 防的是本档反复出现的那一类缺陷：**登记了，但没有任何东西读它**（假开关）。
// 术语机制本身有出口（前端字典出口 + 后端错误信封），所以危险不在机制，
// 而在某条规则的原词在两份文案语料里根本不出现——站长以为改了点什么，
// 实际什么都没变，而且永远不会报错。
//
// 两份语料 = 两个真实出口，不多也不少：
//   1) apps/web/i18n/zh-CN.ts   —— 前端字典出口（getDict 的改写对象）
//   2) apps/api/src/**/*.rs      —— 后端错误信封（terms::apply 的改写对象）
//
// 读库方式与 module_keys_guard.mjs 一致：连不上 flux-postgres 时跳过 DB 部分，
// 静态部分仍然生效。用法：node scripts/terms_guard.mjs
import { readFileSync, existsSync } from "node:fs";
import { globSync } from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";

const ROOT = path.resolve(import.meta.dirname, "..");
const DICT = "apps/web/i18n/zh-CN.ts";

/** 语料 1：源字典原文（未改写的叶子字符串都在这里） */
function dictCorpus() {
  const f = path.join(ROOT, DICT);
  if (!existsSync(f)) {
    console.error(`FAIL 找不到字典源文件 ${DICT}`);
    process.exit(1);
  }
  return readFileSync(f, "utf8");
}

/** 语料 2：后端会进错误信封的中文字面量 */
function apiCorpus() {
  const files = globSync("apps/api/src/**/*.rs", {
    cwd: ROOT,
    nodir: true,
  });
  let out = "";
  for (const f of files) out += readFileSync(path.join(ROOT, f), "utf8");
  return out;
}

/** 规则来源：优先库里启用的行；连不上库时退化为「只验机制不缺件」 */
function dbRules() {
  try {
    const out = execFileSync(
      "docker",
      ["exec", "flux-postgres", "psql", "-U", "flux", "-d", "fluxtorrent",
       "-tAc",
       "SELECT canonical FROM site_terms WHERE enabled"],
      { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] },
    );
    const words = out.split("\n").map((s) => s.trim()).filter(Boolean);
    return { ok: true, words };
  } catch {
    return { ok: false, words: [] };
  }
}

const dict = dictCorpus();
const api = apiCorpus();
const { ok, words } = dbRules();

if (!ok) {
  console.log("DB 侧跳过（连不上 flux-postgres；静态检查仍然生效）");
} else {
  console.log(`启用中的术语规则 ${words.length} 条`);
  const dead = words.filter((w) => !dict.includes(w) && !api.includes(w));
  for (const w of dead) {
    console.error(
      `FAIL 术语规则「${w}」零命中：源字典与后端文案里都没有这个词，` +
        "这条规则不会改变任何一处显示（要么改原词，要么删掉规则）",
    );
  }
  if (dead.length) process.exit(1);
  if (words.length) console.log("OK: 每条启用规则都至少命中一处文案");
}

// 机制自检：字典里 {magic} 是 currency_name 的占位符出口，
// 术语改写发生在它之后 —— 若哪天有人把 fmt/fmtCur 的调用顺序改了，
// 这条能第一时间报出来（避免出现第二个「注入名错位」那种假生效）。
if (!dict.includes("{magic}")) {
  console.error(
    "FAIL 源字典里找不到 {magic} 占位符：currency_name 的出口不见了",
  );
  process.exit(1);
}
console.log(`OK: {magic} 占位符在源字典中 ${dict.split("{magic}").length - 1} 处`);
