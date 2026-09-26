"use client";

/** 促销档位注册表（0213）共享类型与样式常量。
 *  面板拆分：admin-promo-kinds.tsx（档位）/ admin-promo-tiers.tsx（价目）。 */

export interface KindRow {
  kind: string;
  label_zh: string;
  label_en: string;
  effect: string;
  i18n_key: string | null;
  sort_order: number;
  enabled: boolean;
}

export interface TierRow {
  id: number;
  kind: string;
  hours: number;
  price: number;
  enabled: boolean;
}

export interface PromoKindResp {
  kinds: KindRow[];
  tiers: TierRow[];
  effects: string[];
}

export const PK_TITLE_CLS = "mb-2 text-base font-bold";
export const PK_SUB_TITLE_CLS = "mb-2 mt-5 text-sm font-bold";
export const PK_INTRO_CLS = "mb-3 text-xs text-sub";
export const PK_META_CLS = "mb-3 rounded-[var(--r-md)] bg-sky-soft p-2 text-xs";
export const PK_TH_CLS = "colhead";
export const PK_TD_CLS = "rowfollow";
export const PK_FCOL_CLS = "flex flex-col gap-1";
export const PK_ROW_BTN_MR_CLS =
  "min-h-[28px] rounded-full border border-line px-3 font-bold mr-2 "
  + "text-sub disabled:opacity-50";
export const PK_ROW_BTN_DANGER_CLS =
  "min-h-[28px] rounded-full border border-line px-3 font-bold "
  + "text-danger disabled:opacity-50";
