"use client";

/**
 * 站点设定顶部操作条 + 提示条（从 app/(main)/admin/settings/settings-client.tsx
 * 按域拆出）：标题/分区统计、导出/导入入口、未保存计数与保存按钮、
 * 明文导出二次确认、只读横幅与全局 toast。状态与动作由主组件注入。
 */

import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import type { SettingsSchema } from "@/components/setting-field";
import type { SettingsToast, useSettingsTransfer } from "./settings-io";

export function SettingsToolbar({
  schema,
  lastSaved,
  dirtyCount,
  saving,
  editable,
  toast,
  io,
  onSave,
  onDiscard,
}: {
  schema: SettingsSchema | null;
  lastSaved: Date | null;
  dirtyCount: number;
  saving: boolean;
  editable: boolean;
  toast: SettingsToast | null;
  io: ReturnType<typeof useSettingsTransfer>;
  onSave: () => void;
  onDiscard: () => void;
}) {
  const { dict, locale } = useI18n();
  const s = dict.settingsAdmin;
  return (
    <>
      {/* 顶部标题 + 全局操作条 */}
      <div className="sticky top-0 z-10 flex flex-wrap items-center gap-3 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)]/95 p-3 backdrop-blur shadow-[var(--shadow-card)]">
        <div>
          <h1 className="font-display text-xl">{s.title}</h1>
          <p className="text-[11px] text-sub">
            {schema
              ? fmt(s.groupFieldCount, {
                  g: schema.group_count,
                  f: schema.field_count,
                })
              : "…"}
            {lastSaved &&
              ` · ${fmt(s.lastSaved, { time: lastSaved.toLocaleTimeString(dateLocale(locale)) })}`}
          </p>
        </div>
        <span className="flex-1" />
        {/* 导出 / 导入（P2 §4.3）：导出对 staff 开放，明文导出与导入仅 sysop */}
        {schema && (
          <>
            {editable && (
              <label className="flex items-center gap-1.5 text-[11px] font-bold text-sub">
                <input
                  type="checkbox"
                  checked={io.exportPlain}
                  onChange={(e) => {
                    if (e.target.checked) io.setAskPlain(true);
                    else io.setExportPlain(false);
                  }}
                />
                {s.exportPlain}
              </label>
            )}
            <button
              type="button"
              onClick={io.doExport}
              className="min-h-[44px] rounded-full border border-line bg-[var(--surface-card)] px-4 text-xs font-bold text-ink"
            >
              {s.doExport}
            </button>
            {editable && (
              <button
                type="button"
                onClick={() => io.setImportOpen(true)}
                className="min-h-[44px] rounded-full border border-line bg-[var(--surface-card)] px-4 text-xs font-bold text-ink"
              >
                {s.doImport}
              </button>
            )}
          </>
        )}
        {dirtyCount > 0 && (
          <>
            <span className="rounded-full bg-sun/40 px-3 py-1 text-[11px] font-bold text-ink">
              {fmt(dict.settingsAdmin.unsavedTitle, { n: dirtyCount })} ·{" "}
              {dirtyCount}
            </span>
            <button
              type="button"
              onClick={onDiscard}
              className="min-h-[44px] rounded-full border border-line bg-[var(--surface-card)] px-4 text-xs font-bold text-sub"
            >
              {s.discard}
            </button>
          </>
        )}
        {editable && (
          <button
            type="button"
            onClick={onSave}
            disabled={saving || dirtyCount === 0}
            className="min-h-[44px] rounded-full bg-sky-deep px-5 text-sm font-bold text-white disabled:opacity-40"
          >
            {saving
              ? s.saving
              : dirtyCount > 0
                ? fmt(s.saveAllCount, { n: dirtyCount })
                : s.saveAll}
          </button>
        )}
      </div>

      {!editable && schema && (
        <p className="rounded-[var(--r-md)] border border-line bg-sky-soft p-3 text-xs text-ink">
          {s.readOnlyBanner}
        </p>
      )}

      {/* 明文导出二次确认（密文显隐确认，§6.3 / §7.1） */}
      {io.askPlain && (
        <div className="flex flex-wrap items-center gap-2 rounded-[var(--r-md)] border border-danger/40 bg-danger/5 p-3">
          <p className="text-xs font-bold text-ink">{s.exportPlainConfirm}</p>
          <span className="flex-1" />
          <button
            type="button"
            onClick={() => {
              io.setExportPlain(true);
              io.setAskPlain(false);
            }}
            className="min-h-[44px] rounded-full bg-danger px-4 text-xs font-bold text-white"
          >
            {s.revealConfirmYes}
          </button>
          <button
            type="button"
            onClick={() => {
              io.setExportPlain(false);
              io.setAskPlain(false);
            }}
            className="min-h-[44px] rounded-full border border-line bg-[var(--surface-card)] px-4 text-xs font-bold text-sub"
          >
            {s.cancel}
          </button>
        </div>
      )}

      {toast && (
        <div
          className={`rounded-[var(--r-md)] border p-3 text-sm ${
            toast.ok
              ? "border-mint/40 bg-mint/10 text-ink"
              : "border-danger/40 bg-danger/5 text-ink"
          }`}
          role="status"
        >
          <p className="font-bold">{toast.text}</p>
          {toast.ok && (
            <p className="mt-0.5 text-[11px] text-sub">{s.cacheHint}</p>
          )}
          {toast.effects && toast.effects.length > 0 && (
            <ul className="mt-2 list-disc pl-5 text-[11px] text-sub">
              {toast.effects.map((e) => (
                <li key={e}>{e}</li>
              ))}
            </ul>
          )}
        </div>
      )}
    </>
  );
}
