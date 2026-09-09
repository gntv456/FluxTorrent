/** 服务端 i18n 入口：读 cookie 定位语言（仅 RSC / Server Components 内使用） */

import { cookies } from "next/headers";
import { DEFAULT_LOCALE, isLocale, LOCALE_COOKIE, type Locale } from "./config";
import { zhCN, type Dict } from "./zh-CN";
import { zhTW } from "./zh-TW";
import { en } from "./en";

const DICTS: Record<Locale, Dict> = { "zh-CN": zhCN, "zh-TW": zhTW, en };

export async function getLocale(): Promise<Locale> {
  const store = await cookies();
  const v = store.get(LOCALE_COOKIE)?.value;
  return isLocale(v) ? v : DEFAULT_LOCALE;
}

export async function getDict(): Promise<{ dict: Dict; locale: Locale }> {
  const locale = await getLocale();
  return { dict: DICTS[locale], locale };
}

export type { Dict };
