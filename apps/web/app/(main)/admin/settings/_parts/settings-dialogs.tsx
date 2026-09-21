"use client";

/**
 * 站点设定抽屉/对话框件（从 app/(main)/admin/settings/settings-client.tsx 按域拆出）：
 * SettingsHistoryDrawer 修改历史抽屉、SettingsImportDialog 导入对话框。
 * 数据装载与状态留在 settings-client.tsx。
 */

import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import type { DiffRow, HistoryRow } from "./settings-types";

/** 修改历史抽屉：单字段历史列表，diff 高亮（旧值红色划线，新值加粗） */
export function SettingsHistoryDrawer({
  name,
  rows,
  onClose,
}: {
  name: string;
  rows: HistoryRow[];
  onClose: () => void;
}) {
  const { dict, locale } = useI18n();
  const s = dict.settingsAdmin;
  return (
    <div
      className="fixed inset-0 z-30 flex justify-end bg-black/30"
      role="dialog"
      aria-modal="true"
      onClick={onClose}
    >
      <div
        className="h-full w-full max-w-md overflow-y-auto bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-3 flex items-center gap-2">
          <h2 className="flex-1 text-sm font-bold text-ink">
            {fmt(s.historyTitle, { name })}
          </h2>
          <button
            type="button"
            onClick={onClose}
            className="min-h-[44px] rounded-full border border-line px-3 text-xs font-bold"
          >
            {s.close}
          </button>
        </div>
        {rows.length === 0 && (
          <p className="py-6 text-center text-sm text-sub">{s.historyEmpty}</p>
        )}
        <ul className="flex flex-col divide-y divide-line">
          {rows.map((r) => (
            <li key={r.id} className="py-2 text-xs">
              <p className="text-sub">
                {r.actor ?? `#${r.actor_id ?? "-"}`} ·{" "}
                {new Date(r.created_at).toLocaleString(dateLocale(locale))}
              </p>
              <div className="mt-1 flex flex-col gap-1">
                {/* diff 高亮：旧值红色划线，新值加粗 */}
                <p className="flex flex-wrap items-baseline gap-1.5 break-all">
                  <span className="shrink-0 rounded-full bg-danger/10 px-2 py-0.5 text-[10px] font-bold text-danger">
                    {s.historyOld}
                  </span>
                  <span className="font-mono text-danger line-through">
                    {r.detail?.old ?? "—"}
                  </span>
                </p>
                <p className="flex flex-wrap items-baseline gap-1.5 break-all">
                  <span className="shrink-0 rounded-full bg-mint/15 px-2 py-0.5 text-[10px] font-bold text-ink">
                    {s.historyNew}
                  </span>
                  <span className="font-mono font-bold text-ink">
                    {r.detail?.new ?? "—"}
                  </span>
                </p>
                {r.detail?.via === "import" && (
                  <span className="self-start rounded-full bg-sky-soft px-2 py-0.5 text-[10px] font-bold text-sub">
                    {s.viaImport}
                  </span>
                )}
              </div>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}

export interface ImportDialogProps {
  open: boolean;
  busy: boolean;
  text: string;
  unknown: string[];
  skipped: string[];
  diff: DiffRow[] | null;
  onText: (v: string) => void;
  onPickFile: (f: File | null) => void;
  onDryRun: () => void;
  onApply: () => void;
  onClose: () => void;
}

/** 导入对话框：先空跑出 diff，再强制确认落库 */
export function SettingsImportDialog({
  busy,
  text,
  unknown,
  skipped,
  diff,
  onText,
  onPickFile,
  onDryRun,
  onApply,
  onClose,
}: ImportDialogProps) {
  const { dict, currency } = useI18n();
  const s = dict.settingsAdmin;
  return (
    <div
      className="fixed inset-0 z-30 flex items-center justify-center bg-black/30 p-3"
      role="dialog"
      aria-modal="true"
      onClick={onClose}
    >
      <div
        className="max-h-full w-full max-w-2xl overflow-y-auto rounded-[var(--r-md)] bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-3 flex items-center gap-2">
          <h2 className="flex-1 text-sm font-bold text-ink">{s.importTitle}</h2>
          <button
            type="button"
            onClick={onClose}
            className="min-h-[44px] rounded-full border border-line px-3 text-xs font-bold"
          >
            {s.close}
          </button>
        </div>
        <p className="mb-2 text-[11px] leading-snug text-sub">{s.importHint}</p>
        <input
          type="file"
          accept="application/json,.json"
          onChange={(e) => onPickFile(e.target.files?.[0] ?? null)}
          className="mb-2 block w-full text-xs"
        />
        <textarea
          value={text}
          onChange={(e) => onText(e.target.value)}
          rows={6}
          placeholder={s.importPlaceholder}
          className="w-full rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] p-2 font-mono text-[11px] text-ink"
        />
        {unknown.length > 0 && (
          <p className="mt-2 rounded-[var(--r-sm)] border border-sun/60 bg-sun/10 p-2 text-[11px] text-ink">
            {fmt(s.importUnknown, { n: unknown.length })}：
            {unknown.slice(0, 12).join(", ")}
          </p>
        )}
        {skipped.length > 0 && (
          <p className="mt-2 rounded-[var(--r-sm)] border border-line bg-cloud p-2 text-[11px] text-sub">
            {fmt(s.importSkipped, { n: skipped.length })}：{skipped.join(", ")}
          </p>
        )}
        {diff && (
          <div className="mt-3">
            <p className="mb-1 text-xs font-bold text-ink">
              {diff.length === 0
                ? s.importNoChange
                : fmt(s.importWillChange, { n: diff.length })}
            </p>
            {diff.length > 0 && (
              <ul className="max-h-56 divide-y divide-line overflow-y-auto rounded-[var(--r-sm)] border border-line">
                {diff.map((d) => (
                  <li key={d.name} className="p-2 text-[11px]">
                    <p className="flex flex-wrap items-baseline gap-1.5">
                      <span className="font-mono font-bold text-ink">
                        {d.name}
                      </span>
                      <span className="rounded-full bg-sky-soft px-1.5 py-0.5 text-[10px] text-sub">
                        {(dict.admin.settingGroups[d.group] ?? d.group).replace(
                          "{magic}",
                          currency,
                        )}
                      </span>
                    </p>
                    <p className="mt-0.5 break-all">
                      <span className="font-mono text-danger line-through">
                        {d.old || "—"}
                      </span>
                      <span className="mx-1 text-sky">→</span>
                      <span className="font-mono font-bold text-ink">
                        {d.new || "—"}
                      </span>
                    </p>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
        <div className="mt-3 flex flex-wrap gap-2">
          <button
            type="button"
            onClick={onDryRun}
            disabled={busy || text.trim() === ""}
            className="min-h-[44px] rounded-full border border-line bg-[var(--surface-card)] px-4 text-xs font-bold text-ink disabled:opacity-40"
          >
            {s.importPreview}
          </button>
          <button
            type="button"
            onClick={onApply}
            disabled={busy || !diff || diff.length === 0}
            className="min-h-[44px] rounded-full bg-sky-deep px-5 text-xs font-bold text-white disabled:opacity-40"
          >
            {busy ? s.saving : s.importConfirm}
          </button>
        </div>
      </div>
    </div>
  );
}
