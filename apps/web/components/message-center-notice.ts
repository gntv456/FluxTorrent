"use client";

import type { MessageRow } from "@/components/message-center-table";

/** 系统通知的「标题/正文」取词（从 message-center-table.tsx 按域拆出）。
 *
 *  0288 / P1-9：系统通知带 kind 时按键现取译文 + 插值（切语言翻得动）；
 *  命中不到键（老消息、未知 kind）就回落后端存的成品文本。
 *  表格与手机卡片两处共用这一支，别各写一份。 */
export function noticeText(
  t: {
    kindReviewApproved: string;
    bodyReviewApproved: string;
    kindReviewRejected: string;
    bodyReviewRejected: string;
    kindGroupNewVersion: string;
    bodyGroupNewVersion: string;
    kindNetworkNewRelease: string;
    bodyNetworkNewRelease: string;
    kindSubgroupNewRelease: string;
    bodySubgroupNewRelease: string;
  },
  m: MessageRow,
  part: "s" | "b",
  fallback: string,
): string {
  const NOTICE: Record<string, { s: string; b: string }> = {
    review_approved: { s: t.kindReviewApproved, b: t.bodyReviewApproved },
    review_rejected: { s: t.kindReviewRejected, b: t.bodyReviewRejected },
    group_new_version: { s: t.kindGroupNewVersion, b: t.bodyGroupNewVersion },
    network_new_release: {
      s: t.kindNetworkNewRelease,
      b: t.bodyNetworkNewRelease,
    },
    subgroup_new_release: {
      s: t.kindSubgroupNewRelease,
      b: t.bodySubgroupNewRelease,
    },
  };
  const k = m.kind ? NOTICE[m.kind] : undefined;
  if (!k) return fallback;
  const p = (m.params ?? {}) as Record<string, unknown>;
  return k[part].replace(/\{(\w+)\}/g, (_x, key) => String(p[key] ?? ""));
}
