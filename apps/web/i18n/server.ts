/** 服务端 i18n 入口：读 cookie 定位语言（仅 RSC / Server Components 内使用） */

import { cookies } from "next/headers";
import { cache } from "react";
import {
  DEFAULT_LOCALE,
  isLocale,
  LOCALE_COOKIE,
  siteLangToLocale,
  type Locale,
} from "./config";
import { zhCN, type Dict } from "./zh-CN";
import { zhTW } from "./zh-TW";
import { en } from "./en";
import { ja } from "./ja";
import { deepMergeDict } from "./merge";
import { getSiteProfile } from "@/lib/site-profile";
import { applyTerms, sortRules, subtitleRules, type TermRule } from "./apply-terms";

// ja（E10）：段级回落式——翻到的段覆盖，其余回落 zh-CN
const DICTS: Record<Locale, Dict> = {
  "zh-CN": zhCN,
  "zh-TW": zhTW,
  en,
  ja: deepMergeDict(zhCN, ja),
};

export async function getLocale(): Promise<Locale> {
  const store = await cookies();
  const v = store.get(LOCALE_COOKIE)?.value;
  if (isLocale(v)) return v;
  // 0216：没有语言 cookie 的新访客 → 站点默认语言（后台「默认语言」，真驱动）。
  // 库里是 NP 口径 en|chs|cht，经唯一换算处转 BCP47；档案取不到则内置默认。
  const p = await getSiteProfile();
  return siteLangToLocale(p.default_language) ?? DEFAULT_LOCALE;
}

/** 站点档案里与「文案」有关的三项（0082 货币名 + 0205 术语规则 + 0146 字幕区显示名）：
 *  一次 getSiteProfile 拿全，别为几个字段各打一次。 */
async function getSiteWords(): Promise<{
  currency: string;
  terms: TermRule[];
  subtitleLabel: string | null;
}> {
  try {
    const p = await getSiteProfile();
    return {
      currency: p.currency_name?.trim() || "魔力",
      terms: sortRules(p.terms ?? []),
      // subtitle_label（0208 P0 接真）：非默认值时作为术语规则注入——
      // 「字幕」→站长设定的叫法（音乐站「歌词」），52 处字典文案一次性改写，
      // 与 0205 术语表同一条出口，不需要逐组件接 profile。
      subtitleLabel:
        p.subtitle_label && p.subtitle_label.trim() && p.subtitle_label !== "字幕"
          ? p.subtitle_label.trim()
          : null,
    };
  } catch {
    // 站点档案不可用时全站仍需可用：默认货币名 + 不改写
    return { currency: "魔力", terms: [], subtitleLabel: null };
  }
}

/**
 * 取当前语言的字典。**术语表（0205 / 四审 L7）与字幕区显示名（0146/0208）
 * 都在这一个出口生效**：字典叶子字符串过一遍规则，236 个 useI18n 消费点
 * 与所有 RSC 自动跟随，不需要把「种子 / 魔力」这些词从字典里抠成占位符。
 * cache()：一次请求内 57 处 getDict 调用共用同一本改写后的字典
 * （改写要遍历数千个叶子，重复做就是白烧 CPU）。
 */
export const getDict = cache(
  async (): Promise<{ dict: Dict; locale: Locale; currency: string }> => {
    const locale = await getLocale();
    const { currency, terms, subtitleLabel } = await getSiteWords();
    // 字幕区改名规则与站长术语规则合并（G9：复合词保护随规则一起注入，
    // 长词优先保证「字幕组 / 字幕人」不被拆成生造词）
    const allTerms = subtitleLabel
      ? [...terms, ...subtitleRules(subtitleLabel)]
      : terms;
    return { dict: applyTerms(DICTS[locale], allTerms), locale, currency };
  },
);

export type { Dict };
