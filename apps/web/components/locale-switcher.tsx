"use client";

/** 语言切换器：写 cookie + router.refresh()（RSC 重取，无 URL 变化） */

import { useRouter } from "next/navigation";
import { LOCALES, LOCALE_COOKIE, type Locale } from "@/i18n/config";

const LABELS: Record<Locale, string> = {
  "zh-CN": "简体",
  "zh-TW": "繁體",
  en: "EN",
};

export function LocaleSwitcher({ current }: { current: Locale }) {
  const router = useRouter();

  function switchTo(l: Locale) {
    if (l === current) return;
    document.cookie = `${LOCALE_COOKIE}=${l}; path=/; max-age=31536000; samesite=lax`;
    router.refresh();
  }

  return (
    <div
      className="flex items-center gap-0.5 rounded-full bg-white/10 p-0.5"
      role="group"
      aria-label="Language / 语言"
    >
      {LOCALES.map((l) => (
        <button
          key={l}
          type="button"
          onClick={() => switchTo(l)}
          aria-current={l === current ? "true" : undefined}
          className={`min-h-[28px] rounded-full px-2.5 text-xs font-bold transition-colors ${
            l === current
              ? "bg-sky text-white"
              : "text-white/70 hover:text-white"
          }`}
        >
          {LABELS[l]}
        </button>
      ))}
    </div>
  );
}
