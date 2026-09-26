"use client";

/**
 * 用户详情·自定义字段值（G4，2026-09-26）：运营侧查看/代改目标用户的字段值。
 *
 * 与本人入口（usercp 的 PUT /me/fields）的差异：
 *   · 看得到 private 与停用字段（排查「用户说填了却看不到」）
 *   · 停用字段只读——后端对停用字段直接 400，不给「填了没生效」的中间态
 *   · 只提交有改动的字段（null = 清空），校验与本人入口同源（crate::fields）
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { INPUT_MD } from "@/lib/ui-classes";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

interface FieldRow {
  key: string;
  label: string;
  type: string;
  required: boolean;
  visibility: string;
  options: { value: string; label?: string }[];
  enabled: boolean;
  value: unknown;
}

const SUBMIT_BTN =
  "min-h-[32px] rounded-full bg-sky px-4 text-xs font-bold text-white " +
  "disabled:opacity-50";

/** 值 → 控件文本（number/bool 等非字符串都折成可编辑字符串） */
function asText(v: unknown): string {
  if (v === null || v === undefined) return "";
  if (typeof v === "object") return Array.isArray(v) ? v.join(",") : "";
  return String(v);
}

export function AdminUserFieldsPanel({ userId }: { userId: number }) {
  const at = useI18n().dict.adminUsers;
  const [rows, setRows] = useState<FieldRow[] | null>(null);
  const [draft, setDraft] = useState<Record<string, unknown>>({});
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setRows(
        await api.get<FieldRow[]>(`/api/v1/admin/users/${userId}/fields`),
      );
      setDraft({});
    } catch {
      setRows([]);
    }
  }, [userId]);
  useEffect(() => {
    load();
  }, [load]);

  if (!rows || rows.length === 0) return null;
  const dirty = Object.keys(draft);
  const value = (r: FieldRow) => (r.key in draft ? draft[r.key] : r.value);
  const put = (key: string, v: unknown) =>
    setDraft((prev) => ({ ...prev, [key]: v }));

  /** 按类型分派控件；停用字段只读 */
  const control = (r: FieldRow) => {
    if (!r.enabled) {
      return <span className="text-xs text-sub">{asText(r.value) || "—"}</span>;
    }
    const v = value(r);
    if (r.type === "select") {
      return (
        <select
          value={asText(v)}
          onChange={(e) => put(r.key, e.target.value || null)}
          className={INPUT_MD}
        >
          <option value="">{at.fieldsBlank}</option>
          {r.options.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label ?? o.value}
            </option>
          ))}
        </select>
      );
    }
    if (r.type === "multiselect") {
      const on = Array.isArray(v) ? (v as string[]) : [];
      return (
        <div className="flex flex-wrap gap-1">
          {r.options.map((o) => {
            const hit = on.includes(o.value);
            return (
              <button
                key={o.value}
                type="button"
                onClick={() =>
                  put(
                    r.key,
                    hit ? on.filter((x) => x !== o.value) : [...on, o.value],
                  )
                }
                className={
                  "inline-flex min-h-[30px] items-center rounded-full " +
                  "border px-3 text-xs " +
                  (hit ? "border-sky bg-sky text-white" : "border-line")
                }
              >
                {o.label ?? o.value}
              </button>
            );
          })}
        </div>
      );
    }
    if (r.type === "bool") {
      return (
        <select
          value={v === true ? "true" : v === false ? "false" : ""}
          onChange={(e) =>
            put(r.key, e.target.value === "" ? null : e.target.value === "true")
          }
          className={INPUT_MD}
        >
          <option value="">{at.fieldsBlank}</option>
          <option value="true">{at.fieldsYes}</option>
          <option value="false">{at.fieldsNo}</option>
        </select>
      );
    }
    const typ =
      r.type === "number" ? "number" : r.type === "date" ? "date" : "text";
    return (
      <input
        type={typ}
        value={asText(v)}
        onChange={(e) =>
          put(
            r.key,
            e.target.value === ""
              ? null
              : r.type === "number"
                ? Number(e.target.value)
                : e.target.value,
          )
        }
        className={INPUT_MD}
      />
    );
  };

  const save = async () => {
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.put<{ saved: number }>(
        `/api/v1/admin/users/${userId}/fields`,
        { values: draft },
      );
      setMsg(fmt(at.fieldsSaved, { n: r.saved }));
      await load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : at.opFail);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="rounded-[var(--r-md)] border border-line p-3">
      <p className="mb-2 text-xs font-bold text-sub">{at.fieldsTitle}</p>
      <div className="grid grid-cols-1 gap-2 md:grid-cols-3">
        {rows.map((r) => (
          <label key={r.key} className="flex flex-col gap-1 text-xs">
            <span className="text-sub">
              {r.label}
              {!r.enabled && (
                <span className="ml-1 text-danger">{at.fieldsOff}</span>
              )}
              {r.visibility === "private" && (
                <span className="ml-1 text-sub">{at.fieldsPrivate}</span>
              )}
            </span>
            {control(r)}
          </label>
        ))}
      </div>
      <div className="mt-2 flex items-center gap-2">
        <button
          className={SUBMIT_BTN}
          disabled={busy || dirty.length === 0}
          onClick={save}
        >
          {fmt(at.fieldsSave, { n: dirty.length })}
        </button>
        <span className="text-xs text-sub">{at.fieldsHint}</span>
        {msg && <span className="text-xs text-mint">{msg}</span>}
      </div>
    </div>
  );
}
