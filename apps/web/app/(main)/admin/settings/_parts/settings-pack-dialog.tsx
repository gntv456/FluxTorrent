"use client";

/**
 * 内容包对话框（生态商店 M1，策划案 §3）：本站导出（taxonomy/theme 两键）、
 * 外部包粘贴/上传 → 空跑预览 → 确认导入、已装包清单与一键回滚（A2）。
 * 状态与动作在 settings-io.ts 的 useSettingsTransfer，本件纯展示。
 */

import { useState } from "react";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import type {
  AdapterRow,
  CatalogItem,
  CatalogResponse,
  ContentPackRow,
  PackImportResult,
  RuleTryResult,
} from "./settings-types";

export interface PackDialogProps {
  busy: boolean;
  text: string;
  preview: PackImportResult | null;
  rows: ContentPackRow[] | null;
  catalog: CatalogResponse | null;
  installBusyId: string | null;
  ruleTryResult: RuleTryResult | null;
  adapters: AdapterRow[] | null;
  adapterTryId: string | null;
  onText: (v: string) => void;
  onPickFile: (f: File | null) => void;
  onExportKind: (kind: "taxonomy" | "theme") => void;
  onDryRun: () => void;
  onApply: () => void;
  onRollback: (id: number) => void;
  onInstall: (packId: string) => void;
  onTryRule: (key: string, expr: string, termDays: number) => void;
  onToggleAdapter: (adapterId: string, enabled: boolean) => void;
  onDeleteAdapter: (adapterId: string) => void;
  onTryAdapter: (adapterId: string, url: string) => void;
  onClose: () => void;
}

/** 规则试算小节（M3）：选定规则键，输入表达式与期限变量，后端 lint+求值 */
function RuleTrySection({
  ruleTryResult,
  onTryRule,
}: {
  ruleTryResult: RuleTryResult | null;
  onTryRule: (key: string, expr: string, termDays: number) => void;
}) {
  const { dict } = useI18n();
  const s = dict.settingsAdmin;
  const [key, setKey] = useState("rule_bank_term_rate");
  const [expr, setExpr] = useState("min(0.18, max(0.005, term_days / 30 * 0.02))");
  const [days, setDays] = useState(90);
  return (
    <div className="mb-4 rounded-[var(--r-md)] border border-line p-3">
      <h3 className="mb-2 text-xs font-bold text-ink">{s.ruleTryTitle}</h3>
      <div className="mb-2 flex flex-wrap items-center gap-2 text-xs">
        <select
          value={key}
          onChange={(e) => setKey(e.target.value)}
          className="min-h-[36px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2"
        >
          <option value="rule_bank_term_rate">{s.ruleKeyTermRate}</option>
          <option value="rule_bank_loan_daily">{s.ruleKeyLoanDaily}</option>
        </select>
        <label className="flex items-center gap-1 text-[11px] text-sub">
          {s.ruleVarDays}
          <input
            type="number"
            min={1}
            max={3650}
            value={days}
            onChange={(e) => setDays(Number(e.target.value) || 0)}
            className="w-20 rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2 py-1"
          />
        </label>
      </div>
      <textarea
        className="mb-2 h-16 w-full resize-y rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-2 font-mono text-[11px]"
        value={expr}
        onChange={(e) => setExpr(e.target.value)}
        spellCheck={false}
      />
      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          onClick={() => onTryRule(key, expr, days)}
          className="min-h-[36px] rounded-full border border-line px-3 text-[11px] font-bold text-ink"
        >
          {s.ruleTryBtn}
        </button>
        {ruleTryResult && (
          <span className="font-mono text-[11px] text-sub">
            = {ruleTryResult.value ?? "—"}{" "}
            ({s.ruleDomain} [{ruleTryResult.domain[0]}, {ruleTryResult.domain[1]}])
          </span>
        )}
      </div>
      <p className="mt-1.5 text-[10px] text-sub">{s.ruleTryHint}</p>
    </div>
  );
}

/** 适配器小节（M4）：列表 / 启停 / 健康度（strikes）/ 试调 */
const ADAPTER_DEL_CLS =
  "min-h-[36px] rounded-full border border-line px-3 text-[11px] text-sub";

function AdapterSection({
  adapters,
  adapterTryId,
  onToggleAdapter,
  onDeleteAdapter,
  onTryAdapter,
}: {
  adapters: AdapterRow[] | null;
  adapterTryId: string | null;
  onToggleAdapter: (adapterId: string, enabled: boolean) => void;
  onDeleteAdapter: (adapterId: string) => void;
  onTryAdapter: (adapterId: string, url: string) => void;
}) {
  const { dict } = useI18n();
  const s = dict.settingsAdmin;
  const [tryUrl, setTryUrl] = useState("https://movie.douban.com/subject/1292052/");
  return (
    <div className="mb-4">
      <h3 className="mb-2 text-xs font-bold text-ink">{s.adapterTitle}</h3>
      {adapters === null && (
        <p className="py-2 text-center text-xs text-sub">…</p>
      )}
      {adapters?.length === 0 && (
        <p className="py-2 text-center text-xs text-sub">{s.adapterEmpty}</p>
      )}
      {adapters && adapters.length > 0 && (
        <>
          <input
            value={tryUrl}
            onChange={(e) => setTryUrl(e.target.value)}
            placeholder="https://…"
            className="mb-2 w-full rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2 py-1.5 font-mono text-[11px]"
            spellCheck={false}
          />
          <ul className="flex flex-col divide-y divide-line">
            {adapters.map((a) => (
              <li key={a.id} className="flex flex-wrap items-center gap-2 py-2 text-xs">
                <div className="min-w-0 flex-1">
                  <p className="truncate font-bold text-ink">
                    {a.name}{" "}
                    <span className="font-mono text-[10px] text-sub">
                      {a.adapter_id} · v{a.version}
                    </span>
                    {a.enabled ? (
                      <span className="ml-1 rounded-full bg-mint/20 px-2 py-0.5 text-[10px] font-bold text-ink">
                        {s.adapterOn}
                      </span>
                    ) : (
                      <span className="ml-1 rounded-full bg-danger/10 px-2 py-0.5 text-[10px] font-bold text-danger">
                        {s.adapterOff}
                      </span>
                    )}
                  </p>
                  <p className="truncate text-[11px] text-sub">
                    {a.kind} · {s.adapterStrikes}: {a.strikes}/3
                    {a.last_error ? ` · ${a.last_error.slice(0, 60)}` : ""}
                  </p>
                </div>
                <button
                  type="button"
                  disabled={adapterTryId === a.adapter_id}
                  onClick={() => onTryAdapter(a.adapter_id, tryUrl)}
                  className="min-h-[36px] rounded-full border border-line px-3 text-[11px] font-bold text-ink disabled:opacity-40"
                >
                  {s.adapterTryBtn}
                </button>
                <button
                  type="button"
                  onClick={() => onToggleAdapter(a.adapter_id, !a.enabled)}
                  className={`min-h-[36px] rounded-full px-3 text-[11px] font-bold ${
                    a.enabled
                      ? "border border-danger/40 text-danger"
                      : "bg-sky-deep text-white"
                  }`}
                >
                  {a.enabled ? s.adapterDisable : s.adapterEnable}
                </button>
                <button
                  type="button"
                  onClick={() => {
                    const msg = fmt(s.adapterDeleteConfirm, {
                      id: a.adapter_id,
                    });
                    if (window.confirm(msg)) onDeleteAdapter(a.adapter_id);
                  }}
                  className={ADAPTER_DEL_CLS}
                >
                  {s.adapterDelete}
                </button>
              </li>
            ))}
          </ul>
          <p className="mt-1.5 text-[10px] text-sub">{s.adapterHint}</p>
        </>
      )}
    </div>
  );
}

export function ContentPackDialog({
  busy,
  text,
  preview,
  rows,
  catalog,
  installBusyId,
  ruleTryResult,
  adapters,
  adapterTryId,
  onText,
  onPickFile,
  onExportKind,
  onDryRun,
  onApply,
  onRollback,
  onInstall,
  onTryRule,
  onToggleAdapter,
  onDeleteAdapter,
  onTryAdapter,
  onClose,
}: PackDialogProps) {
  const { dict, locale } = useI18n();
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
          <h2 className="flex-1 text-sm font-bold text-ink">{s.packTitle}</h2>
          <button
            type="button"
            onClick={onClose}
            className="min-h-[44px] rounded-full border border-line px-3 text-xs font-bold"
          >
            {s.close}
          </button>
        </div>

        {/* 本站导出：把当前分类学 / 外观打成包文件 */}
        <div className="mb-3 flex flex-wrap items-center gap-2">
          <button
            type="button"
            disabled={busy}
            onClick={() => onExportKind("taxonomy")}
            className="min-h-[44px] rounded-full border border-line px-4 text-xs font-bold text-ink disabled:opacity-40"
          >
            {s.packExportTaxonomy}
          </button>
          <button
            type="button"
            disabled={busy}
            onClick={() => onExportKind("theme")}
            className="min-h-[44px] rounded-full border border-line px-4 text-xs font-bold text-ink disabled:opacity-40"
          >
            {s.packExportTheme}
          </button>
        </div>

        <p className="mb-1 text-[11px] text-sub">{s.packImportHint}</p>
        <input
          type="file"
          accept="application/json"
          className="mb-2 block w-full text-xs"
          onChange={(e) => onPickFile(e.target.files?.[0] ?? null)}
        />
        <textarea
          className="mb-2 h-32 w-full resize-y rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-2 font-mono text-[11px]"
          placeholder='{ "format": "fluxtorrent.contentpack", … }'
          value={text}
          onChange={(e) => onText(e.target.value)}
          spellCheck={false}
        />

        {preview && (
          <div className="mb-3 rounded-[var(--r-md)] border border-line bg-sky-soft p-3 text-xs">
            <p className="font-bold text-ink">
              {preview.kind === "taxonomy"
                ? s.packPreviewTaxonomy
                : s.packPreviewTheme}
            </p>
            {preview.kind === "taxonomy" ? (
              <p className="mt-1 text-sub">
                {fmt(s.packPreviewCats, {
                  cur: preview.current_categories ?? 0,
                  n: preview.pack_categories ?? 0,
                })}
              </p>
            ) : (
              <p className="mt-1 text-sub">
                {fmt(s.packPreviewKeys, { n: preview.will_change ?? 0 })}
              </p>
            )}
            {(preview.errors?.length ?? 0) > 0 && (
              <ul className="mt-1 list-disc pl-5 text-danger">
                {preview.errors?.map((e) => (
                  <li key={e.name}>
                    {e.name}: {e.message}
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}

        <div className="mb-4 flex flex-wrap gap-2">
          <button
            type="button"
            disabled={busy || !text.trim()}
            onClick={onDryRun}
            className="min-h-[44px] rounded-full border border-line px-4 text-xs font-bold text-ink disabled:opacity-40"
          >
            {s.importPreview}
          </button>
          <button
            type="button"
            disabled={busy || !preview || preview.dry_run !== true}
            onClick={onApply}
            className="min-h-[44px] rounded-full bg-danger px-4 text-xs font-bold text-white disabled:opacity-40"
          >
            {s.importConfirm}
          </button>
        </div>

        {/* 商店目录（M2）：内置 + 远程条目，一键安装 */}
        <h3 className="mb-2 text-xs font-bold text-ink">{s.packCatalog}</h3>
        {catalog?.degraded && (
          <p className="mb-2 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-2 text-[11px] text-sub">
            {s.packDegraded}
          </p>
        )}
        {catalog === null && (
          <p className="py-2 text-center text-xs text-sub">…</p>
        )}
        {catalog?.items.length === 0 && (
          <p className="py-2 text-center text-xs text-sub">{s.packEmpty}</p>
        )}
        {catalog && catalog.items.length > 0 && (
          <ul className="mb-4 grid grid-cols-1 gap-2 sm:grid-cols-2">
            {catalog.items.map((it: CatalogItem) => (
              <li
                key={it.pack_id}
                className="rounded-[var(--r-md)] border border-line p-2.5 text-xs"
              >
                <div className="flex items-center gap-1.5">
                  <span className="truncate font-bold text-ink">{it.name}</span>
                  {it.installed_row_id != null && (
                    <span className="shrink-0 rounded-full bg-mint/20 px-2 py-0.5 text-[10px] font-bold text-ink">
                      {s.packInstalledBadge}
                    </span>
                  )}
                  <span className="flex-1" />
                  <span className="shrink-0 rounded-full bg-sky-soft px-2 py-0.5 text-[10px] font-bold text-sub">
                    {it.source === "builtin" ? s.packSourceBuiltin : s.packSourceRemote}
                  </span>
                </div>
                <p className="mt-1 line-clamp-2 text-[11px] text-sub">
                  {it.description}
                </p>
                <div className="mt-2 flex items-center gap-2">
                  <span className="font-mono text-[10px] text-sub">
                    {it.pack_id} · v{it.version}
                  </span>
                  <span className="flex-1" />
                  {it.installed_row_id == null ? (
                    <button
                      type="button"
                      disabled={busy || installBusyId === it.pack_id}
                      onClick={() => onInstall(it.pack_id)}
                      className="min-h-[36px] rounded-full bg-sky-deep px-3 text-[11px] font-bold text-white disabled:opacity-40"
                    >
                      {installBusyId === it.pack_id
                        ? s.packInstalling
                        : s.packInstall}
                    </button>
                  ) : (
                    <button
                      type="button"
                      disabled={busy}
                      onClick={() => onRollback(it.installed_row_id!)}
                      className="min-h-[36px] rounded-full border border-danger/40 px-3 text-[11px] font-bold text-danger disabled:opacity-40"
                    >
                      {s.packRollback}
                    </button>
                  )}
                </div>
              </li>
            ))}
          </ul>
        )}

        {/* 规则试算（M3）：lint + 变量代入求值 */}
        <RuleTrySection ruleTryResult={ruleTryResult} onTryRule={onTryRule} />

        {/* 适配器管理（M4）：沙箱插件启停/健康度/试调 */}
        <AdapterSection
          adapters={adapters}
          adapterTryId={adapterTryId}
          onToggleAdapter={onToggleAdapter}
          onDeleteAdapter={onDeleteAdapter}
          onTryAdapter={onTryAdapter}
        />

        {/* 已装包清单 + 回滚（A2：禁用即还原） */}
        <h3 className="mb-2 text-xs font-bold text-ink">{s.packInstalled}</h3>
        {rows === null && (
          <p className="py-2 text-center text-xs text-sub">…</p>
        )}
        {rows?.length === 0 && (
          <p className="py-2 text-center text-xs text-sub">{s.packEmpty}</p>
        )}
        {rows && rows.length > 0 && (
          <ul className="flex flex-col divide-y divide-line">
            {rows.map((r) => (
              <li key={r.id} className="flex items-center gap-2 py-2 text-xs">
                <div className="min-w-0 flex-1">
                  <p className="truncate font-bold text-ink">
                    {r.name}{" "}
                    <span className="font-mono text-[10px] text-sub">
                      {r.pack_id} · {r.version}
                    </span>
                  </p>
                  <p className="text-sub">
                    {r.kind} ·{" "}
                    {new Date(r.applied_at).toLocaleString(
                      dateLocale(locale),
                    )}
                  </p>
                </div>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => onRollback(r.id)}
                  className="min-h-[36px] shrink-0 rounded-full border border-danger/40 px-3 text-[11px] font-bold text-danger disabled:opacity-40"
                >
                  {s.packRollback}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
