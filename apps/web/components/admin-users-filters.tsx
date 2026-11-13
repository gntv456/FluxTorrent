"use client";

import { useEffect, useState } from "react";
import { INPUT_MD } from "@/lib/ui-classes";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { useMediaQuery } from "@/lib/hooks/use-media";

/** 后台用户管理·筛选面板（从 components/admin-users.tsx 按域拆出）：
 *  五维筛选（ID/等级/状态/启用/下载权限/挂起）+ 搜索（同类站「筛选条件」口径）。
 *  筛选值由父组件持有，搜索时重置回第一页并重新加载。
 *
 *  2026-11-03：筛选项多、展开态把表格推到首屏之外（1024/375 尤其明显）。
 *  <1280 默认收起，表格上方保留一行摘要（已选条件 chips + 结果条数 +
 *  展开按钮）；≥1280 默认展开。展开状态按用户偏好持久化（localStorage），
 *  刷新后保持；查询串语义不变（仍由 admin-users-parts.ts 的单点映射生成）。
 *  Esc 不绑定收起——避免误触把已填条件面板收掉。 */

/** 折叠偏好键（与 flux-theme / flux-pwa-dismiss 同「flux-」前缀习惯） */
const FOLD_KEY = "flux-admin-filters-open";

// 筛选区输入/下拉底色
const FILTER_INPUT = INPUT_MD;
const FILTER_SELECT =
  "min-h-11 rounded-[var(--r-sm)] border border-line " +
  "bg-[var(--surface-card)] px-2";
const SEARCH_BTN =
  "min-h-11 rounded-full bg-sky px-4 text-xs font-bold text-[var(--text-on-accent)]";
/** 重置：次要按钮（描边 + 面板底），排在搜索左侧，不抢主操作；
 *  禁用态用淡字 + 淡边，仍可辨认（不是隐身）。 */
const RESET_BTN =
  "min-h-11 shrink-0 rounded-full border border-line " +
  "bg-[var(--surface-card)] px-4 text-xs font-bold text-[var(--text-body)] " +
  "disabled:cursor-not-allowed disabled:border-[var(--border-soft)] " +
  "disabled:text-[var(--text-faint)]";
const PANEL = "brand-panel grid grid-cols-2 gap-3 p-4 md:grid-cols-4";
const TOGGLE_BTN =
  "min-h-11 shrink-0 rounded-full border border-line px-3" +
  " text-xs font-bold";

export interface FilterValues {
  q: string;
  fId: string;
  fClass: string;
  fStatus: string;
  fEnabled: string;
  fDownload: string;
  fSuspended: string;
  /** 自定义字段筛选（G4）：字段 key + 值关键词（留空 = 填过即可） */
  fField: string;
  fVal: string;
}

export function UsersFilterBar({
  classes,
  fields,
  values,
  applied,
  set,
  onSearch,
  onReset,
  canReset,
  onCollapse,
  total,
}: {
  classes: [number, string][];
  /** 自定义字段（G4）：站长没建字段时整对控件不渲染 */
  fields: { key: string; label: string }[];
  values: FilterValues;
  /** 最近一次生效（提交/防抖触发）的快照（父组件持有，摘要只认它） */
  applied: FilterValues;
  /** 筛选变更：文本位父组件做 400ms 防抖，选择位立即生效 */
  set: <K extends keyof FilterValues>(k: K, v: FilterValues[K]) => void;
  onSearch: () => void;
  /** 清空全部条件（草稿 + 已生效）并立即生效 */
  onReset: () => void;
  /** 是否有可清条件（无则重置按钮 disabled） */
  canReset: boolean;
  /** 收起面板时通知父组件取消待触发的 debounce */
  onCollapse?: () => void;
  /** 当前筛选条件下的结果总数（loading 时为 undefined） */
  total?: number;
}) {
  const { dict } = useI18n();
  const at = dict.adminUsers;
  /** 复用既有「重置全部」文案（torrents2.advReset，三语齐全），不新增近义键 */
  const resetLabel = dict.torrents2.advReset;
  /** SSR/首帧按窄屏处理（默认收起），挂载后由 matchMedia 校正到桌面展开 */
  const narrow = useMediaQuery("(max-width: 1279px)", true);
  const [open, setOpen] = useState(true);

  /** 偏好优先；没有偏好时按断点默认（<1280 收起 / ≥1280 展开）。 */
  useEffect(() => {
    let saved: string | null = null;
    try {
      saved = window.localStorage.getItem(FOLD_KEY);
    } catch {
      /* 隐私模式禁用存储：退回断点默认 */
    }
    if (saved === "1" || saved === "0") setOpen(saved === "1");
    else setOpen(!narrow);
  }, [narrow]);

  function toggle() {
    const next = !open;
    if (!next) onCollapse?.();
    setOpen(next);
    try {
      window.localStorage.setItem(FOLD_KEY, next ? "1" : "0");
    } catch {
      /* 存不了就只在本页生效 */
    }
  }

  /** 已选条件摘要：只取已生效快照（点「搜索」时冻结），不随表单草稿变化 */
  const chips: string[] = [];
  if (applied.q.trim()) chips.push(`${at.fSearch}: ${applied.q.trim()}`);
  if (applied.fId.trim()) chips.push(`ID: ${applied.fId.trim()}`);
  if (applied.fClass) {
    const label =
      classes.find(([id]) => String(id) === applied.fClass)?.[1] ??
      applied.fClass;
    chips.push(`${at.fClass}: ${label}`);
  }
  if (applied.fStatus) {
    const label = [at.stNormal, at.stMuted, at.stBanned][
      Number(applied.fStatus) - 1
    ];
    chips.push(`${at.fStatus}: ${label ?? applied.fStatus}`);
  }
  if (applied.fEnabled)
    chips.push(
      `${at.fEnabled}: ${applied.fEnabled === "yes" ? at.optYes : at.optNo}`,
    );
  if (applied.fDownload)
    chips.push(
      `${at.fDownload}: ${applied.fDownload === "yes" ? at.optHas : at.optNot}`,
    );
  if (applied.fSuspended)
    chips.push(
      `${at.fSuspended}: ${applied.fSuspended === "yes" ? at.optYes : at.optNo}`,
    );
  if (applied.fField) {
    const label =
      fields.find((f) => f.key === applied.fField)?.label ?? applied.fField;
    chips.push(
      `${at.fField}: ${label}${applied.fVal.trim() ? ` · ${applied.fVal.trim()}` : ""}`,
    );
  }

  return (
    <div className="flex flex-col gap-2">
      {/* 摘要行（收起/展开两态都在）：chips + 结果条数 + 一键切换 */}
      <div className="brand-panel flex flex-wrap items-center gap-2 px-3 py-2">
        <span className="tsb-active__title">{at.filtersTitle}</span>
        <div className="tsb-active__list">
          {chips.length === 0 ? (
            <span className="text-xs text-[var(--text-faint)]">
              {at.filtersNone}
            </span>
          ) : (
            chips.map((c, i) => (
              <span key={`${i}-${c}`} className="tsb-active__chip">
                {c}
              </span>
            ))
          )}
        </div>
        {total !== undefined && (
          <span className="text-xs font-bold text-[var(--text-muted)]">
            {fmt(dict.common.totalItems, { n: total })}
          </span>
        )}
        <button
          type="button"
          onClick={toggle}
          aria-expanded={open}
          aria-controls="admin-users-filters"
          className={TOGGLE_BTN}
        >
          {open ? at.filtersCollapse : at.filtersExpand}
        </button>
      </div>
      <section
        id="admin-users-filters"
        className={open ? PANEL : "hidden"}
      >
      <label className="flex flex-col gap-1 text-xs">
        {at.fId}
        <input
          value={values.fId}
          onChange={(e) => set("fId", e.target.value)}
          placeholder="UID"
          className={FILTER_INPUT}
        />
      </label>
      <label className="flex flex-col gap-1 text-xs">
        {at.fClass}
        <select
          value={values.fClass}
          onChange={(e) => set("fClass", e.target.value)}
          className={FILTER_SELECT}
        >
          <option value="">{at.optAll}</option>
          {classes.map(([id, label]) => (
            <option key={id} value={id}>
              {label}
            </option>
          ))}
        </select>
      </label>
      <label className="flex flex-col gap-1 text-xs">
        {at.fStatus}
        <select
          value={values.fStatus}
          onChange={(e) => set("fStatus", e.target.value)}
          className={FILTER_SELECT}
        >
          <option value="">{at.optAll}</option>
          <option value="1">{at.stNormal}</option>
          <option value="2">{at.stMuted}</option>
          <option value="3">{at.stBanned}</option>
        </select>
      </label>
      <label className="flex flex-col gap-1 text-xs">
        {at.fEnabled}
        <select
          value={values.fEnabled}
          onChange={(e) => set("fEnabled", e.target.value)}
          className={FILTER_SELECT}
        >
          <option value="">{at.optAll}</option>
          <option value="yes">{at.optYes}</option>
          <option value="no">{at.optNo}</option>
        </select>
      </label>
      <label className="flex flex-col gap-1 text-xs">
        {at.fDownload}
        <select
          value={values.fDownload}
          onChange={(e) => set("fDownload", e.target.value)}
          className={FILTER_SELECT}
        >
          <option value="">{at.optAll}</option>
          <option value="yes">{at.optHas}</option>
          <option value="no">{at.optNot}</option>
        </select>
      </label>
      <label className="flex flex-col gap-1 text-xs">
        {at.fSuspended}
        <select
          value={values.fSuspended}
          onChange={(e) => set("fSuspended", e.target.value)}
          className={FILTER_SELECT}
        >
          <option value="">{at.optAll}</option>
          <option value="yes">{at.optYes}</option>
          <option value="no">{at.optNo}</option>
        </select>
      </label>
      <label className="col-span-2 flex flex-col gap-1 text-xs">
        {at.fSearch}
        <div className="flex gap-2">
          <input
            value={values.q}
            onChange={(e) => set("q", e.target.value)}
            placeholder={at.qPh}
            className={`${FILTER_INPUT} min-w-0 flex-1`}
          />
          <button
            type="button"
            onClick={onReset}
            disabled={!canReset}
            className={RESET_BTN}
          >
            {resetLabel}
          </button>
          <button type="button" onClick={onSearch} className={SEARCH_BTN}>
            {at.search}
          </button>
        </div>
      </label>
      {/* 自定义字段筛选（G4）：值留空 = 只要填过该字段的人 */}
      {fields.length > 0 && (
        <>
          <label className="flex flex-col gap-1 text-xs">
            {at.fField}
            <select
              value={values.fField}
              onChange={(e) => set("fField", e.target.value)}
              className={FILTER_SELECT}
            >
              <option value="">{at.optAll}</option>
              {fields.map((f) => (
                <option key={f.key} value={f.key}>
                  {f.label}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-xs">
            {at.fFieldVal}
            <input
              value={values.fVal}
              onChange={(e) => set("fVal", e.target.value)}
              placeholder={at.fFieldValPh}
              className={FILTER_INPUT}
            />
          </label>
        </>
      )}
      </section>
    </div>
  );
}
