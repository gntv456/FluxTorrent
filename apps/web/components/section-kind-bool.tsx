"use client";

import { useI18n } from "@/i18n/client";

/**
 * 布尔维度控件（三态：是 / 否 / 未设置）——发布页、编辑页与后台筛选/批改
 * 共用同一实现。旧版发布/编辑页是二态 checkbox，只能表达「是 / 未填」，
 * 与后台按钮组口径分裂（同一维度在不同入口能填的值不同）。
 */

const CHIP_CLS =
  "inline-flex min-h-[30px] items-center rounded-full border px-3 text-xs";
const CHIP_ON = "border-sky bg-sky text-white";
const CHIP_OFF = "border-line text-sub hover:border-sky";

export function SectionKindBool({
  value,
  onChange,
}: {
  /** "true" / "false" / ""（未设置）——与 `sec_{kind}` 协议同形 */
  value: string;
  onChange: (v: string) => void;
}) {
  const { dict } = useI18n();
  const opts: [string, string][] = [
    ["true", dict.common.yes],
    ["false", dict.common.no],
  ];
  return (
    <span className="inline-flex gap-1">
      {opts.map(([v, label]) => (
        <button
          key={v}
          type="button"
          onClick={() => onChange(value === v ? "" : v)}
          className={`${CHIP_CLS} ${value === v ? CHIP_ON : CHIP_OFF}`}
        >
          {label}
        </button>
      ))}
    </span>
  );
}
