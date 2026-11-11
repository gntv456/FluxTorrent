/** 字典供给侧（2026-11-11 第二轮瘦身的唯一实现，双端共用）。
 *
 *  背景：layout 不再把整本字典作为 LocaleProvider 的 prop 传入——那会让
 *  React 把它序列化进每个页面的 flight payload（实测 174KB/页，HTML 不可
 *  缓存、每次访问重付）。字典改为**每语言一颗懒加载 chunk**：浏览器按需
 *  下载且落 /_next/static（SW cache-first + immutable）；服务端同一份
 *  import() 在 bundle 内是本地加载（微秒级）。
 *
 *  曾试过「服务端同步实现 + next.config webpack 别名按编译切换」——实测
 *  Next 15.3 App Router build 不执行 next.config 的 webpack 回调，别名失效
 *  导致四本字典全量回到 eager 包（First Load 103→414KB）。改为本文件的单
 *  实现 + 模块初始化预热：
 *  - 服务端：模块加载即预热全部四语（进程常驻，首个请求前 promise 已落定）
 *  - 浏览器：模块加载即预热 cookie 语言（与剩余 JS chunk 并行下载）
 *  Provider 首帧 resolveDict 未就绪时抛 loadDict 的 promise，layout 的
 *  Suspense 接住；预热使 promise 在抛出前已落定，React（Fizz）同步重试，
 *  SSR 不刷 fallback、HTML 完整；浏览器水合推迟到字典就绪，文案不闪。
 *
 *  水合等价性契约：两端对同一 locale 用同一 applyTerms 处理同一本源字典
 *  （terms 由 layout 经 getAllTerms() 以 prop 下发——TermRule 仅
 *  canonical/replacement 两字段，hxpt 实测 0 条），产出逐字一致。 */

import { applyTerms, termsFingerprint, type TermRule } from "./apply-terms";
import { deepMergeDict } from "./merge";
import {
  DEFAULT_LOCALE,
  isLocale,
  LOCALES,
  LOCALE_COOKIE,
  type Locale,
} from "./config";
import type { Dict } from "./zh-CN";

// 每语言一个懒加载 chunk（webpack 按 import() 分包）；ja 是 zh-CN 的段级
// 覆盖，两颗 chunk 合并后再改写——与 i18n/server.ts 的 DICTS 口径一致
const LOADERS: Record<Locale, () => Promise<Dict>> = {
  "zh-CN": async () => (await import("./zh-CN")).zhCN,
  "zh-TW": async () => (await import("./zh-TW")).zhTW,
  en: async () => (await import("./en")).en,
  ja: async () => {
    const [{ zhCN }, { ja }] = await Promise.all([
      import("./zh-CN"),
      import("./ja"),
    ]);
    return deepMergeDict(zhCN, ja);
  },
};

// 原始字典 promise 缓存（语言级，跨请求/导航复用；失败剔除允许重试）
const rawCache = new Map<Locale, Promise<Dict>>();
// 改写结果缓存（语言+规则指纹级；terms 每请求新引用，按指纹 memo）
const applied = new Map<string, Dict>();

function normLocale(locale: string): Locale {
  return isLocale(locale) ? locale : DEFAULT_LOCALE;
}

function loadRaw(locale: Locale): Promise<Dict> {
  let p = rawCache.get(locale);
  if (!p) {
    p = LOADERS[locale]();
    rawCache.set(locale, p);
    p.catch(() => rawCache.delete(locale));
  }
  return p;
}

// —— 模块初始化预热：让「抛出的 promise 在抛出前已落定」成为常态 ——
// 服务端：全语预热（进程启动即完成，之后每个请求的字典都是同步缓存命中）。
if (typeof window === "undefined") {
  for (const locale of LOCALES) loadRaw(locale);
} else {
  // 浏览器：只预热 cookie 语言（chunk 下载与其余 JS 并行，不阻塞水合起步）
  const m = document.cookie.match(
    new RegExp(`(?:^|;\\s*)${LOCALE_COOKIE}=([^;]+)`),
  );
  const cookieLocale = m ? decodeURIComponent(m[1]) : undefined;
  if (isLocale(cookieLocale)) loadRaw(cookieLocale);
}

/** 已就绪的改写字典；未就绪返回 null（Provider 抛 loadDict 的 promise）。 */
export function resolveDict(locale: string, rules: TermRule[]): Dict | null {
  const key = `${normLocale(locale)}\u0000${termsFingerprint(rules)}`;
  return applied.get(key) ?? null;
}

/** 加载并改写字典；幂等（同语言+同规则返回同一 promise）。 */
export function loadDict(locale: string, rules: TermRule[]): Promise<Dict> {
  const loc = normLocale(locale);
  const key = `${loc}\u0000${termsFingerprint(rules)}`;
  const hit = applied.get(key);
  if (hit) return Promise.resolve(hit);
  return loadRaw(loc).then((raw) => {
    const dict = applyTerms(raw, rules);
    if (applied.size > 8) applied.clear();
    applied.set(key, dict);
    return dict;
  });
}
