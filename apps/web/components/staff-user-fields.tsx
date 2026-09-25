"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { ModuleKeySelect } from "@/components/module-key-select";

/** 用户自定义字段管理面板（0186 一审 R4.1 的「方向盘」，三审 B-1）：
 *  站长定义/启停/删除字段（六类型 + public/private + 注册页展示位）。
 *  后端 /admin/user-fields*；用户填写在 usercp「自定义字段」tab。 */

interface FieldDef {
  key: string;
  label: string;
  type: "text" | "number" | "select" | "multiselect" | "date" | "bool";
  required: boolean;
  visibility: "public" | "private";
  show_on_register: boolean;
  options: { value: string; label?: string }[];
  sort: number;
  enabled: boolean;
  /** 挂载模块键（0199）。PUT 全量覆盖，任何手拼 body 的地方都必须带上，
   *  否则「启用/停用」开关会把挂载悄悄清空。 */
  module_key: string | null;
  filled: number;
}

const TYPES: { v: FieldDef["type"]; zh: string; en: string }[] = [
  { v: "text", zh: "文本", en: "Text" },
  { v: "number", zh: "数字", en: "Number" },
  { v: "select", zh: "单选", en: "Select" },
  { v: "multiselect", zh: "多选", en: "Multi-select" },
  { v: "date", zh: "日期", en: "Date" },
  { v: "bool", zh: "开关", en: "Toggle" },
];

const FLD =
  "min-h-[38px] w-full rounded-[var(--r-sm)] border border-line bg-cloud px-2 text-sm outline-none focus:border-sky";

interface Draft {
  key: string;
  label: string;
  type: FieldDef["type"];
  required: boolean;
  visibility: "public" | "private";
  show_on_register: boolean;
  options: string;
  sort: number;
  enabled: boolean;
  module_key: string | null;
}

const emptyDraft = (): Draft => ({
  key: "",
  label: "",
  type: "text",
  required: false,
  visibility: "public",
  show_on_register: false,
  options: "",
  sort: 100,
  enabled: true,
  module_key: null,
});

export function StaffUserFieldsPanel({
  flash,
}: {
  flash: (m: string) => void;
}) {
  const { dict, locale } = useI18n();
  const zh = locale !== "en";
  const [rows, setRows] = useState<FieldDef[] | null>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [editKey, setEditKey] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    api
      .get<FieldDef[]>("/api/v1/admin/user-fields")
      .then(setRows)
      .catch(() => setRows([]));
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  function errText(e: unknown) {
    return e instanceof ApiError ? e.message : dict.common.networkError;
  }

  function payload(d: Draft) {
    // 展开 Draft 而不是逐字段抄：PUT 是全量覆盖，漏抄一个就等于静默清空那一个
    const { key, options, ...rest } = d;
    return {
      ...rest,
      options: options
        .split(/[,，]/)
        .map((x) => x.trim())
        .filter(Boolean)
        .map((v) => ({ value: v })),
    };
  }

  async function save() {
    if (!draft || busy) return;
    setBusy(true);
    try {
      if (editKey) {
        await api.put(`/api/v1/admin/user-fields/${editKey}`, payload(draft));
      } else {
        await api.post(
          `/api/v1/admin/user-fields/${draft.key.trim().toLowerCase()}`,
          payload(draft),
        );
      }
      flash(dict.adminUserFields.saved);
      setDraft(null);
      setEditKey(null);
      load();
    } catch (e) {
      flash(errText(e));
    } finally {
      setBusy(false);
    }
  }

  async function toggleEnabled(d: FieldDef) {
    const { key, filled, ...body } = d;
    try {
      await api.put(`/api/v1/admin/user-fields/${d.key}`, {
        ...body,
        enabled: !d.enabled,
      });
      load();
    } catch (e) {
      flash(errText(e));
    }
  }

  async function remove(d: FieldDef) {
    const need = d.filled > 0;
    const confirm = need
      ? window.confirm(
          zh
            ? `该字段已有 ${d.filled} 个用户填写，确认连带删除全部值？`
            : `${d.filled} user(s) filled this field. Delete all values too?`,
        )
      : true;
    if (!confirm) return;
    try {
      await api.post(`/api/v1/admin/user-fields/${d.key}/delete`, {
        confirm_drop_values: need,
      });
      flash(dict.adminUserFields.deleted);
      load();
    } catch (e) {
      flash(errText(e));
    }
  }

  const editForm = draft && (
    <div className="mb-4 grid gap-2 rounded-[var(--r-md)] border border-line p-3 md:grid-cols-2">
      {!editKey && (
        <label className="text-xs text-sub">
          key（小写字母/数字/下划线）
          <input
            className={FLD}
            value={draft.key}
            onChange={(e) => setDraft({ ...draft, key: e.target.value })}
          />
        </label>
      )}
      <label className="text-xs text-sub">
        {dict.adminUserFields.label}
        <input
          className={FLD}
          value={draft.label}
          onChange={(e) => setDraft({ ...draft, label: e.target.value })}
        />
      </label>
      <label className="text-xs text-sub">
        {dict.adminUserFields.type}
        <select
          className={FLD}
          value={draft.type}
          onChange={(e) =>
            setDraft({ ...draft, type: e.target.value as Draft["type"] })
          }
        >
          {TYPES.map((t) => (
            <option key={t.v} value={t.v}>
              {zh ? t.zh : t.en}
            </option>
          ))}
        </select>
      </label>
      {(draft.type === "select" || draft.type === "multiselect") && (
        <label className="text-xs text-sub md:col-span-2">
          {dict.adminUserFields.optionsHint}
          <input
            className={FLD}
            value={draft.options}
            onChange={(e) => setDraft({ ...draft, options: e.target.value })}
            placeholder="电影, 音乐, 游戏"
          />
        </label>
      )}
      <label className="flex items-center gap-2 text-xs text-sub">
        <input
          type="checkbox"
          checked={draft.required}
          onChange={(e) => setDraft({ ...draft, required: e.target.checked })}
        />
        {dict.adminUserFields.required}
      </label>
      <label className="flex items-center gap-2 text-xs text-sub">
        <input
          type="checkbox"
          checked={draft.visibility === "public"}
          onChange={(e) =>
            setDraft({
              ...draft,
              visibility: e.target.checked ? "public" : "private",
            })
          }
        />
        {dict.adminUserFields.publicVis}
      </label>
      <label className="flex items-center gap-2 text-xs text-sub">
        <input
          type="checkbox"
          checked={draft.show_on_register}
          onChange={(e) =>
            setDraft({ ...draft, show_on_register: e.target.checked })
          }
        />
        {dict.adminUserFields.onRegister}
      </label>
      <label className="flex items-center gap-2 text-xs text-sub">
        <input
          type="checkbox"
          checked={draft.enabled}
          onChange={(e) => setDraft({ ...draft, enabled: e.target.checked })}
        />
        {dict.adminUserFields.enabled}
      </label>
      <div className="md:col-span-2">
        <ModuleKeySelect
          value={draft.module_key}
          onChange={(k) => setDraft({ ...draft, module_key: k })}
        />
      </div>
      <div className="flex gap-2 md:col-span-2">
        <button
          type="button"
          className="rounded-full bg-sky px-4 py-1.5 text-sm font-bold text-white disabled:opacity-50"
          onClick={save}
          disabled={busy || !draft.label.trim()}
        >
          {dict.usercp.saveBtn}
        </button>
        <button
          type="button"
          className="rounded-full border border-line px-4 py-1.5 text-sm"
          onClick={() => {
            setDraft(null);
            setEditKey(null);
          }}
        >
          {dict.common.cancel}
        </button>
      </div>
    </div>
  );

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <p className="text-xs text-sub">{dict.adminUserFields.hint}</p>
        {!draft && (
          <button
            type="button"
            className="rounded-full border border-line px-4 py-1.5 text-sm font-bold"
            onClick={() => setDraft(emptyDraft())}
          >
            + {dict.adminUserFields.add}
          </button>
        )}
      </div>
      {editForm}
      <table className="nexus-table">
        <thead>
          <tr>
            <th>key</th>
            <th>{dict.adminUserFields.label}</th>
            <th>{dict.adminUserFields.type}</th>
            <th>{dict.adminUserFields.flags}</th>
            <th>{dict.adminUserFields.filled}</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {(rows ?? []).map((d) => (
            <tr key={d.key}>
              <td className="num text-xs">{d.key}</td>
              <td>{d.label}</td>
              <td className="text-xs">{d.type}</td>
              <td className="text-xs text-sub">
                {[
                  d.required ? dict.adminUserFields.required : null,
                  d.visibility === "public"
                    ? dict.adminUserFields.publicVis
                    : dict.adminUserFields.privateVis,
                  d.show_on_register ? dict.adminUserFields.onRegister : null,
                  d.enabled ? null : dict.adminUserFields.disabled,
                ]
                  .filter(Boolean)
                  .join(" · ")}
              </td>
              <td className="num text-xs">{d.filled}</td>
              <td className="flex gap-2 text-xs">
                <button
                  type="button"
                  className="underline"
                  onClick={() => {
                    const { filled, ...def } = d;
                    setEditKey(d.key);
                    setDraft({
                      ...def,
                      options: d.options.map((o) => o.value).join(","),
                    });
                  }}
                >
                  {dict.torrents.edit}
                </button>
                <button
                  type="button"
                  className="underline"
                  onClick={() => toggleEnabled(d)}
                >
                  {d.enabled
                    ? dict.adminUserFields.disabled
                    : dict.adminUserFields.enabled}
                </button>
                <button
                  type="button"
                  className="underline text-coral"
                  onClick={() => remove(d)}
                >
                  {dict.torrents.delete}
                </button>
              </td>
            </tr>
          ))}
          {rows !== null && rows.length === 0 && (
            <tr>
              <td colSpan={6} className="text-sub">
                {dict.adminUserFields.empty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
