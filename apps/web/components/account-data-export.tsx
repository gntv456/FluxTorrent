"use client";

import { useState } from "react";
import { useI18n } from "@/i18n/client";
import { api } from "@/lib/api-client";
import { Badge } from "@/components/ui/badge";

/** 账号数据可携带（数据可移植）：从既有只读端点拼出一份用户数据快照。
 *  不新增后端端点、不含凭据/会话；失败项如实记录，不伪造空数据。 */
const SOURCES = [
  ["profile", "/api/v1/me/overview"],
  ["settings", "/api/v1/me/settings"],
  ["noticePrefs", "/api/v1/me/notice-prefs"],
  ["listPrefs", "/api/v1/me/list-prefs"],
  ["fields", "/api/v1/me/fields"],
  ["risk", "/api/v1/me/risk"],
  ["onboarding", "/api/v1/me/onboarding"],
  ["hr", "/api/v1/me/hr"],
  ["invites", "/api/v1/invites/status"],
  ["invitees", "/api/v1/invites/invitees"],
  ["bookmarks", "/api/v1/me/bookmarks"],
  ["medals", "/api/v1/me/medals"],
] as const;

interface ExportError {
  section: string;
  message: string;
}

export function AccountDataExportCard() {
  const { dict } = useI18n();
  const u = dict.uxOpt;
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<"done" | "partial" | null>(null);

  async function exportData() {
    setBusy(true);
    setStatus(null);
    const data: Record<string, unknown> = {};
    const errors: ExportError[] = [];
    await Promise.all(
      SOURCES.map(async ([key, path]) => {
        try {
          data[key] = await api.get<unknown>(path);
        } catch (err) {
          errors.push({
            section: key,
            message: err instanceof Error ? err.message : "request failed",
          });
        }
      }),
    );
    const payload = {
      schema: "flux-account-data/v1",
      exportedAt: new Date().toISOString(),
      scope: u.dataExportDesc,
      data,
      errors,
    };
    const blob = new Blob([JSON.stringify(payload, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "flux-account-data.json";
    a.click();
    URL.revokeObjectURL(url);
    setStatus(errors.length ? "partial" : "done");
    setBusy(false);
  }

  return (
    <section className="ux-foot" aria-label={u.dataExportTitle}>
      <h2 className="ux-foot__title">{u.dataExportTitle}</h2>
      <p className="ux-foot__desc">{u.dataExportDesc}</p>
      <button
        type="button"
        className="ux-foot__btn"
        disabled={busy}
        onClick={exportData}
      >
        {busy ? u.dataExportBusy : u.dataExportAction}
      </button>
      {status ? (
        <p role="status" className="ux-foot__desc">
          <Badge tone={status === "done" ? "success" : "warn"} size="sm">
            {status === "done" ? u.dataExportDone : u.dataExportPartial}
          </Badge>
        </p>
      ) : null}
      <p className="ux-foot__empty">{u.dataExportPrivacy}</p>
    </section>
  );
}
