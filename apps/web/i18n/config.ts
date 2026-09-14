/** i18n 基础配置（三语：zh-CN 默认 / zh-TW / en；Cookie 切换，无 URL 前缀） */

export const LOCALES = ["zh-CN", "zh-TW", "en"] as const;
export type Locale = (typeof LOCALES)[number];
export const DEFAULT_LOCALE: Locale = "zh-CN";
export const LOCALE_COOKIE = "flux.locale";

export function isLocale(v: string | undefined | null): v is Locale {
  return !!v && (LOCALES as readonly string[]).includes(v);
}

/** 日期格式化 locale（toLocaleDateString/String/Time 用） */
export function dateLocale(locale: Locale): string {
  return locale === "zh-TW" ? "zh-TW" : locale === "en" ? "en-US" : "zh-CN";
}

/** 简易模板插值：fmt("共 {n} 个种子", { n: 3 }) → "共 3 个种子" */
export function fmt(
  tpl: string,
  vars: Record<string, string | number>,
): string {
  return tpl.replace(/\{(\w+)\}/g, (_, k: string) =>
    vars[k] === undefined ? "" : String(vars[k]),
  );
}

/**
 * 站点货币名插值：字典文案中的 {magic} 占位符替换为站点货币名
 * （默认「魔力」，站长后台 site_settings.currency_name 可改任意名）。
 * 例：fmtCur("签到成功！+{reward} {magic}", { reward: 10 }, "魔力")
 */
export function fmtCur(
  tpl: string,
  vars: Record<string, string | number>,
  currency: string,
): string {
  return fmt(tpl, { ...vars, magic: currency });
}
