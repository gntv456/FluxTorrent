// 2026-11-11 瘦身批回归：客户端 chunk 不许再装整本语言字典。
//
// 背景：lib/net-msg.ts 曾「静态 import zhCN + 惰性 require en/zh-TW」，
// require 的「按需」对 bundler 是同步依赖——三本 8000 行字典（含骑在字典
// 里的 audit-labels）全量进了共享 chunk（线上 6538 那颗 745KB 的元凶）。
// 改内联常量表后用「源码层依赖断言」守复发：i18n/ 字典只允许三类 import——
// type-only（编译期擦除）、i18n/server.ts（服务端 getDict 唯一出口）、
// 测试与字典自身。lib/ 与 i18n/client 侧出现 value import 即红。
//
// 为什么不用构建产物 grep：vitest 不碰 .next（CI 也不跑它），产物级断言
// 得挂 next build 后的独立 job——留待需要时再加；源码层守卫已能拦住
// 「新工具模块顺手 import 字典」这个真实的复发路径。
import { readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { expect, it } from "vitest";

const WEB = path.resolve(__dirname, "..");

/** 递归收集目录下 .ts/.tsx（跳过 node_modules/.next/tests/i18n 字典目录） */
function collect(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    if (["node_modules", ".next", "tests", "coverage"].includes(name)) continue;
    const p = path.join(dir, name);
    if (statSync(p).isDirectory()) {
      // i18n/ 字典目录内部的相互引用（server.ts 的 DICTS、en.ts 骑
      // audit-labels）是合法的；这里只管「字典流向客户端代码」
      if (path.basename(p) === "i18n" && path.dirname(p) === WEB) continue;
      collect(p, out);
    } else if (/\.(ts|tsx)$/.test(name)) {
      out.push(p);
    }
  }
  return out;
}

it("lib/ 与 components/ 不出现对字典的 value import", () => {
  const offenders: string[] = [];
  for (const f of [
    ...collect(path.join(WEB, "lib")),
    ...collect(path.join(WEB, "components")),
    ...collect(path.join(WEB, "app")),
  ]) {
    const src = readFileSync(f, "utf8");
    // 命中：import { zhCN } —— 花括号里除 type 前缀成员外还有值成员，且
    // from 指向 i18n/zh-CN|zh-TW|en|ja（正则拆两行守行宽）
    const dictFrom = "[^\"']*i18n\\/(zh-CN|zh-TW|en|ja)[\"']";
    const re = new RegExp(
      `import\\s*\\{([^}]*)\\}\\s*from\\s*["']${dictFrom}`, "g",
    );
    for (const m of src.matchAll(re)) {
      const members = m[1]
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean);
      const valueMembers = members.filter((s) => !s.startsWith("type "));
      if (valueMembers.length > 0) {
        offenders.push(`${path.relative(WEB, f)}: ${m[0]}`);
      }
    }
  }
  expect(offenders, `字典 value import 复发:\n${offenders.join("\n")}`).toEqual([]);
});
