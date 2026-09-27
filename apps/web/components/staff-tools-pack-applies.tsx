"use client";

/**
 * 站型包应用记录（G21）：回看每次 apply 的变更/时间/操作人，一键回滚到
 * 应用前快照（只允许最近一条）。数据由父面板注入，随 guard 的重载刷新。
 */

import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import type { PackApplyItem } from "./staff-tools-content-shared";

const APPLIES_API = "/api/v1/admin/site-type-packs/applies";

interface PackAppliesProps {
  rows: PackApplyItem[];
  busy: boolean;
  guard: (fn: () => Promise<void>, ok: string) => Promise<void>;
}

export function StaffPackAppliesPanel({ rows, busy, guard }: PackAppliesProps) {
  const { dict } = useI18n();
  const t = dict.stafftools;
  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-3 text-base font-bold text-ink">
        {t.packAppliesTitle}
      </h2>
      <p className="mb-2 text-xs text-sub">{t.packAppliesNote}</p>
      {rows.length === 0 ? (
        <p className="text-xs text-sub">{t.packAppliesEmpty}</p>
      ) : (
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead">#</td>
              <td className="colhead">{t.fldApplyTime}</td>
              <td className="colhead">{t.packTitle}</td>
              <td className="colhead">{t.packMode}</td>
              <td className="colhead">{t.fldApplyChanges}</td>
              <td className="colhead">{t.fldApplyActor}</td>
              <td className="colhead text-right">{dict.cmgmt.colActions}</td>
            </tr>
            {rows.map((r) => {
              const lines = (r.changes ?? []).map(
                (c) => `${c.key}: ${c.old} → ${c.new}`,
              );
              const head = fmt(t.packDiffHead, { n: lines.length });
              const detail = lines.length
                ? head +
                  lines.slice(0, 15).join("\n") +
                  (lines.length > 15 ? "\n…" : "")
                : t.packNoDiff;
              return (
                <tr key={r.id}>
                  <td className="num">{r.id}</td>
                  <td>{r.applied_at}</td>
                  <td>{r.pack_name}</td>
                  <td>{r.mode}</td>
                  <td title={detail}>
                    {fmt(t.packApplyChangesN, { n: lines.length })}
                  </td>
                  <td>{r.actor ?? ""}</td>
                  <td className="text-right">
                    {r.rolled_back_at ? (
                      <span className="text-xs text-sub">
                        {fmt(t.packRolledBackAt, { at: r.rolled_back_at })}
                      </span>
                    ) : (
                      <button
                        className="cmgmt-act"
                        disabled={busy}
                        onClick={() => {
                          const msg = `${fmt(t.packRollbackConfirm, {
                            name: r.pack_name,
                          })}\n\n${detail}`;
                          if (!window.confirm(msg)) return;
                          void guard(
                            async () => {
                              await api.post(`${APPLIES_API}/${r.id}/rollback`);
                            },
                            fmt(t.packRolledBack, { name: r.pack_name }),
                          );
                        }}
                      >
                        {t.btnRollback}
                      </button>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </section>
  );
}
