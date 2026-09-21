"use client";

import { api } from "@/lib/api-client";
import { BTN_SM_BOLD } from "@/lib/ui-classes";
import { useI18n } from "@/i18n/client";
import type { RuleItem } from "./staff-tools-content-shared";

/** 内容管理·站点规则 rules tab（从 staff-tools-content.tsx 按域拆出）。 */
export interface RuleEdit {
  id: number | null;
  title: string;
  body: string;
}

export function ContentRulesTab(props: {
  rules: RuleItem[];
  ruleEdit: RuleEdit;
  setRuleEdit: React.Dispatch<React.SetStateAction<RuleEdit>>;
  busy: boolean;
  guard: (fn: () => Promise<void>, ok: string) => Promise<void>;
}) {
  const { rules, ruleEdit, setRuleEdit, busy, guard } = props;
  const { dict } = useI18n();
  const t = dict.stafftools;
  return (
    <>
      <section className="baozi-panel p-4">
        <h2 className="mb-3 text-base font-bold text-ink">
          {ruleEdit.id === null ? t.ruleNew : t.ruleEdit}
        </h2>
        <div className="cmgmt-form">
          <label>
            {dict.cmgmt.fldTitle}
            <input
              value={ruleEdit.title}
              onChange={(e) =>
                setRuleEdit({ ...ruleEdit, title: e.target.value })
              }
            />
          </label>
          <label>
            {dict.cmgmt.fldBody}
            <textarea
              rows={5}
              value={ruleEdit.body}
              onChange={(e) =>
                setRuleEdit({ ...ruleEdit, body: e.target.value })
              }
            />
          </label>
          <div className="flex gap-2">
            <button
              className="baozi-button"
              disabled={busy || !ruleEdit.title.trim()}
              onClick={() =>
                guard(async () => {
                  if (ruleEdit.id === null)
                    await api.post("/api/v1/admin/rules", {
                      title: ruleEdit.title,
                      body: ruleEdit.body,
                    });
                  else
                    await api.put(`/api/v1/admin/rules/${ruleEdit.id}`, {
                      title: ruleEdit.title,
                      body: ruleEdit.body,
                    });
                  setRuleEdit({ id: null, title: "", body: "" });
                }, t.saved)
              }
            >
              {t.btnSave}
            </button>
            {ruleEdit.id !== null && (
              <button
                className={BTN_SM_BOLD}
                onClick={() => setRuleEdit({ id: null, title: "", body: "" })}
              >
                {dict.cmgmt.btnCancel}
              </button>
            )}
          </div>
        </div>
      </section>
      <table className="nexus-table">
        <tbody>
          <tr>
            <td className="colhead">{dict.cmgmt.fldTitle}</td>
            <td className="colhead text-right">{dict.cmgmt.colActions}</td>
          </tr>
          {rules.map((r) => (
            <tr key={r.id}>
              <td>{r.title}</td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  onClick={() =>
                    setRuleEdit({ id: r.id, title: r.title, body: r.body })
                  }
                >
                  {dict.cmgmt.btnEdit}
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  onClick={() =>
                    guard(async () => {
                      await api.del(`/api/v1/admin/rules/${r.id}`);
                    }, t.deleted)
                  }
                >
                  {dict.cmgmt.btnDelete}
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}
