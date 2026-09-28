"use client";

import { useEffect, useState } from "react";
import { useI18n } from "@/i18n/client";

/**
 * PWA 安装引导条（E8）：beforeinstallprompt 捕获后顶部浮条一键安装；
 * 已安装（display-mode: standalone）/已拒绝（localStorage）/无事件时不渲染。
 * iOS Safari 不发 beforeinstallprompt，显示手动指引（分享→添加到主屏幕）。
 */
export function PwaInstallBar() {
  const { dict } = useI18n();
  const t = dict.pwa;
  const [deferred, setDeferred] = useState<
    Event & { prompt: () => Promise<void> } | null
  >(null);
  const [iosHint, setIosHint] = useState(false);
  const [hidden, setHidden] = useState(true);

  useEffect(() => {
    if (typeof window === "undefined") return;
    if (localStorage.getItem("flux-pwa-dismiss") === "1") return;
    const standalone =
      window.matchMedia("(display-mode: standalone)").matches ||
      // iOS Safari 专用（无 beforeinstallprompt；standalone 判定走 navigator）
      (navigator as unknown as { standalone?: boolean }).standalone === true;
    if (standalone) return;
    const onPrompt = (e: Event) => {
      e.preventDefault();
      setDeferred(e as Event & { prompt: () => Promise<void> });
      setHidden(false);
    };
    window.addEventListener("beforeinstallprompt", onPrompt);
    // iOS：无事件 3s 后仍未安装则给手动指引
    const isIOS = /iphone|ipad|ipod/i.test(navigator.userAgent);
    const timer = window.setTimeout(() => {
      if (isIOS) {
        setIosHint(true);
        setHidden(false);
      }
    }, 3000);
    return () => {
      window.removeEventListener("beforeinstallprompt", onPrompt);
      window.clearTimeout(timer);
    };
  }, []);

  if (hidden || (!deferred && !iosHint)) return null;

  return (
    <div className="pwa-install-bar flex items-center gap-2 px-4 py-2 text-xs">
      <span className="flex-1">
        {deferred ? t.installHint : t.iosHint}
      </span>
      {deferred && (
        <button
          type="button"
          className="rounded-[var(--r-sm)] bg-sky px-3 py-1 font-bold"
          onClick={async () => {
            await deferred.prompt();
            setHidden(true);
          }}
        >
          {t.installBtn}
        </button>
      )}
      <button
        type="button"
        aria-label={t.dismiss}
        className="px-2 py-1 text-sub"
        onClick={() => {
          localStorage.setItem("flux-pwa-dismiss", "1");
          setHidden(true);
        }}
      >
        ✕
      </button>
    </div>
  );
}
