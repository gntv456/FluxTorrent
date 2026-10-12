"use client";

/**
 * 客户端 i18n：LocaleProvider 注入整本字典（gzip 后数 KB），useI18n() 消费。
 * dict 内文案中的「火花」占位符 {spark}（源字典写作 {magic}）会按站点货币名
 * 动态替换（默认「魔力」，站长可在后台 site_settings.currency_name 改任意名），
 * 所以 useI18n() 额外暴露 fmt() 之外的 currency 名。
 */

import { createContext, useContext } from "react";
import type { Dict } from "./zh-CN";
import type { Locale } from "./config";
import { ApiError } from "@/lib/api-client";

const I18nContext = createContext<{
  dict: Dict;
  locale: Locale;
  currency: string;
} | null>(null);

export function LocaleProvider({
  dict,
  locale,
  currency = "魔力",
  children,
}: {
  dict: Dict;
  locale: Locale;
  currency?: string;
  children: React.ReactNode;
}) {
  return (
    <I18nContext.Provider value={{ dict, locale, currency }}>
      {children}
    </I18nContext.Provider>
  );
}

export function useI18n(): { dict: Dict; locale: Locale; currency: string } {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("useI18n must be used within LocaleProvider");
  return ctx;
}

/** 错误消息：优先按 code 查字典（与后端 Accept-Language 协商互为兜底） */
export function apiErrorMessage(dict: Dict, err: unknown): string {
  if (err instanceof ApiError) {
    // P-117：1002（Validation）的报文带着**具体判据**——「签发 API Token
    // 未执行：凭证无效」「还款金额必须为正」这类。旧写法一律拿字典里
    // 「参数校验失败」盖掉它，用户只知道「失败了」，看不出是自己密码错、
    // 还是要输验证码。取第一个分隔符之后的正文；正文为空时退回原映射。
    // 其余码（1004/2001/4103…）的字典文案更完整，保持原样。
    if (err.code === 1002) {
      const raw = err.message ?? "";
      const at = [":", "："]
        .map((c) => raw.indexOf(c))
        .filter((i) => i >= 0)
        .sort((a, b) => a - b)[0];
      if (at !== undefined) {
        const tail = raw.slice(at + 1).trim();
        if (tail) return tail;
      }
    }
    return dict.errors[err.code] ?? err.message;
  }
  return err instanceof Error ? err.message : dict.common.networkError;
}
