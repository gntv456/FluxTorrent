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

/**
 * 站点语言 ⇄ 前端 locale 的唯一换算处。库里存 NexusPHP 风格码
 * （site_settings.default_language / users.site_language：en|chs|cht），
 * 前端 i18n 用 BCP47（zh-CN/zh-TW/en）。用户面板（usercp）与
 * 服务端默认语言回落（server getLocale）共用本函数，别再各写一份映射。
 * 未知值返回 null（调用方自行回落），不猜。
 */
export function siteLangToLocale(v: string | null | undefined): Locale | null {
  switch ((v ?? "").trim().toLowerCase()) {
    case "en":
      return "en";
    case "chs":
    case "zh-cn":
      return "zh-CN";
    case "cht":
    case "zh-tw":
      return "zh-TW";
    default:
      return null;
  }
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
