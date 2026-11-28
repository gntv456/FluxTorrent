"use client";

/**
 * 404 页按路径类型补充提示（2026-11-28 站长口径）：
 * 通用文案只说「种子页地址形如 /torrent/{id}」，对 /games、/preserve、
 * /resurrections 等功能页的 404 文不对题。本件读 usePathname 按前缀
 * 回对应板块的地址形如说明；无匹配前缀时渲染 null（通用文案已够）。
 * 为什么是 client：App Router 的 not-found.tsx 拿不到 pathname，
 * 只有 client 组件能用 usePathname 读。
 */
import { usePathname } from "next/navigation";

import { useI18n } from "@/i18n/client";

const HINTS: [RegExp, string][] = [
  [/^\/games(\/|$)/, "notFoundHintGames"],
  [/^\/gacha/, "notFoundHintGames"],
  [/^\/preserve/, "notFoundHintPreserve"],
  [/^\/resurrections/, "notFoundHintRescue"],
  [/^\/torrent(\/|$)/, "notFoundHintTorrent"],
];

export function NotFoundHint() {
  const pathname = usePathname();
  const { dict } = useI18n();
  const hit = HINTS.find(([re]) => re.test(pathname ?? ""));
  if (!hit) return null;
  const key = hit[1] as keyof typeof dict.common;
  const tip = dict.common[key] as string | undefined;
  if (!tip) return null;
  return <p className="text-sm text-sub">{tip}</p>;
}
