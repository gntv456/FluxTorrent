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
  if (err instanceof ApiError) return dict.errors[err.code] ?? err.message;
  return err instanceof Error ? err.message : dict.common.networkError;
}
