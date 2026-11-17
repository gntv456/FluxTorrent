#!/usr/bin/env node
/**
 * 山海经主题 CSS 出包脚本（2026-11-11 瘦身批）。
 *
 * globals.css 不再全量 @import 山海经的字体（184 条 @font-face）与主题令牌
 * （theme-shanhaijing.css，完全 [data-theme] 作用域化）——它们只服务手选的
 * 山海经两档主题。本脚本把两份源文件逐字拷贝到 public/theme-css/，运行时由
 * ThemeNoFlash（首绘前）与 theme-toggle（切换时）按需注入 <link>。
 *
 * 逐字拷贝是刻意约束：styles/ 下的源文件行宽基线（368/426 条超宽行）与
 * ui_design_guard 的扫描口径都以源文件为准，public/ 下的拷贝件不进门禁；
 * 想改山海经样式请改 styles/ 源文件后重跑本脚本，别手改拷贝件。
 *
 * 版本号用源文件内容的 sha256 前 8 位（内容变 → 文件名变 → <link> 缓存自动
 * 失效），注入侧（layout.tsx / theme-toggle.tsx）读 manifest.json 保持同步。
 */

import { createHash } from "node:crypto";
import {
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const webRoot = join(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "apps",
  "web",
);
const SRC = [
  ["app/styles/fonts-shanhaijing.css", "shanhaijing-fonts"],
  ["app/styles/theme-shanhaijing.css", "shanhaijing-theme"],
];
const OUT_DIR = join(webRoot, "public", "theme-css");

rmSync(OUT_DIR, { recursive: true, force: true });
mkdirSync(OUT_DIR, { recursive: true });

const manifest = { fonts: "", theme: "" };
for (const [srcRel, base] of SRC) {
  // hash 与拷贝件都走 LF 规范化字节：core.autocrlf=true 的 Windows 工作区
  // 源文件是 CRLF（git checkout 转的），若按原始字节 hash，同一份内容在
  // Windows/Linux 会产出不同文件名（2026-11-18 d9d900f0 vs f34fb4d3 实证），
  // 跨平台不可复现。仓库内 blob 恒为 LF（.gitattributes 锁定），拷贝件
  // 统一写 LF 与提交侧对齐。
  const content = readFileSync(join(webRoot, srcRel));
  const normalized = Buffer.from(
    content.toString("utf8").replace(/\r\n/g, "\n"),
    "utf8",
  );
  const tag = createHash("sha256").update(normalized).digest("hex").slice(0, 8);
  const name = `${base}.${tag}.css`;
  writeFileSync(join(OUT_DIR, name), normalized);
  manifest[base.includes("fonts") ? "fonts" : "theme"] = `/theme-css/${name}`;
  console.log(`[gen-theme-css] ${srcRel} -> public/theme-css/${name}`);
}
writeFileSync(
  join(OUT_DIR, "manifest.json"),
  JSON.stringify(manifest, null, 2),
);
console.log("[gen-theme-css] manifest.json written");
