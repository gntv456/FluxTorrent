"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { Row } from "@/components/usercp-row";

/** 自定义字段面板（0186 一审 R4.1）：站长在后台定义的字段（文本/数字/
 *  单选/多选/日期/开关），用户在此填写；保存走 PUT /me/fields。
 *  public 字段会展示在公开档案；private 仅本人与管理组可见。 */

interface FieldDef {
  key: string;
  label: string;
  type: "text" | "number" | "select" | "multiselect" | "date" | "bool";
  required: boolean;
  options: { value: string; label?: string }[];
  value?: unknown;
}

const FLD =
  "w-full max-w-md rounded-[var(--r-sm)] border border-line bg-[var(--baozi-bg-input)] px-2 py-1 text-sm text-ink";

export function UserFieldsTab() {
  const { dict } = useI18n();
  const t = dict.usercp.fields;
  const [defs, setDefs] = useState<FieldDef[] | null>(null);
  const [vals, setVals] = useState<Record<string, unknown>>({});
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api
      .get<FieldDef[]>("/api/v1/me/fields")
      .then((rows) => {
        setDefs(rows);
        const init: Record<string, unknown> = {};
        for (const d of rows) init[d.key] = d.value ?? null;
        setVals(init);
      })
      .catch(() => setDefs([]));
  }, []);

  if (defs === null)
    return <p className="py-4 text-sm text-sub">{dict.my.loading}</p>;
  if (defs.length === 0)
    return (
      <p className="py-4 text-sm text-sub">{t.empty}</p>
    );

  const setV = (k: string, v: unknown) =>
    setVals((prev) => ({ ...prev, [k]: v }));

  async function save() {
    if (busy) return;
    setBusy(true);
    setMsg(null);
    try {
      await api.put("/api/v1/me/fields", { values: vals });
      setMsg(t.saved);
    } catch (e) {
      setMsg(
        e instanceof Error && e.message
          ? e.message
          : dict.common.loadFailed,
      );
    } finally {
      setBusy(false);
    }
  }

  const input = (d: FieldDef) => {
    const v = vals[d.key];
    switch (d.type) {
      case "number":
        return (
          <input
            type="number"
            className={FLD}
            value={v === null || v === undefined ? "" : String(v)}
            onChange={(e) =>
              setV(
                d.key,
                e.target.value === ""
                  ? null
                  : Number(e.target.value),
              )
            }
          />
        );
      case "date":
        return (
          <input
            type="date"
            className={FLD}
            value={v === null || v === undefined ? "" : String(v)}
            onChange={(e) => setV(d.key, e.target.value || null)}
          />
        );
      case "bool":
        return (
          <label>
            <input
              type="checkbox"
              checked={v === true}
              onChange={(e) => setV(d.key, e.target.checked)}
            />
            {t.boolOn}
          </label>
        );
      case "select":
        return (
          <select
            className={FLD}
            value={v === null || v === undefined ? "" : String(v)}
            onChange={(e) => setV(d.key, e.target.value || null)}
          >
            <option value="">{dict.upload.gradeNone}</option>
            {d.options.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label ?? o.value}
              </option>
            ))}
          </select>
        );
      case "multiselect": {
        const arr = Array.isArray(v) ? (v as string[]) : [];
        return (
          <div className="flex max-w-md flex-wrap gap-x-4 gap-y-1">
            {d.options.map((o) => (
              <label key={o.value}>
                <input
                  type="checkbox"
                  checked={arr.includes(o.value)}
                  onChange={(e) =>
                    setV(
                      d.key,
                      e.target.checked
                        ? [...arr, o.value].slice(0, 20)
                        : arr.filter((x) => x !== o.value),
                    )
                  }
                />
                {o.label ?? o.value}
              </label>
            ))}
          </div>
        );
      }
      default:
        return (
          <input
            type="text"
            className={FLD}
            value={v === null || v === undefined ? "" : String(v)}
            onChange={(e) => setV(d.key, e.target.value || null)}
          />
        );
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <table className="nexus-table nexus-form">
        <tbody>
          {defs.map((d) => (
            <Row key={d.key} head={`${d.label}${d.required ? " *" : ""}`}>
              {input(d)}
            </Row>
          ))}
        </tbody>
      </table>
      <div className="flex items-center gap-3">
        <button
          type="button"
          className="baozi-button"
          onClick={save}
          disabled={busy}
        >
          {busy ? dict.my.loading : dict.usercp.saveBtn}
        </button>
        {msg && <span className="text-sm text-sub">{msg}</span>}
      </div>
    </div>
  );
}
