"use client";

/** 根布局级错误边界（2026-11-20 五问评审 P1 补验）。
 *
 *  与 app/error.tsx 的分工：
 *   - error.tsx        = 路由段渲染/数据异常，仍挂在根 layout 下（有站点壳）；
 *   - global-error.tsx = 根 layout 自身抛错（getAllTerms / getSiteProfile /
 *     ThemeNoFlash 等），此时连站点壳都没了，Next 会落内置白页。
 *
 *  Next.js 硬性要求：global-error.tsx **必须自带 <html> 与 <body>**。
 *  本文件不能调用 useI18n()（LocaleProvider 挂在根 layout 里，它已经崩了），
 *  所以直接 import 四语 review 段，按 document.cookie 的语言标记兜底选词；
 *  这样既四语齐全，又不引入任何硬编码文案（i18n_guard 零新增）。
 */
import { useEffect } from "react";
import Link from "next/link";
import { LOCALE_COOKIE, type Locale } from "@/i18n/config";
import { reviewZh, reviewTw, reviewEn, reviewJa } from "@/i18n/review";

const DICTS = {
  "zh-CN": reviewZh,
  "zh-TW": reviewTw,
  en: reviewEn,
  // ja 是 Partial，调用侧用 ?? reviewZh 兜底（段级回落同 server.ts）
  ja: reviewJa,
};

function pickLang(): Locale {
  let lang = "";
  try {
    if (typeof document !== "undefined") {
      const hit = document.cookie
        .split("; ")
        .find((c) => c.startsWith(LOCALE_COOKIE + "="));
      lang = hit ? decodeURIComponent(hit.slice(LOCALE_COOKIE.length + 1)) : "";
    }
  } catch {
    /* 隐私模式下读 cookie 失败：回落简体 */
  }
  const key: Locale =
    lang === "zh-TW" || lang === "en" || lang === "ja" ? lang : "zh-CN";
  return key;
}

export default function GlobalError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  const lang = pickLang();
  const w = DICTS[lang] ?? reviewZh;
  useEffect(() => {
    console.error("[global-error]", error.digest ?? error.message);
  }, [error]);

  return (
    <html lang={lang}>
      <body className="ge-body">
        <div className="ge-card">
          <p className="ge-icon" aria-hidden>
            ⚠
          </p>
          <h1 className="ge-title">{w.geTitle}</h1>
          <p className="ge-desc">{w.geDesc}</p>
          <div className="ge-actions">
            <button type="button" className="ge-btn ge-btn--primary" onClick={reset}>
              {w.geRetry}
            </button>
            <Link href="/" className="ge-btn ge-btn--ghost">
              {w.geHome}
            </Link>
            <Link href="/contactstaff" className="ge-link">
              {w.geContact}
            </Link>
          </div>
          <p className="ge-ref">
            {w.geRef}: <code>{error.digest ?? "-"}</code>
          </p>
        </div>
      </body>
    </html>
  );
}
