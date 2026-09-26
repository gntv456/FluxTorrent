"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { avatarFrameStyle, FrameImageOverlay } from "@/lib/format";

interface FrameRow {
  id: number;
  name: string;
  css: string;
  image_url?: string | null;
}

let cache: Promise<FrameRow[]> | null = null;

/** 框列表全站共享一次请求（商店卡片/购买弹窗都会用） */
function loadFrames(): Promise<FrameRow[]> {
  cache ??= api
    .get<FrameRow[]>("/api/v1/avatar-frames")
    .catch(() => [] as FrameRow[]);
  return cache;
}

/**
 * 头像框预览（0207 商店装扮候选）：圆形头像 + 框 css/框图，
 * 与站点各处头像佩戴效果同一套渲染口径（format.tsx 白名单解析）。
 */
export function FramePreview({
  frameId,
  size = 44,
}: {
  frameId: number;
  /** 预览直径（px）：卡片 44 / 弹窗 56 */
  size?: number;
}) {
  const [frame, setFrame] = useState<FrameRow | null>(null);
  useEffect(() => {
    let alive = true;
    void loadFrames().then((rows) => {
      if (alive) setFrame(rows.find((r) => r.id === frameId) ?? null);
    });
    return () => {
      alive = false;
    };
  }, [frameId]);

  return (
    <span
      className={
        "relative inline-flex shrink-0 items-center justify-center " +
        "overflow-hidden rounded-full bg-sky-soft"
      }
      style={{
        width: size,
        height: size,
        ...avatarFrameStyle(frame?.css),
      }}
      aria-hidden
    >
      <FrameImageOverlay url={frame?.image_url} />
      <span
        className="font-display text-sm text-ink/60"
        style={{ fontSize: Math.max(12, Math.round(size / 3)) }}
      >
        {frame?.name?.slice(0, 1) ?? "·"}
      </span>
    </span>
  );
}

/**
 * 动态头像款式预览（0207b）：无图片资产前用 CSS 动画近似两款效果——
 * 霓虹脉冲（neon_pulse）= 呼吸的洋红描边；像素星环（pixel_ring）= 旋转的
 * 虚线青色环。让「按款式挑选」至少能看出两款区别，不再只有一个 ✨。
 */
const EFFECT_CLASS: Record<string, string> = {
  neon_pulse: "aa-preview-neon",
  pixel_ring: "aa-preview-pixel",
};

export function AnimatedAvatarPreview({
  effect,
}: {
  effect?: string;
}) {
  const cls = EFFECT_CLASS[effect ?? ""] ?? "aa-preview-neon";
  return (
    <span
      className={`aa-preview ${cls}`}
      aria-hidden
    >
      <span className="aa-preview-face">✨</span>
    </span>
  );
}
