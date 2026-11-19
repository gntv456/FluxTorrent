"use client";

import { fmt } from "@/i18n/config";

/** 等级规则页的影响预览卡（0499 从 admin-classes.tsx 拆出：行数门禁）。
 *  纯展示：预览数据由父组件请求后传入，草稿一改即被置 null。 */

export interface ImpactPreview {
  promote: number;
  demote: number;
  sample_promote: string[];
  sample_demote: string[];
  current_users: number;
}

interface Labels {
  pvPromote: string;
  pvDemote: string;
  pvCurrent: string;
  pvSampleUp: string;
  pvSampleDown: string;
}

export function ImpactPreviewCard({
  pv,
  at,
}: {
  pv: ImpactPreview;
  at: Labels;
}) {
  return (
    <tr className="border-t border-line">
      <td colSpan={14}>
        <div
          className="rounded-[var(--r-md)] bg-[var(--surface-sunken)] p-3
            text-xs text-ink"
        >
          <p>
            {fmt(at.pvPromote, { n: pv.promote })} ·{" "}
            {fmt(at.pvDemote, { n: pv.demote })} ·{" "}
            {fmt(at.pvCurrent, { n: pv.current_users })}
          </p>
          {pv.sample_promote.length > 0 && (
            <p className="mt-1">
              {at.pvSampleUp}: {pv.sample_promote.join(", ")}
            </p>
          )}
          {pv.sample_demote.length > 0 && (
            <p className="mt-1">
              {at.pvSampleDown}: {pv.sample_demote.join(", ")}
            </p>
          )}
        </div>
      </td>
    </tr>
  );
}
