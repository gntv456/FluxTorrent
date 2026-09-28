"use client";

import { useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { LOCALES, LOCALE_COOKIE, type Locale } from "@/i18n/config";
import { Icon } from "@/components/icons";

/** 语言切换器（0148 下拉版）：地球图标按钮 + 弹出菜单（好学/财神口径）。
 *  选中写 cookie + router.refresh()（RSC 重取，无 URL 变化）；
 *  点外部/ESC 收起。原三按钮并排形态改为收进菜单，给导航条腾位。 */

const LABELS: Record<Locale, string> = {
  "zh-CN": "简体中文",
  "zh-TW": "繁體中文",
  en: "English",
  ja: "日本語",
};

export function LocaleSwitcher({ current }: { current: Locale }) {
  const router = useRouter();
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node))
        setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  function switchTo(l: Locale) {
    setOpen(false);
    if (l === current) return;
    document.cookie = `${LOCALE_COOKIE}=${l}; path=/; max-age=31536000; samesite=lax`;
    router.refresh();
  }

  return (
    <div className="langmenu" ref={rootRef}>
      <button
        type="button"
        className="langmenu__trigger"
        aria-expanded={open}
        aria-haspopup="true"
        aria-label="Language / 语言"
        title={LABELS[current]}
        onClick={() => setOpen((v) => !v)}
      >
        <Icon name="globe" size={19} />
        <span className="langmenu__current">{current === "en" ? "EN" : ""}</span>
        <span className="langmenu__caret" aria-hidden="true">
          ▾
        </span>
      </button>
      {open && (
        <div className="langmenu__drop" role="menu">
          {LOCALES.map((l) => (
            <button
              key={l}
              type="button"
              role="menuitemradio"
              aria-checked={l === current}
              className="langmenu__item"
              data-active={l === current ? "true" : undefined}
              onClick={() => switchTo(l)}
            >
              <span>{LABELS[l]}</span>
              {l === current && (
                <span className="langmenu__check" aria-hidden="true">
                  ✓
                </span>
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
