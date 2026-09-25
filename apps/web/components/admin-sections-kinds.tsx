"use client";

/** 版块管理·介质类型 kinds 区（从 admin-sections.tsx 按域拆出）。
 *  B2（0195）：六类型字段系统 —— 每维可选 field_type/必填/多值/启用，
 *  编辑走抽屉（原「改名 prompt」写不下这些字段）。 */
import { useState } from "react";
import { api } from "@/lib/api-client";
import {
  type SectionKindMeta,
  FIELD_INPUT_CLS,
  FIELD_TYPE_KEYS,
} from "./admin-sections-shared";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

/** 编辑抽屉外壳（长 className 提常量，守 ≤80 行宽门禁） */
const DRAWER_CLS =
  "mt-3 rounded-[var(--r-md)] border border-line "
  + "bg-[var(--surface-card)] p-3";

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
  const { dict } = useI18n();
  const at = dict.adminSections;
  const ft = at.fieldTypes;
  const [edit, setEdit] = useState<SectionKindMeta | null>(null);
  // 新建时的类型选择（存量语义：缺省 select）
  const [newType, setNewType] = useState("select");

  const typeLabel = (t?: string) =>
    (ft as Record<string, string>)[t ?? "select"] ?? t ?? "select";

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
        <select
          value={newType}
          onChange={(e) => setNewType(e.target.value)}
          className={FIELD_INPUT_CLS}
          aria-label={at.thFieldType}
        >
          {FIELD_TYPE_KEYS.map((k) => (
            <option key={k} value={k}>
              {typeLabel(k)}
            </option>
          ))}
        </select>
        <button
          disabled={busy || !newKind.trim() || !newKindLabel.trim()}
          className="baozi-button"
          onClick={() =>
            act(async () => {
              await api.post("/api/v1/admin/section-kinds", {
                kind: newKind,
                label: newKindLabel,
                field_type: newType,
              });
              setNewKind("");
              setNewKindLabel("");
              setNewType("select");
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
            <td className="colhead">{at.thFieldType}</td>
            <td className="colhead">{at.thRequired}</td>
            <td className="colhead">{at.thMultiple}</td>
            <td className="colhead">{at.thEnabled}</td>
            <td className="colhead">{at.thCount}</td>
            <td className="colhead">{at.thSort}</td>
            <td className="colhead text-right">{at.thAction}</td>
          </tr>
        </thead>
        <tbody>
          {kinds.map((k) => {
            const n = kindCounts[k.kind] ?? 0;
            const on = k.enabled !== false;
            return (
              <tr key={k.kind} className={on ? undefined : "opacity-50"}>
                <td className="font-mono">{k.kind}</td>
                <td>{k.label}</td>
                <td className="text-sub">
                  {typeLabel(k.field_type)}
                </td>
                <td className="text-center">{k.required ? "✓" : ""}</td>
                <td className="text-center">{k.multiple ? "✓" : ""}</td>
                <td className="text-center">{on ? "✓" : ""}</td>
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
                    onClick={() => setEdit(k)}
                  >
                    {at.edit}
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

      {/* 编辑抽屉：B2 六类型设置（字段类型只读——后端拒改，改类型会让存量值语义失效） */}
      {edit && (
        <div className={DRAWER_CLS}>
          <div className="mb-2 flex items-center justify-between">
            <b className="text-sm">
              {at.editKindTitle}：<span className="font-mono">{edit.kind}</span>
            </b>
            <button className="cmgmt-act" onClick={() => setEdit(null)}>
              {at.close}
            </button>
          </div>
          <div className="flex flex-wrap items-end gap-2 text-xs">
            <label className="flex flex-col gap-1">
              <span className="text-sub">{at.thKindLabel}</span>
              <input
                value={edit.label}
                onChange={(e) =>
                  setEdit({ ...edit, label: e.target.value })
                }
                className={`w-40 ${FIELD_INPUT_CLS}`}
              />
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-sub">{at.thFieldType}</span>
              <input
                value={typeLabel(edit.field_type)}
                readOnly
                className={`w-32 ${FIELD_INPUT_CLS} opacity-60`}
                title={at.fieldTypeLocked}
              />
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-sub">{at.thSort}</span>
              <input
                type="number"
                value={edit.sort}
                onChange={(e) =>
                  setEdit({ ...edit, sort: Number(e.target.value) })
                }
                className={`w-20 ${FIELD_INPUT_CLS}`}
              />
            </label>
            {(
              [
                ["required", at.thRequired],
                ["multiple", at.thMultiple],
                ["enabled", at.thEnabled],
              ] as const
            ).map(([key, label]) => (
              <label key={key} className="flex items-center gap-1 pb-2">
                <input
                  type="checkbox"
                  checked={Boolean(edit[key] ?? key === "enabled")}
                  onChange={(e) =>
                    setEdit({ ...edit, [key]: e.target.checked })
                  }
                />
                <span>{label}</span>
              </label>
            ))}
            <button
              className="baozi-button mb-0.5"
              disabled={busy || !edit.label.trim()}
              onClick={() =>
                act(async () => {
                  await api.put(
                    `/api/v1/admin/section-kinds/${edit.kind}`,
                    {
                      kind: edit.kind,
                      label: edit.label.trim(),
                      sort: edit.sort,
                      // field_type 原样回传（后端同值放行、异值拒收）
                      field_type: edit.field_type ?? "select",
                      required: Boolean(edit.required),
                      multiple: Boolean(edit.multiple),
                      enabled: edit.enabled !== false,
                    },
                  );
                  setEdit(null);
                }, at.saved)
              }
            >
              {at.saveKind}
            </button>
          </div>
          <p className="mt-2 text-xs text-sub">{at.fieldTypeLocked}</p>
        </div>
      )}
    </section>
  );
}
