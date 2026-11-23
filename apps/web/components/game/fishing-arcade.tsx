"use client";

/**
 * 甜梦钓场 · 完整渔场（内嵌挂载器）
 *
 * 为什么用 Shadow DOM 而不是直接挂 HTML：
 * - 游戏自带一套 55KB 的全局样式（.frame/.sheet/.bubble/.fab…），直接注入会
 *   和站内 Tailwind / 甜梦皮肤互相污染；
 * - ShadowRoot 提供样式与 DOM 隔离，事件经 composed 冒泡仍可委托；
 * - 游戏脚本本身是 IIFE + 全局命名空间，按序 append <script> 即可重复挂载，
 *   卸载时调用游戏暴露的 __FT_FISHING_DESTROY__ 清掉心跳/计时器/监听。
 *
 * 资源在 public/arcade/fishing/（与 design/arcade-sweet-dream/game 同源）。
 */

import { useEffect, useRef, useState } from "react";
import { useI18n } from "@/i18n/client";

const BASE = "/arcade/fishing";
const SCRIPTS = [
  "js/data.js",
  "js/engine.js",
  "js/store.js",
  "js/ui.js",
  "js/main.js",
] as const;

/** 覆盖游戏自带的 100dvh / 桌面限宽，让它填满宿主容器 */
const HOST_CSS = `
:host{display:block;position:relative;height:100%;min-height:0;overflow:hidden;color:#2c3e5c}
.app{height:100%!important;padding:0!important}
.frame{height:100%!important;max-width:100%!important;
  border-radius:0!important;box-shadow:none!important}
.view{overscroll-behavior:contain}
/* 游戏原版按整屏设计：overlay/toast 是 position:fixed。嵌进卡片后 fixed 会
   覆盖整站（收线/结算弹层跑到页面级），改成相对 :host 的绝对定位。 */
.overlay{position:absolute!important;inset:0!important}
.toast-wrap{position:absolute!important;left:50%!important;bottom:96px!important;
  width:min(86%,420px)!important}
#dbg{display:none!important}
`;

interface ArcadeWindow extends Window {
  FT_ROOT?: ShadowRoot | Document | null;
  FT_NS?: string;
  /** 站点 API 前缀：设了它，存档走服务端（/games/fishing/pond），否则本地存档 */
  FT_API?: string;
  __FT_FISHING_DESTROY__?: (() => void) | null;
}

function loadScript(
  src: string,
  timeoutMsg: string,
  failMsg: string,
  timeoutMs = 12000,
): Promise<void> {
  return new Promise((resolve, reject) => {
    const el = document.createElement("script");
    const timer = window.setTimeout(() => {
      cleanup();
      reject(new Error(timeoutMsg + src));
    }, timeoutMs);
    function cleanup() {
      window.clearTimeout(timer);
      el.onload = null;
      el.onerror = null;
    }
    el.async = false;
    el.src = src;
    el.onload = () => {
      cleanup();
      resolve();
    };
    el.onerror = () => {
      cleanup();
      reject(new Error(failMsg + src));
    };
    document.head.appendChild(el);
  });
}

export function FishingArcade({ ns = "guest" }: { ns?: string }) {
  const { dict } = useI18n();
  const tf = dict.games.fishing;
  const hostRef = useRef<HTMLDivElement | null>(null);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const w = window as ArcadeWindow;
    let cancelled = false;

    const shadow = host.shadowRoot ?? host.attachShadow({ mode: "open" });
    shadow.innerHTML = "";

    (async () => {
      const [html, css] = await Promise.all([
        fetch(BASE + "/index.html", { cache: "force-cache" }).then((r) => {
          if (!r.ok) throw new Error(tf.pondShellFail);
          return r.text();
        }),
        fetch(BASE + "/css/game.css", { cache: "force-cache" }).then((r) => {
          if (!r.ok) throw new Error(tf.pondCssFail);
          return r.text();
        }),
      ]);
      if (cancelled) return;

      const doc = new DOMParser().parseFromString(html, "text/html");
      doc.querySelectorAll("script,link").forEach((n) => n.remove());
      shadow.innerHTML =
        `<style>${css}</style><style>${HOST_CSS}</style>` + doc.body.innerHTML;

      // 先定作用域、存档命名空间与 API 前缀，再加载 game 脚本（main.js 读到才 boot）
      w.FT_ROOT = shadow;
      w.FT_NS = ns;
      w.FT_API = "/api/v1";
      w.__FT_FISHING_DESTROY__ = null;
      for (const src of SCRIPTS) {
        await loadScript(BASE + "/" + src, tf.pondLoadTimeout, tf.pondLoadFail);
        if (cancelled) return;
      }
      setReady(true);
    })().catch((e: unknown) => {
      if (cancelled) return;
      setError(e instanceof Error ? e.message : tf.pondInitFail);
    });

    return () => {
      cancelled = true;
      try {
        w.__FT_FISHING_DESTROY__?.();
      } catch {
        /* 卸载期忽略 */
      }
      w.__FT_FISHING_DESTROY__ = null;
      w.FT_ROOT = null;
      w.FT_API = undefined;
      setReady(false);
    };
  }, [ns]);

  return (
    <div
      className="relative w-full overflow-hidden rounded-2xl
        border border-[#d8e4f2] bg-[#f4f8fd] shadow-sm
        h-[min(920px,calc(100dvh-350px))] min-h-[360px]
        md:h-[min(920px,calc(100dvh-176px))] md:min-h-[540px]"
    >
      <div
        ref={hostRef}
        className="h-full w-full"
        aria-label={tf.pondStageLabel}
      />
      {!ready && !error ? (
        <div className="absolute inset-0 grid place-items-center bg-[#f4f8fd]">
          <div className="flex flex-col items-center gap-2 text-xs text-sub">
            <span
              className="size-6 animate-spin rounded-full border-2
                border-[#c5dff0] border-t-[#4a7ab0]"
              aria-hidden
            />
            <span>{tf.pondLoading}</span>
          </div>
        </div>
      ) : null}
      {error ? (
        <div className="absolute inset-0 grid place-items-center
          bg-[#f4f8fd] px-6 text-center">
          <div className="max-w-sm">
            <p className="text-sm font-semibold text-[#2c3e5c]">{error}</p>
            <p className="mt-2 text-xs text-sub">
              {tf.pondAloneHint}
              <a
                className="ml-1 underline"
                href={BASE + "/index.html"}
                target="_blank"
                rel="noreferrer"
              >
                /arcade/fishing/
              </a>
            </p>
          </div>
        </div>
      ) : null}
    </div>
  );
}
