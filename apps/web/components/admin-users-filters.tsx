"use client";

import { INPUT_MD } from "@/lib/ui-classes";
import { useI18n } from "@/i18n/client";

/** 后台用户管理·筛选面板（从 components/admin-users.tsx 按域拆出）：
 *  五维筛选（ID/等级/状态/启用/下载权限/挂起）+ 搜索（好学站「筛选条件」口径）。
 *  筛选值由父组件持有，搜索时重置回第一页并重新加载。 */

// 筛选区输入/下拉底色
const FILTER_INPUT =
  INPUT_MD;
const FILTER_SELECT =
  "min-h-[40px] rounded-[var(--r-sm)] border border-line " +
  "bg-[var(--surface-card)] px-2";
const SEARCH_BTN =
  "min-h-[40px] rounded-full bg-sky px-4 text-xs font-bold text-white";

export interface FilterValues {
  q: string;
  fId: string;
  fClass: string;
  fStatus: string;
  fEnabled: string;
  fDownload: string;
  fSuspended: string;
}

export function UsersFilterBar({
  classes,
  values,
  set,
  onSearch,
}: {
  classes: [number, string][];
  values: FilterValues;
  set: <K extends keyof FilterValues>(k: K, v: FilterValues[K]) => void;
  onSearch: () => void;
}) {
  const at = useI18n().dict.adminUsers;
  return (
    <section className="baozi-panel grid grid-cols-2 gap-3 p-4 md:grid-cols-4">
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
      <label className="flex flex-col gap-1 text-xs md:col-span-2">
        {at.fSearch}
        <div className="flex gap-2">
          <input
            value={values.q}
            onChange={(e) => set("q", e.target.value)}
            placeholder={at.qPh}
            className={`${FILTER_INPUT} flex-1`}
          />
          <button onClick={onSearch} className={SEARCH_BTN}>
            {at.search}
          </button>
        </div>
      </label>
    </section>
  );
}
