"use client";

/**
 * 后台种子管理·多维筛选（B3，2026-09-25）。
 *
 * 前台列表页早就能按站长自建维度筛（六类型全量），后台却不认这些参数
 * —— 上下游不一致。本组件把同一套 `sec_{kind}` 协议接到批量工作台上；
 * 谓词由后端与前台**共用同一实现**（`torrent_http::parse_section_params`
 * + `torrents::section_pred::section_where`），不复制语义。
 *
 * 六类型各自的输入形态：
 *   · select / multiselect → 字典项多选（重复 `sec_{kind}=id`，逗号串存储）
 *   · bool                 → 是 / 否 二选一
 *   · number / date        → 上下界（`_min` / `_max`，含端点）
 *   · text                 → 关键词输入（子串匹配）
 */

import { useI18n } from "@/i18n/client";

export interface SectionDictRow {
  id: number;
  kind: string;
  name: string;
  sort: number;
}
export interface SectionKindMeta {
  kind: string;
  label: string;
  field_type: string;
  /** B2（0195）起：站长可停用维度（停用后发布表单不再出现） */
  enabled?: boolean;
}

/** 筛选值集合：键 = `sec_{kind}` / `sec_{kind}_min` / `sec_{kind}_max` */
export type ValueMap = Record<string, string>;

/** 值以逗号串存储（与 URL / 后端解析一致）：勾选即追加，取消即移除 */
function toggleCsv(cur: string, v: string): string {
  const set = new Set(cur.split(",").filter(Boolean));
  if (set.has(v)) set.delete(v);
  else set.add(v);
  return [...set].join(",");
}

const CHIP_ON = "bg-sky text-white border-sky";
const CHIP_OFF = "border-line text-sub hover:border-sky";
const CHIP_CLS =
  "inline-flex min-h-[30px] items-center rounded-full border px-3 text-xs";
const IN_CLS =
  "min-h-[34px] rounded-[var(--r-sm)] border border-line " +
  "bg-[var(--surface-card)] px-2 text-xs";
/** 维度名标签宽度统一，让各类型控件左边缘对齐 */
const LBL_CLS = "w-24 shrink-0 text-xs text-sub";

type Setter = (key: string, value: string) => void;

/** 枚举 / 多选：字典项多选 chips */
function EnumRow({
  k,
  opts,
  values,
  onChange,
}: {
  k: SectionKindMeta;
  opts: SectionDictRow[];
  values: ValueMap;
  onChange: Setter;
}) {
  const base = `sec_${k.kind}`;
  const cur = values[base] ?? "";
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className={LBL_CLS}>{k.label}</span>
      <div className="flex flex-wrap gap-1">
        {opts.map((d) => {
          const on = cur.split(",").includes(String(d.id));
          return (
            <button
              key={d.id}
              type="button"
              onClick={() => onChange(base, toggleCsv(cur, String(d.id)))}
              className={`${CHIP_CLS} ${on ? CHIP_ON : CHIP_OFF}`}
            >
              {d.name}
            </button>
          );
        })}
      </div>
    </div>
  );
}

/** 布尔：二选一（再点一次取消） */
function BoolRow({
  k,
  values,
  onChange,
  yes,
  no,
}: {
  k: SectionKindMeta;
  values: ValueMap;
  onChange: Setter;
  yes: string;
  no: string;
}) {
  const base = `sec_${k.kind}`;
  const cur = values[base] ?? "";
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className={LBL_CLS}>{k.label}</span>
      <div className="flex gap-1">
        {(
          [
            ["true", yes],
            ["false", no],
          ] as [string, string][]
        ).map(([v, label]) => (
          <button
            key={v}
            type="button"
            onClick={() => onChange(base, cur === v ? "" : v)}
            className={`${CHIP_CLS} ${cur === v ? CHIP_ON : CHIP_OFF}`}
          >
            {label}
          </button>
        ))}
      </div>
    </div>
  );
}

/** 数字 / 日期：上下界输入（含端点；留空即不限） */
function RangeRow({
  k,
  values,
  onChange,
}: {
  k: SectionKindMeta;
  values: ValueMap;
  onChange: Setter;
}) {
  const base = `sec_${k.kind}`;
  const typ = k.field_type === "date" ? "date" : "number";
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className={LBL_CLS}>{k.label}</span>
      <input
        type={typ}
        value={values[`${base}_min`] ?? ""}
        onChange={(e) => onChange(`${base}_min`, e.target.value)}
        placeholder="≥"
        className={`${IN_CLS} w-32`}
      />
      <input
        type={typ}
        value={values[`${base}_max`] ?? ""}
        onChange={(e) => onChange(`${base}_max`, e.target.value)}
        placeholder="≤"
        className={`${IN_CLS} w-32`}
      />
    </div>
  );
}

/** 文本：关键词子串 */
function TextRow({
  k,
  values,
  onChange,
}: {
  k: SectionKindMeta;
  values: ValueMap;
  onChange: Setter;
}) {
  const base = `sec_${k.kind}`;
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className={LBL_CLS}>{k.label}</span>
      <input
        type="text"
        value={values[base] ?? ""}
        onChange={(e) => onChange(base, e.target.value)}
        placeholder={k.label}
        className={`${IN_CLS} w-48`}
      />
    </div>
  );
}

/** 按 field_type 分派单维度控件；未知类型回落文本输入（后端无法解释时不生成谓词） */
function DimRow(props: {
  k: SectionKindMeta;
  opts: SectionDictRow[];
  values: ValueMap;
  onChange: Setter;
  yes: string;
  no: string;
}) {
  const ft = props.k.field_type;
  if (ft === "select" || ft === "multiselect") return <EnumRow {...props} />;
  if (ft === "bool") return <BoolRow {...props} />;
  if (ft === "number" || ft === "date") return <RangeRow {...props} />;
  return <TextRow {...props} />;
}

/** 维度筛选面板：站长没建启用中的维度时整块不渲染 */
export function DimFilterPanel({
  kinds,
  dict,
  values,
  onChange,
  onClear,
}: {
  kinds: SectionKindMeta[];
  dict: Record<string, SectionDictRow[]>;
  values: ValueMap;
  onChange: Setter;
  onClear: () => void;
}) {
  const { dict: d } = useI18n();
  const at = d.adminTorrents;
  if (kinds.length === 0) return null;
  const anyOn = Object.values(values).some((v) => v !== "");
  return (
    <section className="baozi-panel flex flex-col gap-2 p-3">
      <div className="flex items-center justify-between">
        <span className="text-xs font-bold text-sub">{at.dimTitle}</span>
        {anyOn && (
          <button
            type="button"
            onClick={onClear}
            className="text-xs text-sky hover:underline"
          >
            {at.dimClear}
          </button>
        )}
      </div>
      {kinds.map((k) => (
        <DimRow
          key={k.kind}
          k={k}
          opts={dict[k.kind] ?? []}
          values={values}
          onChange={onChange}
          yes={at.dimBoolYes}
          no={at.dimBoolNo}
        />
      ))}
    </section>
  );
}
