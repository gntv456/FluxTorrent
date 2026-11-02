import { zhCN } from "@/i18n/zh-CN";

/** 网络层兜底文案（i18n 收口批）：api-client 不在 React 树内拿不到
 *  LocaleProvider 字典，这里按 locale cookie 取对应字典的网络段；
 *  取不到（SSR 无 cookie 等）回落 zh-CN 源字典。en/zh-TW 字典整本
 *  数千键，顶层静态 import 会把它拖进每个页面的 bundle——按需
 *  require（webpack 分包）只在此分支命中时加载。
 *
 *  2026-11-02 从 api-client.ts 拆出：那边要给「带分页头的 GET」腾行数
 *  （300 行门禁），而这张表与请求本身无关，单独成文件更贴「按域拆」的惯例。 */
const NET_DICTS: Record<string, () => { errors?: Record<string, string> }> = {
  en: () => require("@/i18n/en").en as { errors?: Record<string, string> },
  "zh-TW": () =>
    require("@/i18n/zh-TW").zhTW as { errors?: Record<string, string> },
};

export function netMsg(
  locale: string | undefined,
  key: "httpServerError" | "httpDownloadFail" | "httpPleaseLogin",
): string {
  const load = locale ? NET_DICTS[locale] : undefined;
  if (load) {
    const v = load().errors?.[key];
    if (v) return v;
  }
  return (zhCN.errors as unknown as Record<string, string>)?.[key] ?? "";
}
