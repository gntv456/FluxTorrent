"use client";

import { useState } from "react";
import { ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { ToolTab } from "@/components/staff-tools";
import { StaffOpsForms } from "./staff-tools-ops-forms";

/** 运营域面板（从 staff-tools.tsx 按域拆出，300 行门禁）：
 *  批量私信（staffmess）/ 添加用户（adduser）；表单拆至
 *  ./staff-tools-ops-forms.tsx。
 *
 *  曾同时承载「种子促销（promo）」的状态机与 /admin/freeleech 拉取，但那段
 *  渲染函数（OpsPromoTab）早已不被调用（同一端点已由 FreeleechPanel 承接），
 *  只剩一批只写不读的 useState 和一次无人消费的请求 —— 已一并删除。 */

export function StaffOpsPanel({
  tab,
  flash,
}: {
  tab: ToolTab;
  flash: (m: string) => void;
}) {
  const { dict } = useI18n();
  const [busy, setBusy] = useState(false);

  async function guard(fn: () => Promise<void>, ok: string) {
    setBusy(true);
    try {
      await fn();
      flash(ok);
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  // 表单组件只认这两个 tab；收窄后交给它（否则 tab: ToolTab 不匹配）
  if (tab !== "staffmess" && tab !== "adduser") return null;
  return <StaffOpsForms tab={tab} busy={busy} guard={guard} />;
}
