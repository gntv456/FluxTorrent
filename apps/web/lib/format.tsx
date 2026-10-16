import type React from "react";
import type { TorrentListItem, UserPublic } from "@fluxtorrent/domain-types";

/** 体积人性化（§8.2：金额/流量整数最小单位，前端负责格式化） */
export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v >= 100 ? v.toFixed(0) : v.toFixed(1)} ${units[i]}`;
}

/** 分享率：INF 特殊样式（设计稿数据口径） */
export function formatRatio(up: number, down: number): string {
  if (down === 0) return "INF";
  const r = up / down;
  return r >= 100 ? "INF" : r.toFixed(2);
}

/** 促销徽章（§7.3：免费=薄荷绿 / 2x=珊瑚橙）。label 由调用方按
 *  dict.promotion[key] 本地化。
 *  2026-10-16（S2 Badge）：返回 **tone/variant** 而非 Tailwind 色类串——
 *  配色从「写死在 switch 里」改为交给 Badge 原语，明暗主题自动适配。
 *  映射：free=success 实底；x2/x2free=danger 实底；half/x2half/p30=warn 浅底。 */
export type PromotionKey = "free" | "x2" | "x2free" | "half" | "x2half" | "p30";

/** Badge 的语义档（与 components/ui/badge.tsx 的 props 同形） */
export type PromotionTone = {
  tone: "success" | "danger" | "warn";
  variant: "soft" | "solid";
};

export function promotionBadge(
  promotion: TorrentListItem["promotion"],
): ({ key: PromotionKey } & PromotionTone) | null {
  switch (promotion) {
    case "free":
      return { key: "free", tone: "success", variant: "solid" };
    case "x2":
      return { key: "x2", tone: "danger", variant: "solid" };
    case "x2free":
      return { key: "x2free", tone: "danger", variant: "solid" };
    case "half":
      return { key: "half", tone: "warn", variant: "soft" };
    case "x2half":
      return { key: "x2half", tone: "warn", variant: "soft" };
    case "p30":
      return { key: "p30", tone: "warn", variant: "soft" };
    default:
      return null;
  }
}

/** 用户等级 → 成长豆荚名称（§3.3，数据口径与后端 user_classes 一致） */
export function podName(user: Pick<UserPublic, "class_name">): string {
  return user.class_name;
}

/**
 * 头像框 CSS 白名单解析：avatar_frames.css 只放行 border-color / box-shadow
 * 两个声明（与 plugins.tsx frameStyle 同一口径，服务端存的 css 是可信内容，
 * 白名单是防历史脏数据/未来后台误编辑）。套在圆形头像容器的 style 上。
 */
export function avatarFrameStyle(
  css: string | null | undefined,
): React.CSSProperties {
  const style: Record<string, string> = {};
  if (!css) return style;
  for (const decl of css.split(";")) {
    const [k, v] = decl.split(":").map((s) => s?.trim());
    if (k && v && ["border-color", "box-shadow"].includes(k)) style[k] = v;
  }
  return style as React.CSSProperties;
}

/**
 * 头像框图片叠层（0140）：框有 image_url（PNG/GIF 立绘框）时在头像容器内
 * 绝对定位铺满叠一层框图，不挡点击（pointer-events:none）。
 * 用法：头像容器需要 relative；<FrameImageOverlay url={f.avatar_frame_image} />。
 * 与 CSS 描边形态互斥由调用方决定：有图时通常不再套 avatarFrameStyle。
 */
export function FrameImageOverlay({ url }: { url: string | null | undefined }) {
  if (!url) return null;
  return (
    // eslint-disable-next-line @next/next/no-img-element
    <img
      src={url}
      alt=""
      aria-hidden
      draggable={false}
      className="pointer-events-none absolute inset-0 z-10 h-full w-full select-none object-fill"
    />
  );
}
