/**
 * 用户偏好假开关门禁（资深用户视角实测审计 2026-10-10 批二）。
 *
 * 现象：`/me/settings` 暴露了 40 来个用户偏好，界面能改、保存回显都正常，
 * 但其中一批**没有任何消费者**——用户把「包括断种」「H&R 通知」关掉，
 * 行为一字不变。这与 0193 的站点设置假开关是同一类缺陷，只是换了主体：
 * 加设置项（struct + 表单）与写消费逻辑从来不是同一个提交。
 *
 * 判据：把 `UserSettings`（`me_settings_kv.rs`）的字段名当设置面清单，
 * 在 api/worker/web 三侧找引用；**只算代码不算注释**，并且排除
 * 「定义它」与「编辑它」的文件（me_settings_kv.*、usercp*、i18n、类型声明），
 * 因为只在设置页出现的名字恰好就是假开关的形状。
 *
 * 已知偏差（都往"少报"方向）：
 *  1. 动态拼出来的字段名读不到；
 *  2. 子串匹配，别处更长的同名串会被算成有引用；
 *  3. **消费点落在被 SKIP_PARTS 屏蔽的文件里时看不见**——`usercp.tsx` 既是
 *     设置界面又是语言偏好的消费点（读库里的 site_language 同步 locale
 *     cookie），跳过它就把消费者一起跳过了。此类走
 *     `CONSUMED_STATIC_INVISIBLE` 例外清单：每条**必须**带消费点 file:line，
 *     且新增前须人工确认消费路径真实存在（清单是例外，不是漂白名单）。
 *
 * 用法：
 *   node scripts/user_pref_guard.mjs            # 校验（只拦新增假开关）
 *   node scripts/user_pref_guard.mjs --update   # 重算基线（接完消费者后再跑）
 */
import {
  existsSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const DEF = join(ROOT, "apps/api/src/auth_http/me_settings_kv.rs");
const BASELINE = join(ROOT, "scripts/user_pref_baseline.json");
const SCAN_DIRS = ["apps/api/src", "apps/worker/src", "apps/web"];
const SKIP_PARTS = [
  "node_modules",
  ".next",
  "coverage",
  "/i18n/",
  "me_settings_kv.rs",
  "usercp",
];

/** 例外清单（见头部偏差 3）：偏好确有消费者，但消费点落在被 SKIP_PARTS
 *  屏蔽的文件里。每条必须写清消费点 file:line 与消费方式。 */
const CONSUMED_STATIC_INVISIBLE = {
  site_language:
    "apps/web/components/usercp.tsx:167,198 → 读 s.site_language 后经" +
    " siteLangToLocale + syncLocaleCookie + router.refresh()（改偏好即" +
    "切整站语言；另有反向一致：库里语言与 cookie 不符时以库为准）",
};
const EXT_OK = [".rs", ".ts", ".tsx"];
const update = process.argv.includes("--update");

/** 去掉注释：只认代码里的引用 */
function stripComments(text) {
  let out = "";
  let i = 0;
  const n = text.length;
  let quote = null;
  while (i < n) {
    const c = text[i];
    const d = text[i + 1];
    if (quote) {
      out += c;
      if (c === "\\" && quote !== "`") {
        out += text[i + 1] ?? "";
        i += 2;
        continue;
      }
      if (c === quote) quote = null;
      i += 1;
      continue;
    }
    if (c === '"' || c === "'" || c === "`") {
      quote = c;
      out += c;
      i += 1;
      continue;
    }
    if (c === "/" && d === "/") {
      while (i < n && text[i] !== "\n") i += 1;
      continue;
    }
    if (c === "/" && d === "*") {
      i += 2;
      while (i < n && !(text[i] === "*" && text[i + 1] === "/")) i += 1;
      i += 2;
      continue;
    }
    out += c;
    i += 1;
  }
  return out;
}

/** UserSettings 的字段名 = 站点承认的用户偏好清单 */
function prefNames() {
  const src = readFileSync(DEF, "utf8");
  const at = src.indexOf("struct UserSettings");
  if (at < 0) throw new Error("找不到 struct UserSettings");
  const body = src.slice(at, src.indexOf("\n}", at));
  return [...body.matchAll(/pub\(super\)\s+(\w+)\s*:/g)].map((m) => m[1]);
}

function walk(dir, acc) {
  if (!existsSync(dir)) return acc;
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    // 目录先剪枝：.next/standalone 里的符号链接在 Windows 上 stat 会 EPERM
    if (name === "node_modules" || name === ".next" || name === "coverage") {
      continue;
    }
    let st;
    try {
      st = statSync(p);
    } catch {
      continue; // 断链/权限问题的产物目录一律跳过，不影响判据
    }
    if (st.isDirectory()) walk(p, acc);
    else if (EXT_OK.some((e) => name.endsWith(e))) acc.push(p);
  }
  return acc;
}

const relPath = (p) => relative(ROOT, p).replace(/\\/g, "/");
const files = SCAN_DIRS.flatMap((d) => walk(join(ROOT, d), []))
  .filter((p) => !SKIP_PARTS.some((s) => relPath(p).includes(s)))
  .map((p) => [relPath(p), stripComments(readFileSync(p, "utf8"))]);

const names = prefNames();
const unwired = names.filter(
  (nm) =>
    !CONSUMED_STATIC_INVISIBLE[nm] &&
    !files.some(([, text]) => text.includes(nm)),
);
const known = existsSync(BASELINE)
  ? JSON.parse(readFileSync(BASELINE, "utf8")).unwired || []
  : [];

if (update) {
  writeFileSync(
    BASELINE,
    JSON.stringify({ unwired: unwired.slice().sort() }, null, 2) + "\n",
    "utf8",
  );
  console.log(`WROTE user_pref_baseline.json: unwired=${unwired.length}`);
  process.exit(0);
}

const fresh = unwired.filter((nm) => !known.includes(nm));
console.log(
  `用户偏好 ${names.length} 项；无消费者 ${unwired.length} 项` +
    `（基线 ${known.length}）`,
);
if (fresh.length) {
  console.log("FAIL 新增假开关（界面可改、代码零消费）：");
  for (const nm of fresh) console.log("  " + nm);
  process.exit(1);
}
console.log("OK: 没有新增无人消费的偏好项");
