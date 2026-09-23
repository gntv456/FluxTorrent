"use client";

/**
 * 考核岗位·列表子面板（从 components/admin-exams.tsx 按域拆出）：
 * 岗位表（底薪、达标门槛、加成档位、登记数）+ 编辑回填与删除。
 * 指标目录与换算工具拆至 ./admin-exams-shared.ts。
 */

import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
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
  const { dict } = useI18n();
  const t = dict.adminExams;
  const METRIC_OPTIONS = metricOptions(currency, dict.adminExams.metrics);
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
          <td className="colhead">{t.colPos}</td>
          <td className="colhead">{t.colBasePay}</td>
          <td className="colhead">{t.colGates}</td>
          <td className="colhead">{t.colBonus}</td>
          <td className="colhead">{t.colAssigned}</td>
          <td className="colhead text-right">{t.colActions}</td>
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
            .join(t.gateJoin);
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
              <td className="max-w-[320px] truncate">{reqs || t.noGates}</td>
              <td className="num">
                {rules.percent_per_step
                  ? fmt(t.bonusRule, {
                      n: rules.months_per_step ?? 3,
                      p: rules.percent_per_step,
                    })
                  : "—"}
              </td>
              <td className="num">{r.assigned_count ?? "—"}</td>
              <td className="text-right">
                <button className="cmgmt-act" onClick={() => startEdit(r)}>
                  {t.editBtn}
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  disabled={busy}
                  onClick={async () => {
                    try {
                      await api.del(`/api/v1/admin/jixiao-types/${r.id}`);
                      flash(t.deleted);
                      await load();
                    } catch (e) {
                      flash(e instanceof ApiError ? e.message : t.delFail);
                    }
                  }}
                >
                  {t.delBtn}
                </button>
              </td>
            </tr>
          );
        })}
        {rows.length === 0 && (
          <tr>
            <td colSpan={7} className="py-6 text-center text-sub">
              {t.posEmpty}
            </td>
          </tr>
        )}
      </tbody>
    </table>
  );
}
