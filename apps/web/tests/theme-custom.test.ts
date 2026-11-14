/** 自选色派生双实现一致性门禁（2026-11-15）。
 *
 * lib/theme-custom.ts（TS 版，运行时切换用）与 lib/theme-custom-es5.ts
 * （ES5 版，no-flash 内联脚本用）的 hexToHsl/deriveCustomVars 必须
 * **同输入同输出**——首绘防闪与用户切换走的是两条独立实现，一旦漂移，
 * 刷新前后的主题会不一样。测试在 Node 里用 Function 构造器跑 ES5 版，
 * 与 TS 版逐键比对。 */
import { describe, expect, it } from "vitest";
import { deriveCustomVars, hexToHsl } from "@/lib/theme-custom";
import { CUSTOM_DERIVE_ES5 } from "@/lib/theme-custom-es5";

/** 在当前 Node 上下文构造 ES5 函数（源码只依赖 Math/parseInt/RegExp） */
function buildEs5Derive(hex: string): Record<string, string> | null {
  const fn = new Function(
    `${CUSTOM_DERIVE_ES5} return __cDerive(${JSON.stringify(hex)});`,
  );
  return fn() as Record<string, string> | null;
}

const SAMPLES = [
  "#3a8fe0", // 常规蓝
  "#d47628", // 暖橙（高饱和）
  "#2f6616", // 深绿
  "#8a68c0", // 紫
  "#b5544a", // 红
  "#808080", // 灰阶（走 56% 兜底分支）
  "#e8e8e8", // 近灰（<12% 兜底分支）
  "#123456", // 低明度
  "#fedcba", // 高明度
];

describe("hexToHsl：TS 版基础行为", () => {
  it("非法输入返回 null", () => {
    expect(hexToHsl("nope")).toBeNull();
    expect(hexToHsl("#12345")).toBeNull();
    expect(hexToHsl("")).toBeNull();
  });

  it("纯白/纯黑灰阶分支", () => {
    expect(hexToHsl("#ffffff")).toEqual([0, 0, 100]);
    expect(hexToHsl("#000000")).toEqual([0, 0, 0]);
  });
});

describe("deriveCustomVars：TS 版键集与钳制", () => {
  it("非法 hex 返回 null；合法 hex 返回完整键集", () => {
    expect(deriveCustomVars("zzz")).toBeNull();
    const vars = deriveCustomVars("#3a8fe0");
    expect(vars).not.toBeNull();
    expect(Object.keys(vars!).length).toBeGreaterThan(30);
    expect(vars!["--sky"]).toMatch(/^hsl\(/);
    expect(vars!["--grad-aurora"]).toMatch(/^linear-gradient/);
  });

  it("灰阶输入不产出灰板（S 落 56 兜底）", () => {
    const vars = deriveCustomVars("#808080")!;
    expect(vars["--sky"]).toBe("hsl(0, 56%, 38%)");
  });
});

describe("双实现同构：ES5(no-flash) ≡ TS(运行时)", () => {
  for (const hex of SAMPLES) {
    it(`hex=${hex} 两版逐键相等`, () => {
      const ts = deriveCustomVars(hex);
      const es5 = buildEs5Derive(hex);
      expect(es5).not.toBeNull();
      expect(ts).not.toBeNull();
      const tsKeys = Object.keys(ts!).sort();
      const es5Keys = Object.keys(es5!).sort();
      expect(es5Keys).toEqual(tsKeys);
      for (const k of tsKeys) {
        expect(es5![k], `key ${k}`).toBe(ts![k]);
      }
    });
  }

  it("非法 hex 两版同返 null", () => {
    const fn = new Function(
      `${CUSTOM_DERIVE_ES5} return __cDerive("nope");`,
    );
    expect(fn()).toBeNull();
    expect(deriveCustomVars("nope")).toBeNull();
  });
});
