"use client";

/**
 * 后台种子管理·批量改维度（G5，2026-09-26）。
 *
 * 维度是站长自建的，批量入口**一次只改一个**：批量选择里各种子的既有维度
 * 各不相同，多维混提极易误清。载荷与单条编辑口同源（`sections` 对象，
 * 详见 `publish_http::upload_sections` 六类型格式），后端按**部分更新**
 * 处理——未给的维度保持原值；「清空」是显式按钮（发空值）而非默认行为。
 */

import { useState } from "react";
import { INPUT_CARD } from "@/lib/ui-classes";
import { useI18n } from "@/i18n/client";
import { SectionKindBool } from "@/components/section-kind-bool";
import type { SectionDictRow, SectionKindMeta } from "./admin-torrents-dims";

const CHIP_ON = "bg-sky text-white border-sky";
const CHIP_OFF = "border-line text-sub hover:border-sky";
const CHIP_CLS =
  "inline-flex min-h-[32px] items-center rounded-full border px-3 text-xs";

/** 按 field_type 组装单维度载荷；空值形态 = 后端「清空该维度」 */
function payload(k: SectionKindMeta, raw: string): Record<string, unknown> {
  const ft = k.field_type;
  if (ft === "select" || ft === "multiselect") {
    return { dict_ids: raw.split(",").filter(Boolean).map(Number) };
  }
  if (ft === "number") return raw ? { number: Number(raw) } : {};
  if (ft === "date") return raw ? { date: raw } : {};
  if (ft === "bool") return raw ? { bool: raw === "true" } : {};
  return { text: raw };
}

/** 值控件：按 field_type 分派（形态与筛选面板同款，便于对照） */
function ValueControl({
  k,
  opts,
  raw,
  setRaw,
}: {
  k: SectionKindMeta;
  opts: SectionDictRow[];
  raw: string;
  setRaw: (v: string) => void;
}) {
  const ft = k.field_type;
  if (ft === "select" || ft === "multiselect") {
    const on = raw.split(",").filter(Boolean);
    return (
      <div className="flex flex-wrap gap-1">
        {opts.map((o) => {
          const hit = on.includes(String(o.id));
          return (
            <button
              key={o.id}
              type="button"
              onClick={() => {
                const next = hit
                  ? on.filter((x) => x !== String(o.id))
                  : [...on, String(o.id)];
                setRaw(next.join(","));
              }}
              className={`${CHIP_CLS} ${hit ? CHIP_ON : CHIP_OFF}`}
            >
              {o.name}
            </button>
          );
        })}
      </div>
    );
  }
  if (ft === "bool") {
    return <SectionKindBool value={raw} onChange={setRaw} />;
  }
  const typ = ft === "date" ? "date" : ft === "number" ? "number" : "text";
  return (
    <input
      type={typ}
      value={raw}
      onChange={(e) => setRaw(e.target.value)}
      placeholder={k.label}
      className={`${INPUT_CARD} w-44 text-xs`}
    />
  );
}

export function DimSetPanel({
  kinds,
  dict,
  busy,
  onApply,
}: {
  kinds: SectionKindMeta[];
  dict: Record<string, SectionDictRow[]>;
  busy: boolean;
  onApply: (sections: Record<string, unknown>) => void;
}) {
  const at = useI18n().dict.adminTorrents;
  const [kind, setKind] = useState("");
  const [raw, setRaw] = useState("");
  if (kinds.length === 0) return null;
  const k = kinds.find((x) => x.kind === kind);
  const pick = (v: string) => {
    setKind(v);
    setRaw("");
  };
  return (
    <section className="baozi-panel flex flex-wrap items-center gap-2 p-3">
      <span className="text-xs font-bold text-sub">{at.dimSetTitle}</span>
      <select
        value={kind}
        onChange={(e) => pick(e.target.value)}
        className={`${INPUT_CARD} text-xs`}
      >
        <option value="">{at.dimSetKind}</option>
        {kinds.map((x) => (
          <option key={x.kind} value={x.kind}>
            {x.label}
          </option>
        ))}
      </select>
      {k && (
        <ValueControl
          k={k}
          opts={dict[k.kind] ?? []}
          raw={raw}
          setRaw={setRaw}
        />
      )}
      <button
        disabled={busy || !k || raw === ""}
        onClick={() => k && onApply({ [k.kind]: payload(k, raw) })}
        className="min-h-[32px] rounded-full bg-sky px-4 text-xs font-bold
         text-white disabled:opacity-50"
      >
        {at.dimSetApply}
      </button>
      <button
        disabled={busy || !k}
        onClick={() => k && onApply({ [k.kind]: payload(k, "") })}
        className="min-h-[32px] rounded-full border border-line px-4 text-xs
         font-bold disabled:opacity-50"
      >
        {at.dimSetClear}
      </button>
      <span className="text-xs text-sub">{at.dimSetHint}</span>
    </section>
  );
}
