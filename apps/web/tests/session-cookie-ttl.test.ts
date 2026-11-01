import { describe, it, expect } from "vitest";
import { readFileSync, readdirSync, statSync } from "fs";
import { join } from "path";

/**
 * 会话标记（flux.session）必须跟着票一起长——全仓普查（2026-11-02）。
 *
 * middleware 判登录态只看 `flux.session` 这枚可读标记，不看 HttpOnly 的
 * flux_token。所以**每一个**把人流进已登录态的出口都要把票的剩余时长写进
 * 标记；漏一处的症状是「票还活着、人被判成游客」，而且只在 12 小时之后
 * 出现，测试当天看不见。
 *
 * 2026-11-01 只给密码登录口接了 `tokenSecondsLeft`（79c993a8），passkey /
 * 注册后自动登录 / 装机向导两处都还停在默认 12h。本判据扫全仓而不是列
 * 文件名清单：以后新增登录口也自动被这条尺量到。
 *
 * 负控：把任何一处调用改成不带第二参，本用例必须红（见文件末尾的自检）。
 */

const ROOT = join(__dirname, "..");

function walk(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    if (["node_modules", ".next", "tests", "coverage"].includes(name)) continue;
    const p = join(dir, name);
    const st = statSync(p);
    if (st.isDirectory()) walk(p, out);
    else if (/\.(ts|tsx)$/.test(name) && !/\.test\./.test(name)) {
      out.push(p);
    }
  }
  return out;
}

/** 找 `setSessionCookie(true` 的调用，返回 [文件, 是否带第二个实参] */
function sessionCookieTrueCalls(): [string, boolean][] {
  const hits: [string, boolean][] = [];
  for (const file of walk(ROOT)) {
    const src = readFileSync(file, "utf8");
    let i = 0;
    while ((i = src.indexOf("setSessionCookie(true", i)) !== -1) {
      // 从命中的括号起做一层配对，数到第一个逗号 = 有第二个实参
      const open = src.indexOf("(", i);
      let depth = 0;
      let commaAt = -1;
      for (let j = open; j < src.length; j++) {
        const ch = src[j];
        if (ch === "(") depth++;
        else if (ch === ")") {
          depth--;
          if (depth === 0) break;
        } else if (ch === "," && depth === 1) {
          commaAt = j;
          break;
        }
      }
      hits.push([file.slice(ROOT.length + 1), commaAt !== -1]);
      i = open + 1;
    }
  }
  return hits;
}

describe("flux.session 标记跟票长（全仓登录出口普查）", () => {
  const calls = sessionCookieTrueCalls();

  it("普查确实抓到了登录出口（不是空扫描假绿）", () => {
    expect(calls.length).toBeGreaterThanOrEqual(6);
  });

  it("每一处 setSessionCookie(true) 都带了会话时长", () => {
    const missing = calls.filter(([, hasTtl]) => !hasTtl).map(([f]) => f);
    expect(missing).toEqual([]);
  });
});
