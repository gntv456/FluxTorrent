"use client";

/** 客户端 i18n：LocaleProvider 注入整本字典（gzip 后数 KB），useI18n() 消费 */

import { createContext, useContext } from "react";
import type { Dict } from "./zh-CN";
import type { Locale } from "./config";
import { ApiError } from "@/lib/api-client";

const I18nContext = createContext<{ dict: Dict; locale: Locale } | null>(null);

export function LocaleProvider({
  dict,
  locale,
  children,
}: {
  dict: Dict;
  locale: Locale;
  children: React.ReactNode;
}) {
  return (
    <I18nContext.Provider value={{ dict, locale }}>
      {children}
    </I18nContext.Provider>
  );
}

export function useI18n(): { dict: Dict; locale: Locale } {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("useI18n must be used within LocaleProvider");
  return ctx;
}

/** 错误消息：优先按 code 查字典（与后端 Accept-Language 协商互为兜底） */
export function apiErrorMessage(dict: Dict, err: unknown): string {
  if (err instanceof ApiError) return dict.errors[err.code] ?? err.message;
  return err instanceof Error ? err.message : dict.common.networkError;
}
