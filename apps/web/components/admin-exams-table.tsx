"use client";

/**
 * 考核岗位·列表子面板（从 components/admin-exams.tsx 按域拆出）：
 * 岗位表（底薪、达标门槛、加成档位、登记数）+ 编辑回填与删除。
 * 指标目录与换算工具拆至 ./admin-exams-shared.ts。
 */

import { api, ApiError } from "@/lib/api-client";
import type { JixiaoTypeRow } from "./admin-exams-shared";
import { GB, GB_KEYS, metricOptions } from "./admin-exams-shared";

interface ExamsTableProps {
  rows: JixiaoTypeRow[];
  busy: boolean;
  currency: string;
  flash: (m: string) => void;
  load: () => Promise<void>;
  startEdit: (r: JixiaoTypeRow) => void;
}

export function ExamsTable({
  rows,
  busy,
  currency,
  flash,
  load,
  startEdit,
}: ExamsTableProps) {
  const METRIC_OPTIONS = metricOptions(currency);
  const metricLabel = (k: string) =>
    METRIC_OPTIONS.find((m) => m.key === k)?.label ?? k;
  const metricUnit = (k: string) =>
    METRIC_OPTIONS.find((m) => m.key === k)?.unit ?? "";
  const fmtReq = (k: string, v: number) =>
    GB_KEYS.has(k)
      ? `${Math.round(v / GB)} GB`
      : `${v} ${metricUnit(k)}`.trim();

  return (
    <table className="nexus-table text-xs">
      <thead>
        <tr>
          <td className="colhead">ID</td>
          <td className="colhead">岗位</td>
          <td className="colhead">底薪</td>
          <td className="colhead">达标门槛</td>
          <td className="colhead">加成</td>
          <td className="colhead">登记数</td>
          <td className="colhead text-right">操作</td>
        </tr>
      </thead>
      <tbody>
        {rows.map((r) => {
          const rules = r.bonus_rules as {
            months_per_step?: number;
            percent_per_step?: number;
          };
          const reqs = Object.entries(r.min_requirements ?? {})
            .filter(([, v]) => typeof v === "number" && (v as number) > 0)
            .map(([k, v]) => `${metricLabel(k)} ${fmtReq(k, v as number)}`)
            .join(" 且 ");
          return (
            <tr key={r.id}>
              <td className="num">{r.id}</td>
              <td className="font-bold">
                {r.name}
                {r.description ? (
                  <span
                    className="ml-1 font-normal text-sub"
                    title={r.description}
                  >
                    ⓘ
                  </span>
                ) : null}
              </td>
              <td className="num">{r.base_pay}</td>
              <td className="max-w-[320px] truncate">{reqs || "无门槛"}</td>
              <td className="num">
                {rules.percent_per_step
                  ? `每${rules.months_per_step ?? 3}月+${rules.percent_per_step}%`
                  : "—"}
              </td>
              <td className="num">{r.assigned_count ?? "—"}</td>
              <td className="text-right">
                <button className="cmgmt-act" onClick={() => startEdit(r)}>
                  编辑
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  disabled={busy}
                  onClick={async () => {
                    try {
                      await api.del(`/api/v1/admin/jixiao-types/${r.id}`);
                      flash("已删除");
                      await load();
                    } catch (e) {
                      flash(e instanceof ApiError ? e.message : "删除失败");
                    }
                  }}
                >
                  删除
                </button>
              </td>
            </tr>
          );
        })}
        {rows.length === 0 && (
          <tr>
            <td colSpan={7} className="py-6 text-center text-sub">
              暂无考核岗位
            </td>
          </tr>
        )}
      </tbody>
    </table>
  );
}
