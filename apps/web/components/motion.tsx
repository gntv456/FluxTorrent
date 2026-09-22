"use client";

/**
 * TIDE 动效壳（P4）—— 不渲染任何 DOM，只负责两件「CSS 做不到」的事：
 *
 * 1) 顶栏吸顶态：滚动超过 4px 给 header.tide-header 加 .is-stuck（投影加深）；
 * 2) 滚动进场：给视口下方的板块容器加 .tide-reveal（先隐藏），进入视口后加 .is-in 揭示。
 *
 * 渐进增强纪律（重要）：
 *  - **元素默认可见**。只有 JS 就绪、且元素确实位于首屏之外时才加隐藏类；
 *    脚本失败 / 禁用 / SSR 首屏都不会出现"内容消失"。
 *  - prefers-reduced-motion 时整段跳过（不隐藏、不观察），交给 CSS 的降级分支。
 *  - 已在视口内的元素直接跳过（避免首屏闪烁与布局抖动）。
 *
 * 每次客户端导航（pathname 变化）都会重扫一遍，因此换页后的新板块同样生效。
 */
import { useEffect } from "react";
import { usePathname } from "next/navigation";

/** 参与滚动进场的容器（板块级，不下探到底层行 —— 长列表逐行动画会掉帧） */
const REVEAL_TARGETS = [
  "main .baozi-panel",
  "main .nexus-table",
  "main .up-card",
  "main .tsb-card",
  "main .pting-card",
  "main .rss-card",
  "main .task-panel",
  "main .home-resource-stats__metrics",
  "main .home-site-data__grid",
].join(",");

/** 已由 CSS 首屏编排接管、不再参与滚动进场的区域 */
const SKIP_CLOSEST = [".home-stack", ".home-row", ".home-duo"];

export function MotionShell() {
  const pathname = usePathname();

  useEffect(() => {
    const header = document.querySelector<HTMLElement>("header.tide-header");
    const syncHeader = () => {
      if (header) header.classList.toggle("is-stuck", window.scrollY > 4);
    };
    syncHeader();
    window.addEventListener("scroll", syncHeader, { passive: true });

    const reduce =
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    let observer: IntersectionObserver | undefined;
    let frame = 0;
    let fallbackRaf = 0;
    let onScrollFallback: (() => void) | undefined;
    const marked: HTMLElement[] = [];

    if (!reduce && typeof IntersectionObserver !== "undefined") {
      frame = window.requestAnimationFrame(() => {
        const candidates = Array.from(
          document.querySelectorAll<HTMLElement>(REVEAL_TARGETS),
        ).filter((el) => !SKIP_CLOSEST.some((sel) => el.closest(sel)));

        /**
         * 自愈兜底：`is-in` 只会触发一次「上浮淡入」动画，CSS 并未预隐藏元素。
         * 但若动画被其它样式影响而停在起始帧（opacity 0），这里在动画窗口后强制回退到
         * 「无动画的可见态」—— 保证任何情况下都不会出现内容不可见。
         */
        function ensureVisible(el: HTMLElement) {
          window.setTimeout(() => {
            if (Number(window.getComputedStyle(el).opacity) < 0.5) {
              el.classList.remove("tide-reveal", "is-in");
            }
          }, 1600);
        }

        observer = new IntersectionObserver(
          (entries) => {
            for (const entry of entries) {
              if (entry.isIntersecting) {
                entry.target.classList.add("is-in");
                observer?.unobserve(entry.target);
                ensureVisible(entry.target as HTMLElement);
              }
            }
          },
          // rootMargin 一律不收缩底部：当页高≈视口（几乎不可滚动）时，
          // 收缩会把「文档末尾的元素」永久排除在相交区之外 → 永不揭示。
          { threshold: 0 },
        );

        /** 几何兜底：已标记未揭示、且当前确实在视口内 → 补揭示 */
        const revealIfVisible = () => {
          for (const el of marked) {
            if (el.classList.contains("is-in")) continue;
            const r = el.getBoundingClientRect();
            if (r.top < window.innerHeight && r.bottom > 0) {
              el.classList.add("is-in");
              observer?.unobserve(el);
              ensureVisible(el);
            }
          }
        };

        const viewportH = window.innerHeight;
        for (const el of candidates) {
          // 首屏内已有的板块交给 CSS 首屏编排，不参与滚动进场（避免二次抖动）
          if (el.getBoundingClientRect().top < viewportH * 0.98) continue;
          el.classList.add("tide-reveal");
          observer.observe(el);
          marked.push(el);
        }
        // 滚动兜底（rAF 节流）：保证「标记了就一定会在可见时揭示」
        onScrollFallback = () => {
          if (fallbackRaf) return;
          fallbackRaf = window.requestAnimationFrame(() => {
            fallbackRaf = 0;
            revealIfVisible();
          });
        };
        revealIfVisible();
        window.addEventListener("scroll", onScrollFallback, { passive: true });
      });
    }

    return () => {
      if (frame) window.cancelAnimationFrame(frame);
      if (fallbackRaf) window.cancelAnimationFrame(fallbackRaf);
      observer?.disconnect();
      if (onScrollFallback) window.removeEventListener("scroll", onScrollFallback);
      window.removeEventListener("scroll", syncHeader);
    };
  }, [pathname]);

  return null;
}

export default MotionShell;
