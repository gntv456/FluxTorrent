"use client";

/** 未计收益提示（从 page.tsx 按域拆出，300 行门禁）。
 *  「做种 6 颗、只有 4 颗计收益」必须说清是哪几颗、为什么 —— 只扣不解释，
 *  用户只会以为站点算错。三类原因与后端 earning_split 的分类一一对应，
 *  且互斥，因此三项之和恰等于「总数 - 计收益」。 */

import type { Dict } from "@/i18n/zh-CN";
import type { SparkInfo } from "./spark-types";

type MySparkDict = Dict["myspark"];

export function ExcludedNote({
  info,
  t,
}: {
  info: SparkInfo;
  t: MySparkDict;
}) {
  const ex = info.seeding_excluded;
  const excluded = ex ? ex.port_closed + ex.unreachable + ex.stale : 0;
  if (!info.payout_suspended && excluded === 0) return null;
  const reasons: string[] = [];
  if (ex?.port_closed) reasons.push(`${t.excludedPort} ${ex.port_closed}`);
  if (ex?.unreachable) {
    reasons.push(`${t.excludedUnreach} ${ex.unreachable}`);
  }
  if (ex?.stale) reasons.push(`${t.excludedStale} ${ex.stale}`);
  return (
    <div
      className={
        "brand-panel border-l-2 border-[var(--danger-text)] px-4 py-3"
      }
    >
      <p className="text-sm font-bold text-[var(--danger-text)]">
        {info.payout_suspended
          ? t.payoutSuspended
          : t.excludedTitle.replace("{n}", String(excluded))}
      </p>
      {!info.payout_suspended && reasons.length > 0 && (
        <p className="mt-1 text-xs text-sub">{reasons.join(" · ")}</p>
      )}
      <details className="mt-2">
        <summary className="cursor-pointer text-xs text-sub">
          {t.excludedFix}
        </summary>
        <ul className="mt-1 list-disc pl-4 text-xs text-sub">
          {!!ex?.port_closed && <li>{t.excludedFixPort}</li>}
          {!!ex?.unreachable && <li>{t.excludedFixUnreach}</li>}
          {!!ex?.stale && <li>{t.excludedFixStale}</li>}
        </ul>
      </details>
    </div>
  );
}
