/** 网络层兜底文案（i18n 收口批）：api-client / blob-error 不在 React 树内
 *  拿不到 LocaleProvider 字典，这里按 locale cookie 出对应文案。
 *
 *  ⚠️ 2026-11-11 瘦身批改写：此前「静态 import zhCN + 惰性 require en/zh-TW」
 *  被打包器整本拖进共享 chunk（6538/6974 那颗 745KB 的元凶——require 的
 *  「按需」对 bundler 是同步依赖，三本 8000 行字典全量进包）。这张表实际
 *  只用 3 个 key × 3 语 ≈ 300 字节，改为**内联常量**：字典零依赖、零 fetch、
 *  同步签名不变。ja 走 zh-CN 回落（源字典 errors 段本就没有 ja 覆盖）。
 *  文案改动须与 i18n/{zh-CN,zh-TW,en}.ts 的 errors 段同步（键同名）。
 *
 *  2026-11-02 从 api-client.ts 拆出：那边要给「带分页头的 GET」腾行数
 *  （300 行门禁），而这张表与请求本身无关，单独成文件更贴「按域拆」的惯例。 */

const NET_MSGS: Record<string, Record<NetMsgKey, string>> = {
  "zh-CN": {
    httpServerError: "服务异常（HTTP {status}）",
    httpDownloadFail: "下载失败（HTTP {status}）",
    httpPleaseLogin: "请先登录",
  },
  "zh-TW": {
    httpServerError: "服務異常（HTTP {status}）",
    httpDownloadFail: "下載失敗（HTTP {status}）",
    httpPleaseLogin: "請先登入",
  },
  en: {
    httpServerError: "Server error (HTTP {status})",
    httpDownloadFail: "Download failed (HTTP {status})",
    httpPleaseLogin: "Please sign in first",
  },
};

type NetMsgKey = "httpServerError" | "httpDownloadFail" | "httpPleaseLogin";

/** zh-TW/en 之外的 locale（ja/未知值）回落 zh-CN——与源字典回落口径一致。 */
export function netMsg(locale: string | undefined, key: NetMsgKey): string {
  const table = NET_MSGS[locale ?? ""] ?? NET_MSGS["zh-CN"];
  return table[key] ?? NET_MSGS["zh-CN"][key];
}
