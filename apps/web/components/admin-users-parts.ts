"use client";

/**
 * 后台用户管理·筛选状态（G4，2026-09-26 从 admin-users.tsx 按域拆出，
 * 主组件守 300 行门禁）。七维筛选 + 自定义字段值筛选；「筛选项 ↔ 查询串」
 * 的单点映射在这里，列表加载只负责带上排序/分页。
 */

import { useCallback, useMemo, useState } from "react";

export interface UserFilterValues {
  q: string;
  fId: string;
  fClass: string;
  fStatus: string;
  fEnabled: string;
  fDownload: string;
  fSuspended: string;
  /** 自定义字段 key（G4）；空 = 不按字段筛 */
  fField: string;
  /** 字段值关键词（子串）；字段已选但这里留空 = 「填过这个字段的人」 */
  fVal: string;
}

const EMPTY: UserFilterValues = {
  q: "",
  fId: "",
  fClass: "",
  fStatus: "",
  fEnabled: "",
  fDownload: "",
  fSuspended: "",
  fField: "",
  fVal: "",
};

export function useUserFilters() {
  const [values, setValues] = useState<UserFilterValues>(EMPTY);
  const set = useCallback(
    <K extends keyof UserFilterValues>(k: K, v: UserFilterValues[K]): void =>
      setValues((prev) => ({ ...prev, [k]: v })),
    [],
  );
  /** 拼查询串：空值跳过；字段筛选只在选了字段时才带 */
  const params = useCallback(
    (extra: Record<string, string>) => {
      const p = new URLSearchParams(extra);
      if (values.q.trim()) p.set("q", values.q.trim());
      if (values.fId.trim()) p.set("id", values.fId.trim());
      if (values.fClass) p.set("class_id", values.fClass);
      if (values.fStatus) p.set("status", values.fStatus);
      if (values.fEnabled) p.set("enabled", values.fEnabled);
      if (values.fDownload) p.set("download", values.fDownload);
      if (values.fSuspended) p.set("suspended", values.fSuspended);
      if (values.fField) {
        p.set("ffield", values.fField);
        if (values.fVal.trim()) p.set("fval", values.fVal.trim());
      }
      return p;
    },
    [values],
  );
  // 引用稳定：调用方把 `filters` 放进 useCallback deps 时不至于每渲染都变
  return useMemo(() => ({ values, set, params }), [values, set, params]);
}
