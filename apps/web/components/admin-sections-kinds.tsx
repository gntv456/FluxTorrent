"use client";

/** 版块管理·介质类型 kinds 区（从 admin-sections.tsx 按域拆出）。 */
import { api } from "@/lib/api-client";
import { type SectionKindMeta, FIELD_INPUT_CLS } from "./admin-sections-shared";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

export function SectionKindsPanel(props: {
  kinds: SectionKindMeta[];
  kindCounts: Record<string, number>;
  newKind: string;
  setNewKind: (v: string) => void;
  newKindLabel: string;
  setNewKindLabel: (v: string) => void;
  busy: boolean;
  act: (fn: () => Promise<unknown>, okMsg: string) => void;
}) {
  const {
    kinds,
    kindCounts,
    newKind,
    setNewKind,
    newKindLabel,
    setNewKindLabel,
    busy,
    act,
  } = props;
  const at = useI18n().dict.adminSections;
  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-2 text-base font-bold">{at.kindsTitle}</h2>
      <p className="mb-3 text-xs text-sub">{at.kindsHint}</p>
      <div className="flex flex-wrap items-end gap-2">
        <input
          value={newKind}
          onChange={(e) => setNewKind(e.target.value)}
          placeholder={at.phKind}
          className={`w-48 ${FIELD_INPUT_CLS}`}
        />
        <input
          value={newKindLabel}
          onChange={(e) => setNewKindLabel(e.target.value)}
          placeholder={at.phKindLabel}
          className={`w-36 ${FIELD_INPUT_CLS}`}
        />
        <button
          disabled={busy || !newKind.trim() || !newKindLabel.trim()}
          className="baozi-button"
          onClick={() =>
            act(async () => {
              await api.post("/api/v1/admin/section-kinds", {
                kind: newKind,
                label: newKindLabel,
              });
              setNewKind("");
              setNewKindLabel("");
            }, at.kindCreated)
          }
        >
          {at.createKind}
        </button>
      </div>
      <table className="nexus-table mt-3 text-xs">
        <thead>
          <tr>
            <td className="colhead">{at.thKindKey}</td>
            <td className="colhead">{at.thKindLabel}</td>
            <td className="colhead">{at.thCount}</td>
            <td className="colhead">{at.thSort}</td>
            <td className="colhead text-right">{at.thAction}</td>
          </tr>
        </thead>
        <tbody>
          {kinds.map((k) => {
            const n = kindCounts[k.kind] ?? 0;
            return (
              <tr key={k.kind}>
                <td className="font-mono">{k.kind}</td>
                <td>{k.label}</td>
                <td className="num">
                  {n > 0 ? (
                    n
                  ) : (
                    <span className="text-sub">{at.zeroHint}</span>
                  )}
                </td>
                <td className="num">{k.sort}</td>
                <td className="text-right">
                  <button
                    className="cmgmt-act"
                    disabled={busy}
                    onClick={() => {
                      const nl = window.prompt(at.renameKindPrompt, k.label);
                      if (nl && nl !== k.label)
                        act(
                          () =>
                            api.put(`/api/v1/admin/section-kinds/${k.kind}`, {
                              kind: k.kind,
                              label: nl,
                              sort: k.sort,
                            }),
                          at.saved,
                        );
                    }}
                  >
                    {at.rename}
                  </button>
                  <button
                    className="cmgmt-act cmgmt-act--danger"
                    disabled={busy}
                    onClick={() => {
                      if (
                        window.confirm(
                          fmt(at.delKindConfirm, { name: k.label }),
                        )
                      )
                        act(
                          () =>
                            api.del(`/api/v1/admin/section-kinds/${k.kind}`),
                          at.deleted,
                        );
                    }}
                  >
                    {at.del}
                  </button>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </section>
  );
}
