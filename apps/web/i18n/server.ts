/** 服务端 i18n 入口：读 cookie 定位语言（仅 RSC / Server Components 内使用） */

import { cookies } from "next/headers";
import { DEFAULT_LOCALE, isLocale, LOCALE_COOKIE, type Locale } from "./config";
import { zhCN, type Dict } from "./zh-CN";
import { zhTW } from "./zh-TW";
import { en } from "./en";
import { getSiteProfile } from "@/lib/site-profile";

const DICTS: Record<Locale, Dict> = { "zh-CN": zhCN, "zh-TW": zhTW, en };

export async function getLocale(): Promise<Locale> {
  const store = await cookies();
  const v = store.get(LOCALE_COOKIE)?.value;
  return isLocale(v) ? v : DEFAULT_LOCALE;
}

/** 站点货币名（0082，默认「魔力」，站长后台可改）：site-profile 失败回落默认 */
export async function getCurrencyName(): Promise<string> {
  try {
    const p = await getSiteProfile();
    if (p.currency_name && p.currency_name.trim()) return p.currency_name.trim();
  } catch {
    // 站点档案不可用时全站仍需可用，回落默认货币名
  }
  return "魔力";
}

export async function getDict(): Promise<{
  dict: Dict;
  locale: Locale;
  currency: string;
}> {
  const locale = await getLocale();
  const currency = await getCurrencyName();
  return { dict: DICTS[locale], locale, currency };
}

export type { Dict };
