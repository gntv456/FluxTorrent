/** 主题注册表门禁（W-42 后续：scheme 单一真源 + 主题名 id-keyed）。
 *  失败即 CI 红：
 *  1. 每个注册主题 id 的显示名键（themeNameKey）必须在 zh-CN / zh-TW / en
 *     三语 common 下齐全且非空——新增主题忘了补三语时在这里炸；
 *  2. 每个主题必须声明 scheme 与合法 chrome；
 *  3. light / dark 两族都要存在（auto 跟随系统才永远有解）；
 *  4. auto 解析只走 themeIdByScheme（scheme 字段），与 matchMedia 联动正确。 */
import { describe, expect, it, vi, afterEach } from "vitest";
import {
  THEMES,
  THEME_IDS,
  parseChoice,
  resolveTheme,
  themeIdByScheme,
  themeNameKey,
} from "@/lib/themes";
import { zhCN } from "@/i18n/zh-CN";
import { zhTW } from "@/i18n/zh-TW";
import { en } from "@/i18n/en";

const DICTS = { "zh-CN": zhCN, "zh-TW": zhTW, en } as const;

describe("主题注册表 ↔ i18n 三语名（id-keyed）", () => {
  it("每个注册主题 id 在三语 common 下都有非空显示名", () => {
    const missing: string[] = [];
    for (const id of THEME_IDS) {
      const key = themeNameKey(id);
      for (const [locale, dict] of Object.entries(DICTS)) {
        const v = (dict.common as Record<string, unknown>)[key];
        if (typeof v !== "string" || !v.trim()) {
          missing.push(`${locale}: common.${key}（主题 "${id}"）`);
        }
      }
    }
    expect(
      missing,
      `以下主题名缺失，请按 themeNameKey(id) 补三语文案：\n  ${missing.join("\n  ")}`,
    ).toEqual([]);
  });

  it("themeNameKey 推导稳定（brand / brand-night）", () => {
    expect(themeNameKey("brand")).toBe("themeBrand");
    expect(themeNameKey("brand-night")).toBe("themeBrandNight");
  });
});

describe("ThemeDef.scheme（auto 解析唯一真源）", () => {
  it("每个主题都声明 scheme 与 #rrggbb chrome", () => {
    for (const id of THEME_IDS) {
      expect(THEMES[id].scheme, id).toMatch(/^(light|dark)$/);
      expect(THEMES[id].chrome, id).toMatch(/^#[0-9a-fA-F]{6}$/);
    }
  });

  it("light / dark 两族齐全，auto 永远有解", () => {
    const schemes = new Set(THEME_IDS.map((id) => THEMES[id].scheme));
    expect(schemes.has("light")).toBe(true);
    expect(schemes.has("dark")).toBe(true);
    expect(THEME_IDS).toContain(themeIdByScheme("light"));
    expect(THEME_IDS).toContain(themeIdByScheme("dark"));
  });

  it("themeIdByScheme 命中注册表中的对应族主题", () => {
    expect(THEMES[themeIdByScheme("light")].scheme).toBe("light");
    expect(THEMES[themeIdByScheme("dark")].scheme).toBe("dark");
  });
});

describe("resolveTheme（跟随系统路径无回归）", () => {
  afterEach(() => vi.unstubAllGlobals());

  function stubMatchMedia(dark: boolean) {
    vi.stubGlobal("matchMedia", (q: string) => ({
      matches: dark && q === "(prefers-color-scheme: dark)",
      addEventListener() {},
      removeEventListener() {},
    }));
  }

  it("auto + 系统深色 → dark 族主题；auto + 系统浅色 → light 族主题", () => {
    stubMatchMedia(true);
    expect(THEMES[resolveTheme("auto")].scheme).toBe("dark");
    stubMatchMedia(false);
    expect(THEMES[resolveTheme("auto")].scheme).toBe("light");
  });

  it("手选具体主题原样通过；非法存储值归 auto 再解析", () => {
    stubMatchMedia(true);
    for (const id of THEME_IDS) expect(resolveTheme(id)).toBe(id);
    const parsed = parseChoice("not-a-theme");
    expect(parsed).toBe("auto");
    expect(THEMES[resolveTheme(parsed)].scheme).toBe("dark");
  });
});
