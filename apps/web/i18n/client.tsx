"use client";

/**
 * 客户端 i18n：useI18n() 消费字典与货币名。dict 内文案中的「火花」占位符
 * {spark}（源字典写作 {magic}）会按站点货币名动态替换（默认「魔力」，站长
 * 可在后台 site_settings.currency_name 改任意名），所以 useI18n() 额外暴露
 * fmt() 之外的 currency 名。
 *
 * 2026-11-11 第二轮瘦身：layout 不再把整本字典作为 prop 传入（曾让 React
 * 把它序列化进每个页面的 flight payload，174KB/页且 HTML 不可缓存）。改为
 * 唯一供给侧 i18n/dict-impl.ts（每语言一颗懒加载 chunk，双端共用）：
 *  - SSR：模块初始化已预热全部四语，同步缓存命中，服务端 HTML 完整零挂起；
 *  - 浏览器首帧：resolveDict 返回 null → 抛 loadDict 的 promise，由 layout
 *    的 Suspense 接住。cookie 语言在模块加载时已预热（chunk 与其余 JS 并行
 *    下载），promise 抛出前已落定，React 推迟该子树水合至字典就绪——文案
 *    不闪、fallback 不显示。
 *  - `dict` prop 保留为逃生门：71 个测试与应急回滚（layout 恢复传 dict 即
 *    回到「字典进 payload」的旧行为）都走这条直供通道。
 *
 * apiErrorMessage 已搬去 ./api-error-message.ts（isomorphic），此处
 * re-export 保住既有 191 处 `from "@/i18n/client"` 的 import 语句不动。
 */

import { createContext, useContext } from "react";
import type { Dict } from "./zh-CN";
import type { Locale } from "./config";
import type { TermRule } from "./apply-terms";
import { loadDict, resolveDict } from "@/i18n/dict-impl";

export { apiErrorMessage } from "./api-error-message";

const I18nContext = createContext<{
  dict: Dict;
  locale: Locale;
  currency: string;
} | null>(null);

export function LocaleProvider({
  dict,
  locale,
  currency = "魔力",
  terms,
  children,
}: {
  /** 逃生门：显式字典直供（测试 / 应急回滚），完全绕过懒加载供给链 */
  dict?: Dict;
  locale: Locale;
  currency?: string;
  /** 服务端 getAllTerms() 直传；客户端 dict-impl 用同一 applyTerms 改写，
   *  与 RSC 出口（getDict）同函数同输入 → 水合前后逐字一致 */
  terms?: TermRule[];
  children: React.ReactNode;
}) {
  if (dict) {
    return (
      <I18nContext.Provider value={{ dict, locale, currency }}>
        {children}
      </I18nContext.Provider>
    );
  }
  const rules = terms ?? [];
  const synced = resolveDict(locale, rules);
  if (synced) {
    return (
      <I18nContext.Provider value={{ dict: synced, locale, currency }}>
        {children}
      </I18nContext.Provider>
    );
  }
  // 浏览器首帧：字典懒加载 chunk 未就绪。抛 promise 交 layout 的 Suspense——
  // 注意这里不能渲染 children（useI18n 消费点会拿到 null context 直接炸），
  // 挂起期间界面由已有的 SSR HTML 呈现。
  throw loadDict(locale, rules);
}

export function useI18n(): { dict: Dict; locale: Locale; currency: string } {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("useI18n must be used within LocaleProvider");
  return ctx;
}
