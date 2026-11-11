#!/usr/bin/env node
/**
 * 字典懒 chunk 清单生成（2026-11-11 提速批 #5）。
 *
 * 背景：i18n 字典已拆每语言一颗懒 chunk（i18n/dict-impl.ts），但发现时机
 * 要等 eager JS 执行到 LocaleProvider 挂起才发起 import()——冷访客在劣化
 * 链路上串行多一跳 RTT。本脚本在 next build 后扫描产物，把「语言 → 字典
 * chunk URL」写入 public/i18n/dict-chunks.json；layout.tsx（SSR）读该文件
 * 在 <head> 注入 <link rel="preload" as="script">，浏览器在解析 HTML 时
 * 即并行拉取，与 eager JS 下载重叠。
 *
 * 指纹判据（与 tests/i18n-no-dict-in-client.test.ts 同口径）：
 *   zh-CN chunk 含「管理员清除两步验证」（audit-labels zh-CN 段独有）
 *   zh-TW chunk 含「管理員清除兩步驗證」
 *   en   chunk 含 "FluxTorrent" titleSuffix 且含 "Server error"
 *   ja 复用 zh-CN chunk + 自己的小 chunk（合并逻辑在 dict-impl，
 *   preload zh-CN 即可覆盖 ja 的大头）
 *
 * 注意：public/i18n/dict-chunks.json 是构建产物，gitignore 不入库；
 * 服务端 layout 读不到（文件缺失）时静默跳过 preload，行为退化为现状。
 */

import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const webRoot = join(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "apps",
  "web",
);
const chunksDir = join(webRoot, ".next", "static", "chunks");
const outFile = join(webRoot, "public", "i18n", "dict-chunks.json");

if (!existsSync(chunksDir)) {
  console.error("[gen-dict-chunks] .next/static/chunks 不存在——先 next build");
  process.exit(1);
}
const { readdirSync } = await import("node:fs");

// 主体判据（互斥字形）：zhCN=含简体「种子」不含繁体「種子」；zhTW 反之；
// en=英文网络文案。audit-labels 被打简繁共享 chunk（6624 类），单独识别，
// zh-CN/zh-TW 的 preload 都带上它；en 字典自带 audit 段（en.ts 内嵌）。
const found = { "zh-CN": "", "zh-TW": "", en: "" };
let auditShared = "";
for (const f of readdirSync(chunksDir)) {
  if (!f.endsWith(".js")) continue;
  const body = readFileSync(join(chunksDir, f), "utf8");
  const hasCN = body.includes("下载"); // 下载
  const hasTW = body.includes("下載"); // 下載
  // audit 共享块判据：简繁同体（audit-labels 被打简繁共享 chunk），
  // 且带 audit 独有词「两步验证」——LocaleProvider 所在小 chunk 也是
  // 简繁混排（netMsg 常量表），用它区分开。
  if (!auditShared && hasCN && hasTW && body.includes("两步验证")) {
    auditShared = `/_next/static/chunks/${f}`;
    continue;
  }
  if (!found["zh-CN"] && hasCN && !hasTW)
    found["zh-CN"] = `/_next/static/chunks/${f}`;
  else if (!found["zh-TW"] && hasTW && !hasCN)
    found["zh-TW"] = `/_next/static/chunks/${f}`;
  else if (
    !found.en &&
    body.includes("Server error (HTTP {status})") &&
    body.includes("Please sign in first") &&
    !hasCN
  )
    found.en = `/_next/static/chunks/${f}`;
}
// audit 段的树摇落位不定（zh-CN 常被内联进主体，zh-TW 拆共享块）：
// 只给「主体不含 audit 独有词」的语言补共享块，避免重复 preload。
if (auditShared) {
  const needAudit = (url) => {
    const f2 = join(webRoot, ".next", "static", "chunks", url.split("/").pop());
    const body = readFileSync(f2, "utf8");
    return !body.includes("两步验证") && !body.includes("兩步驗證");
  };
  for (const lang of ["zh-CN", "zh-TW"]) {
    if (found[lang] && needAudit(found[lang])) found[lang] += ` ${auditShared}`;
  }
}
const missing = Object.entries(found).filter(([, v]) => !v);
if (missing.length) {
  console.error("[gen-dict-chunks] 未找全字典 chunk:", missing);
  process.exit(1);
}
mkdirSync(dirname(outFile), { recursive: true });
writeFileSync(outFile, JSON.stringify(found, null, 2));
console.log("[gen-dict-chunks] public/i18n/dict-chunks.json 写入:");
for (const [k, v] of Object.entries(found))
  console.log(`  ${k}: ${v.split("/").pop()}`);
