"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";

interface Row {
  id: number;
  name: string;
}

/**
 * 官种制作组名单的多选控件（0449）。
 *
 * 设置项存的是逗号分隔的字典 ID，但让站长手抄 ID 不现实：这里从
 * 「维度字典 → 制作组」拉词表渲染勾选，写回仍是 CSV 字符串——保存路径与
 * 其它字段完全一致，不新增后端口。
 *
 * 静默回落：词表为空或接口不可用时退回纯文本输入，站长仍可手填 ID，
 * 于是 0449 上线前后都能用。
 */
export function OfficialTeamsPicker({
  value,
  disabled,
  onChange,
}: {
  value: string;
  disabled: boolean;
  onChange: (v: string) => void;
}) {
  const [rows, setRows] = useState<Row[] | null>(null);

  useEffect(() => {
    let alive = true;
    api
      .get<Row[]>("/api/v1/admin/section-dict?kind=team")
      .then((r) => {
        if (alive) setRows(Array.isArray(r) ? r : []);
      })
      .catch(() => {
        if (alive) setRows([]);
      });
    return () => {
      alive = false;
    };
  }, []);

  const picked = new Set(
    value
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean),
  );

  const toggle = (id: number) => {
    const key = String(id);
    const next = new Set(picked);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    onChange([...next].sort((a, b) => Number(a) - Number(b)).join(","));
  };

  if (rows === null) return <p className="text-xs text-sub">…</p>;

  if (rows.length === 0) {
    return (
      <input
        type="text"
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
        className="min-h-[44px] w-full rounded-[var(--r-sm)] border
          border-line bg-[var(--surface-card)] px-3 text-sm text-ink"
      />
    );
  }

  return (
    <div
      className="flex max-h-64 flex-wrap gap-1.5 overflow-y-auto
        rounded-[var(--r-sm)] border border-line p-2"
    >
      {rows.map((r) => {
        const on = picked.has(String(r.id));
        return (
          <button
            key={r.id}
            type="button"
            disabled={disabled}
            aria-pressed={on}
            onClick={() => toggle(r.id)}
            className={
              "rounded-full border px-3 py-1 text-xs " +
              (on
                ? "border-line font-semibold text-ink " +
                  "bg-[var(--sky-soft)]"
                : "border-line text-sub hover:text-ink")
            }
          >
            {r.name}
          </button>
        );
      })}
    </div>
  );
}
