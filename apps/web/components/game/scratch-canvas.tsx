"use client";

import { useCallback, useEffect, useRef, useState } from "react";

const CLEAR_THRESHOLD = 0.55; // 刮开比例达到此值自动开完
const BRUSH = 22; // 笔触半径（CSS px）

/**
 * 刮刮乐涂层（Canvas）。
 * 设计要点：
 * - 按 devicePixelRatio 缩放，高分屏不糊；涂层色取 CSS 变量（暗色主题自动跟随）。
 * - 刮开比例按降采样统计（每 40 个像素取 1 个），避免全图 getImageData 卡顿。
 * - 「一键刮开」是键盘/无障碍的等价路径：无障碍不能只靠指针拖拽。
 */
export function ScratchCanvas({
  armed,
  revealAll,
  onProgress,
  onRevealed,
  label,
}: {
  /** 已买卡、可刮（响应返回后才置 true，杜绝「先刮后开奖」的假结果） */
  armed: boolean;
  /** 外部要求自动开完（reduced-motion / 一键刮开 / 页面切走） */
  revealAll: boolean;
  onProgress?: (pct: number) => void;
  onRevealed: () => void;
  label: string;
}) {
  const ref = useRef<HTMLCanvasElement | null>(null);
  const drawing = useRef(false);
  const done = useRef(false);
  const [painted, setPainted] = useState(false);

  const finish = useCallback(() => {
    if (done.current) return;
    done.current = true;
    const cv = ref.current;
    if (cv) {
      cv.style.transition = "opacity .45s ease";
      cv.style.opacity = "0";
      window.setTimeout(() => {
        cv.style.display = "none";
      }, 480);
    }
    onRevealed();
  }, [onRevealed]);

  // 涂层绘制：armed 变 true 时重画一次（= 新买了一张卡）
  useEffect(() => {
    if (!armed) return;
    const cv = ref.current;
    if (!cv) return;
    done.current = false;
    const w = cv.clientWidth;
    const h = cv.clientHeight;
    if (!w || !h) return;
    const dpr = window.devicePixelRatio || 1;
    cv.width = Math.round(w * dpr);
    cv.height = Math.round(h * dpr);
    const ctx = cv.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.globalCompositeOperation = "source-over";
    cv.style.display = "block";
    cv.style.opacity = "1";

    const cs = getComputedStyle(document.documentElement);
    const sun = cs.getPropertyValue("--sun").trim() || "#ffc93c";
    const coral = cs.getPropertyValue("--coral").trim() || "#ff7a59";
    const g = ctx.createLinearGradient(0, 0, w, h);
    g.addColorStop(0, sun);
    g.addColorStop(1, coral);
    ctx.fillStyle = g;
    ctx.fillRect(0, 0, w, h);
    // 闪粉
    for (let i = 0; i < 180; i++) {
      ctx.fillStyle = `rgba(255,255,255,${0.15 + Math.random() * 0.35})`;
      ctx.fillRect(Math.random() * w, Math.random() * h, 2, 2);
    }
    ctx.globalCompositeOperation = "destination-out";
    setPainted(true);
  }, [armed]);

  // 自动开完（一键刮开 / reduced-motion / 切后台）
  useEffect(() => {
    if (revealAll && armed && !done.current) finish();
  }, [revealAll, armed, finish]);

  const ratio = () => {
    const cv = ref.current;
    if (!cv || !cv.width) return 0;
    const ctx = cv.getContext("2d");
    if (!ctx) return 0;
    const d = ctx.getImageData(0, 0, cv.width, cv.height).data;
    let cleared = 0;
    let total = 0;
    for (let i = 3; i < d.length; i += 4 * 40) {
      total++;
      if (d[i] < 40) cleared++;
    }
    return total ? cleared / total : 0;
  };

  const scratchAt = (x: number, y: number) => {
    const cv = ref.current;
    const ctx = cv?.getContext("2d");
    if (!cv || !ctx || done.current) return;
    ctx.beginPath();
    ctx.arc(x, y, BRUSH, 0, Math.PI * 2);
    ctx.fill();
    const r = ratio();
    onProgress?.(r);
    if (r >= CLEAR_THRESHOLD) finish();
  };

  const pos = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    return { x: e.clientX - rect.left, y: e.clientY - rect.top };
  };

  if (!armed && !painted) return null;

  return (
    <canvas
      ref={ref}
      role="img"
      aria-label={label}
      className="absolute inset-0 h-full w-full cursor-crosshair touch-none"
      onPointerDown={(e) => {
        if (!armed || done.current) return;
        drawing.current = true;
        e.currentTarget.setPointerCapture(e.pointerId);
        const p = pos(e);
        scratchAt(p.x, p.y);
      }}
      onPointerMove={(e) => {
        if (!drawing.current || done.current) return;
        const p = pos(e);
        scratchAt(p.x, p.y);
      }}
      onPointerUp={() => {
        drawing.current = false;
      }}
      onPointerCancel={() => {
        drawing.current = false;
      }}
    />
  );
}
