/** 字典懒 chunk 清单读取（postbuild 由 scripts/gen-dict-chunks.mjs 生成
 *  public/i18n/dict-chunks.json，gitignore 不入库）。进程内缓存一次；
 *  文件缺失（dev / 旧构建产物）返回 null——layout 的 preload 静默跳过，
 *  行为退化为「字典在 Provider 挂起时才发起 import()」的现状。 */

let cache: Record<string, string> | null | undefined;

export function readDictChunk(locale: string): string | null {
  if (cache === undefined) {
    try {
      // process.cwd() 在 standalone server 下是 apps/web
      // node:fs 的 require 形态（Next 对 app/ 下 node: 导入友好）
      const fs = require("node:fs") as typeof import("node:fs");
      cache = JSON.parse(
        fs.readFileSync("public/i18n/dict-chunks.json", "utf8"),
      ) as Record<string, string>;
    } catch {
      cache = null;
    }
  }
  return cache?.[locale] ?? null;
}
