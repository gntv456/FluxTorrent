"use client";

import { INPUT_BRAND } from "@/lib/ui-classes";
import { SectionKindBool } from "@/components/section-kind-bool";

/** 六类型维度控件（发布表单 / 编辑弹层共用同一实现，防两处口径漂移）：
 *  select / multiselect / number / date / bool / text（0195 B2 六类型）。
 *  值统一以字符串承载：枚举存 dict_id、自由值存原文、多选逗号串
 *  （与 `sec_{kind}` 协议及 torrent_sections 存储同形）。
 *
 *  可达名：外层若是 `<label>` 包裹（编辑弹层），控件可继承标签文本；
 *  外层是表格 rowhead（发布表单）不构成编程关联，必须传 `ariaLabel`。 */

export interface SecKindOpt {
  id: number;
  name: string;
}

/** 多选复选框容器（边框盒 + 超高内滚）：选项多的维度（出品方 14 项）
 *  不会把行撑爆，观感仍是「一个维度的选项组」。宽度由调用方决定：
 *  fluid（网格格子里）撑满，否则限宽 2xl（详情/行内）。 */
const MULTI_BOX_CLS =
  "flex max-h-40 flex-wrap items-center gap-x-4 gap-y-1 "
  + "overflow-y-auto rounded-[var(--r-sm)] "
  + "border border-[var(--brand-line)] px-3 py-1.5";

export function SectionKindControl({
  type,
  value,
  onChange,
  opts,
  noneLabel,
  ariaLabel,
  inputCls,
  fluid,
}: {
  type: string;
  value: string;
  onChange: (v: string) => void;
  opts: SecKindOpt[];
  /** select 空项文案（「不选择」） */
  noneLabel: string;
  /** 控件可达名（rowhead 场景必传） */
  ariaLabel?: string;
  /** 输入框底盘类（缺省 INPUT_BRAND；编辑弹层传 FLD 对齐模态视觉） */
  inputCls?: string;
  /** 撑满容器（发布表单网格格子）：宽度交给外层，不再自带固定宽度 */
  fluid?: boolean;
}) {
  const cls = inputCls ?? INPUT_BRAND;
  // 固定宽 vs 撑满：网格里每格宽度不同，固定宽会让控件参差
  const w = (fixed: string) => (fluid ? "w-full min-w-0" : fixed);
  // 多选：逗号串 → id 集合；勾选状态切换后写回逗号串
  const multiIds = value
    .split(",")
    .map((s) => Number(s.trim()))
    .filter((n) => n > 0);
  switch (type) {
    case "multiselect":
      return (
        <span
          role="group"
          aria-label={ariaLabel}
          className={`${MULTI_BOX_CLS} ${fluid ? "w-full" : "max-w-2xl"}`}
        >
          {opts.map((o) => {
            const on = multiIds.includes(o.id);
            return (
              <label
                key={o.id}
                className="flex cursor-pointer items-center gap-1.5 text-sm"
              >
                <input
                  type="checkbox"
                  checked={on}
                  onChange={() => {
                    const next = on
                      ? multiIds.filter((x) => x !== o.id)
                      : [...multiIds, o.id];
                    onChange(next.join(","));
                  }}
                  aria-label={
                    ariaLabel ? `${ariaLabel} ${o.name}` : undefined
                  }
                  className="h-4 w-4 accent-[var(--brand-orange)]"
                />
                {o.name}
              </label>
            );
          })}
        </span>
      );
    case "select":
      return (
        <select
          value={value}
          onChange={(e) => onChange(e.target.value)}
          aria-label={ariaLabel}
          className={`${cls} ${w("w-52 max-w-full")}`}
        >
          <option value="">{noneLabel}</option>
          {opts.map((o) => (
            <option key={o.id} value={o.id}>
              {o.name}
            </option>
          ))}
        </select>
      );
    case "number":
      return (
        <input
          type="number"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          aria-label={ariaLabel}
          className={`${cls} ${w("w-28")}`}
        />
      );
    case "date":
      return (
        <input
          type="date"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          aria-label={ariaLabel}
          className={`${cls} ${w("w-40")}`}
        />
      );
    case "bool":
      return (
        <SectionKindBool
          value={value}
          onChange={onChange}
          ariaLabel={ariaLabel}
        />
      );
    default:
      return (
        <input
          value={value}
          onChange={(e) => onChange(e.target.value)}
          aria-label={ariaLabel}
          className={`${cls} ${w("w-64 max-w-full")}`}
        />
      );
  }
}
