/** 服务端 i18n 入口：读 cookie 定位语言（仅 RSC / Server Components 内使用） */

import { cookies } from "next/headers";
import { cache } from "react";
import { DEFAULT_LOCALE, isLocale, LOCALE_COOKIE, type Locale } from "./config";
import { zhCN, type Dict } from "./zh-CN";
import { zhTW } from "./zh-TW";
import { en } from "./en";
import { getSiteProfile } from "@/lib/site-profile";
import { applyTerms, sortRules, type TermRule } from "./apply-terms";

const DICTS: Record<Locale, Dict> = { "zh-CN": zhCN, "zh-TW": zhTW, en };

export async function getLocale(): Promise<Locale> {
  const store = await cookies();
  const v = store.get(LOCALE_COOKIE)?.value;
  return isLocale(v) ? v : DEFAULT_LOCALE;
}

/** 站点档案里与「文案」有关的两项（0082 货币名 + 0205 术语规则）：
 *  一次 getSiteProfile 拿全，别为两个字段各打一次。 */
async function getSiteWords(): Promise<{ currency: string; terms: TermRule[] }> {
  try {
    const p = await getSiteProfile();
    return {
      currency: p.currency_name?.trim() || "魔力",
      terms: sortRules(p.terms ?? []),
    };
  } catch {
    // 站点档案不可用时全站仍需可用：默认货币名 + 不改写
    return { currency: "魔力", terms: [] };
  }
}

/**
 * 取当前语言的字典。**术语表（0205 / 四审 L7）就在这一个出口生效**：
 * 字典叶子字符串过一遍规则，236 个 useI18n 消费点与所有 RSC 自动跟随，
 * 不需要把「种子 / 魔力」这些词从字典里抠成占位符。
 * cache()：一次请求内 57 处 getDict 调用共用同一本改写后的字典
 * （改写要遍历数千个叶子，重复做就是白烧 CPU）。
 */
export const getDict = cache(
  async (): Promise<{ dict: Dict; locale: Locale; currency: string }> => {
    const locale = await getLocale();
    const { currency, terms } = await getSiteWords();
    return { dict: applyTerms(DICTS[locale], terms), locale, currency };
  },
);

export type { Dict };
