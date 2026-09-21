"use client";

import { BTN_SM_BOLD } from "@/lib/ui-classes";
import { OpsPromoTab } from "./staff-tools-ops-promo";
import type { SitePromo } from "./staff-tools-ops-types";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { ToolTab } from "@/components/staff-tools";
import { StaffOpsForms } from "./staff-tools-ops-forms";

/** 运营域面板（从 staff-tools.tsx 按域拆出，300 行门禁）：
 *  种子促销（promo）/ 批量私信（staffmess）/ 添加用户（adduser）。
 *  私信/建号表单拆至 ./staff-tools-ops-forms.tsx。 */

interface CatItem {
  id: number;
  name: string;
  torrents: number;
}

export function StaffOpsPanel({
  tab,
  flash,
}: {
  tab: ToolTab;
  flash: (m: string) => void;
}) {
  const { dict } = useI18n();
  const t = dict.stafftools;
  const [cats, setCats] = useState<CatItem[]>([]);
  const [busy, setBusy] = useState(false);

  // 运营工具状态
  const [promo, setPromo] = useState<SitePromo[]>([]);
  const [promoEditId, setPromoEditId] = useState<number | null>(null);
  const [promoScope, setPromoScope] = useState("global");
  const [promoCat, setPromoCat] = useState<number | "">("");
  const [promoStart, setPromoStart] = useState("");
  const [promoEnd, setPromoEnd] = useState("");
  const [promoKind, setPromoKind] = useState("free");
  const [promoHours, setPromoHours] = useState(24);

  const load = useCallback(async () => {
    api
      .get<SitePromo[]>("/api/v1/admin/freeleech")
      .then(setPromo)
      .catch(() => {});
    // 促销按分类时需分类列表（管理口径，无权限时为空）
    api
      .get<CatItem[]>("/api/v1/admin/categories")
      .then(setCats)
      .catch(() => setCats([]));
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  async function guard(fn: () => Promise<void>, ok: string) {
    setBusy(true);
    try {
      await fn();
      flash(ok);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      {/* 批量私信/添加用户（拆至 ./staff-tools-ops-forms.tsx） */}
      {(tab === "staffmess" || tab === "adduser") && (
        <StaffOpsForms tab={tab} busy={busy} guard={guard} />
      )}
    </>
  );
}
