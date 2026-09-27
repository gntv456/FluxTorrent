"use client";

import { INPUT_LG } from "@/lib/ui-classes";

/** 注册页动态字段渲染（0186）：从 register/page.tsx 拆出（300 行门禁）。 */

export interface RegField {
  key: string;
  label: string;
  type: string;
  required: boolean;
  options: { value: string; label?: string }[];
}

export function RegisterFields({
  fields,
  values,
  onChange,
}: {
  fields: RegField[];
  values: Record<string, unknown>;
  onChange: (p: Record<string, unknown>) => void;
}) {
  return (
    <>
      {fields.map((f) => (
        <label key={f.key} className="flex flex-col gap-1">
          <span className="text-sm text-sub">
            {f.label}
            {f.required ? " *" : ""}
          </span>
          {f.type === "select" ? (
            <select
              className={INPUT_LG}
              required={f.required}
              value={String(values[f.key] ?? "")}
              onChange={(e) =>
                onChange({
                  ...values,
                  [f.key]: e.target.value || null,
                })
              }
            >
              <option value="">—</option>
              {f.options.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label ?? o.value}
                </option>
              ))}
            </select>
          ) : f.type === "bool" ? (
            <input
              type="checkbox"
              checked={values[f.key] === true}
              onChange={(e) =>
                onChange({
                  ...values,
                  [f.key]: e.target.checked,
                })
              }
            />
          ) : (
            <input
              type={
                f.type === "number"
                  ? "number"
                  : f.type === "date"
                    ? "date"
                    : "text"
              }
              className={INPUT_LG}
              required={f.required}
              maxLength={f.type === "text" ? 500 : undefined}
              step={f.type === "number" ? 1 : undefined}
              value={String(values[f.key] ?? "")}
              onChange={(e) =>
                onChange({
                  ...values,
                  [f.key]:
                    f.type === "number"
                      ? e.target.value === ""
                        ? null
                        : Number(e.target.value)
                      : e.target.value || null,
                })
              }
            />
          )}
        </label>
      ))}
    </>
  );
}
