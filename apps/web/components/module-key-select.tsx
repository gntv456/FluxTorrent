"use client";

import { useEffect, useState } from "react";
import { useI18n } from "@/i18n/client";

const SEL =
  "min-h-[32px] rounded-[var(--r-sm)] border border-line bg-transparent px-2";

/** 「自建产物 → 挂模块键」选择器（0199）。
 *  选项来自 site-profile.modules（就是后台「模块开关」那张表的键集），
 *  已关闭的键在选项里标注出来，避免站长挂到一个正关着的键上却以为是坏了。
 *  空值 = 不挂，产物恒可见。 */
export function ModuleKeySelect({
  value,
  onChange,
}: {
  value: string | null | undefined;
  onChange: (k: string | null) => void;
}) {
  const { dict } = useI18n();
  const t = dict.moduleBind;
  const [mods, setMods] = useState<Record<string, boolean>>({});
  useEffect(() => {
    fetch("/api/v1/site-profile")
      .then((r) => r.json())
      .then((b: { data?: { modules?: Record<string, boolean> } }) =>
        setMods(b?.data?.modules ?? {}),
      )
      .catch(() => setMods({}));
  }, []);
  const keys = Object.keys(mods).sort();
  if (keys.length === 0) return null;
  return (
    <label className="flex flex-col gap-1 text-sm">
      <span>{t.label}</span>
      <select
        className={SEL}
        value={value ?? ""}
        onChange={(e) => onChange(e.target.value || null)}
      >
        <option value="">{t.none}</option>
        {keys.map((k) => (
          <option key={k} value={k}>
            {mods[k] === false ? `${t.offPrefix} ${k}` : k}
          </option>
        ))}
      </select>
      <span className="text-xs text-sub">{t.hint}</span>
    </label>
  );
}
