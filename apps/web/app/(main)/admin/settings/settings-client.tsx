"use client";

/**
 * 站点设定（方案 §5）：从 /admin 页签升级为独立路由 /admin/settings。
 * 左侧 12 分区导航 + 字段搜索，右侧按卡片分组渲染类型化字段；
 * 右上角「保存全部修改」+ Ctrl/Cmd+S + 未保存离开提醒；administrator 只读模式。
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import {
  SettingField,
  type SettingFieldMeta,
  type SettingsSchema,
} from "@/components/setting-field";

interface HistoryRow {
  id: number;
  action: string;
  actor: string | null;
  actor_id: number | null;
  detail: { setting?: string; group?: string; old?: string; new?: string; via?: string } | null;
  created_at: string;
}

interface SaveResult {
  group: string;
  saved: number;
  changed: string[];
  cache: string;
  effects: string[];
}

interface ExportPayload {
  format: string;
  version: number;
  plaintext: boolean;
  count: number;
  masked_secrets: string[];
  settings: Record<string, string>;
}

interface DiffRow {
  name: string;
  group: string;
  old: string;
  new: string;
}

interface ImportDryRun {
  dry_run: true;
  unknown: string[];
  skipped_readonly: string[];
  will_change: number;
  diff: DiffRow[];
  effects: string[];
}

interface ImportApplied {
  dry_run: false;
  unknown: string[];
  skipped_readonly: string[];
  applied: number;
  changed: string[];
  groups: string[];
  effects: string[];
}

export function SettingsClient({ initialSchema }: { initialSchema: SettingsSchema | null }) {
  const { dict, locale } = useI18n();
  const s = dict.settingsAdmin;

  // 服务端预取时同步推导首屏所需的扁平值 / 分区归属 —— useEffect 不参与 SSR，
  // 派生状态必须在这里算好，字段才能进入首屏 HTML。
  const seeded = useMemo(() => {
    const flat: Record<string, string> = {};
    const owner: Record<string, string> = {};
    if (initialSchema) {
      for (const g of initialSchema.groups) {
        for (const c of g.cards) {
          for (const f of c.fields) {
            flat[f.name] = f.value;
            owner[f.name] = g.key;
          }
        }
      }
    }
    return { flat, owner };
  }, [initialSchema]);

  const [schema, setSchema] = useState<SettingsSchema | null>(initialSchema);
  const [original, setOriginal] = useState<Record<string, string>>(seeded.flat);
  const [fieldGroup, setFieldGroup] = useState<Record<string, string>>(seeded.owner);
  const [edited, setEdited] = useState<Record<string, string>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [active, setActive] = useState(initialSchema?.groups[0]?.key ?? "");
  const [q, setQ] = useState("");
  const [saving, setSaving] = useState(false);
  const [toast, setToast] = useState<{ ok: boolean; text: string; effects?: string[] } | null>(
    null,
  );
  const [lastSaved, setLastSaved] = useState<Date | null>(null);
  const [drawer, setDrawer] = useState<{ name: string; rows: HistoryRow[] } | null>(null);
  // 导出 / 导入（P2 §4.3）
  const [exportPlain, setExportPlain] = useState(false);
  const [askPlain, setAskPlain] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const [importText, setImportText] = useState("");
  const [importUnknown, setImportUnknown] = useState<string[]>([]);
  const [importSkipped, setImportSkipped] = useState<string[]>([]);
  const [importDiff, setImportDiff] = useState<DiffRow[] | null>(null);
  const [importBusy, setImportBusy] = useState(false);

  /** 应用一份 schema：服务端预取（首屏）与客户端兜底重试共用同一条落地逻辑 */
  const applySchema = useCallback((data: SettingsSchema) => {
    const flat: Record<string, string> = {};
    const owner: Record<string, string> = {};
    for (const g of data.groups) {
      for (const c of g.cards) {
        for (const f of c.fields) {
          flat[f.name] = f.value;
          owner[f.name] = g.key;
        }
      }
    }
    setSchema(data);
    setOriginal(flat);
    setFieldGroup(owner);
    setEdited({});
    setErrors({});
    setActive((prev) => {
      if (prev && data.groups.some((g) => g.key === prev)) return prev;
      const fromUrl =
        typeof window === "undefined"
          ? ""
          : new URLSearchParams(window.location.search).get("group") ?? "";
      if (fromUrl && data.groups.some((g) => g.key === fromUrl)) return fromUrl;
      return data.groups[0]?.key ?? "";
    });
    if (typeof window !== "undefined") {
      const fromUrl = new URLSearchParams(window.location.search).get("q");
      if (fromUrl) setQ(fromUrl);
    }
  }, []);

  const load = useCallback(async () => {
    try {
      const data = await api.get<SettingsSchema>("/api/v1/admin/settings/schema");
      applySchema(data);
    } catch (e) {
      setToast({ ok: false, text: apiErrorMessage(dict, e) || s.loadFailed });
    }
  }, [applySchema, dict, s.loadFailed]);

  // RSC 已预取 schema：首屏由服务端渲染，这里只把 ?group=/?q= 深链参数应用上去；
  // 仅当预取失败（initialSchema 为 null）才回退到浏览器侧重试。
  useEffect(() => {
    if (!initialSchema) {
      load();
      return;
    }
    const sp = new URLSearchParams(window.location.search);
    const g = sp.get("group") ?? "";
    if (g && initialSchema.groups.some((x) => x.key === g)) setActive(g);
    const q = sp.get("q");
    if (q) setQ(q);
  }, [initialSchema, load]);

  // 未保存计数
  const dirty = useMemo(
    () =>
      Object.entries(edited).filter(([k, v]) => v !== (original[k] ?? "")).map(([k]) => k),
    [edited, original],
  );
  const dirtyRef = useRef(0);
  dirtyRef.current = dirty.length;

  // Ctrl/Cmd+S 保存
  const saveAllRef = useRef<() => void>(() => {});
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        saveAllRef.current();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // 离开页面未保存提醒
  useEffect(() => {
    function onBeforeUnload(e: BeforeUnloadEvent) {
      if (dirtyRef.current === 0) return;
      e.preventDefault();
      e.returnValue = "";
    }
    window.addEventListener("beforeunload", onBeforeUnload);
    return () => window.removeEventListener("beforeunload", onBeforeUnload);
  }, []);

  const editable = schema?.editable ?? false;

  const saveAll = useCallback(async () => {
    if (dirty.length === 0 || saving || !editable) return;
    // 按分区归并（后端接口以分区为单位）
    const byGroup = new Map<string, Record<string, string>>();
    for (const name of dirty) {
      const g = fieldGroup[name] ?? "misc";
      const bucket = byGroup.get(g) ?? {};
      bucket[name] = edited[name];
      byGroup.set(g, bucket);
    }
    setSaving(true);
    setErrors({});
    let saved = 0;
    const effects: string[] = [];
    const errs: Record<string, string> = {};
    try {
      for (const [group, values] of byGroup) {
        try {
          const r = await api.put<SaveResult>("/api/v1/admin/settings/groups", {
            group,
            values,
          });
          saved += r.saved;
          effects.push(...r.effects);
        } catch (e) {
          if (e instanceof ApiError && e.code === 1002 && Array.isArray(e.data)) {
            for (const item of e.data as { field: string; error: string }[]) {
              errs[item.field] = item.error;
            }
          } else {
            throw e;
          }
        }
      }
      if (Object.keys(errs).length > 0) {
        setErrors(errs);
        setToast({ ok: false, text: fmt(s.invalidField, { n: Object.keys(errs).length }) });
      } else {
        setToast({ ok: true, text: fmt(s.savedToast, { n: saved }), effects });
        setLastSaved(new Date());
        await load();
      }
    } catch (e) {
      setToast({ ok: false, text: apiErrorMessage(dict, e) });
    } finally {
      setSaving(false);
    }
  }, [dirty, saving, editable, fieldGroup, edited, s.invalidField, s.savedToast, dict, load]);

  saveAllRef.current = saveAll;

  async function openHistory(name: string) {
    try {
      const r = await api.get<{ rows: HistoryRow[] }>(
        `/api/v1/admin/settings/history?name=${encodeURIComponent(name)}`,
      );
      setDrawer({ name, rows: r.rows });
    } catch (e) {
      setToast({ ok: false, text: apiErrorMessage(dict, e) });
    }
  }

  // ---- 导出（密文默认掩码；明文导出需二次确认且仅 sysop） ----
  async function doExport() {
    try {
      const r = await api.get<ExportPayload>(
        `/api/v1/admin/settings/export${exportPlain ? "?plaintext=1" : ""}`,
      );
      const blob = new Blob([JSON.stringify(r, null, 2)], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `fluxtorrent-settings-${new Date().toISOString().slice(0, 10)}.json`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(url);
      setToast({ ok: true, text: fmt(s.exported, { n: r.count }), effects: [] });
    } catch (e) {
      setToast({ ok: false, text: apiErrorMessage(dict, e) });
    }
  }

  /** 兼容「导出文件原样导入」与「仅 settings 映射」两种输入。
   *  `plaintext` 标记原样透传后端，决定密文空值是"保持不变"还是"字面还原"：
   *  明文快照里空密钥应当被还原为空，掩码快照里空密钥应当保持原值。 */
  function parseImport(raw: string): { settings: Record<string, string>; plaintext: boolean } | null {
    try {
      const j = JSON.parse(raw);
      const hasWrap = j && typeof j === "object" && "settings" in j;
      const settings = hasWrap ? j.settings : j;
      if (!settings || typeof settings !== "object") return null;
      const out: Record<string, string> = {};
      for (const [k, v] of Object.entries(settings as Record<string, unknown>)) {
        out[k] = v == null ? "" : String(v);
      }
      return { settings: out, plaintext: hasWrap && j.plaintext === true };
    } catch {
      return null;
    }
  }

  // ---- 导入：先空跑出 diff，再强制确认落库 ----
  async function importDryRun() {
    const parsed = parseImport(importText);
    if (!parsed) {
      setToast({ ok: false, text: s.importBadJson });
      return;
    }
    setImportBusy(true);
    try {
      const r = await api.post<ImportDryRun>("/api/v1/admin/settings/import", {
        settings: parsed.settings,
        plaintext: parsed.plaintext,
        confirm: false,
      });
      setImportDiff(r.diff);
      setImportUnknown(r.unknown);
      setImportSkipped(r.skipped_readonly);
    } catch (e) {
      setToast({ ok: false, text: apiErrorMessage(dict, e) });
    } finally {
      setImportBusy(false);
    }
  }

  async function importApply() {
    const parsed = parseImport(importText);
    if (!parsed) return;
    setImportBusy(true);
    try {
      const r = await api.post<ImportApplied>("/api/v1/admin/settings/import", {
        settings: parsed.settings,
        plaintext: parsed.plaintext,
        confirm: true,
      });
      setToast({ ok: true, text: fmt(s.imported, { n: r.applied }), effects: r.effects });
      setImportOpen(false);
      setImportText("");
      setImportDiff(null);
      setImportUnknown([]);
      setImportSkipped([]);
      setExportPlain(false);
      await load();
    } catch (e) {
      setToast({ ok: false, text: apiErrorMessage(dict, e) });
    } finally {
      setImportBusy(false);
    }
  }

  function onPickFile(file: File | null) {
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      setImportText(String(reader.result ?? ""));
      setImportDiff(null);
      setImportUnknown([]);
      setImportSkipped([]);
    };
    reader.readAsText(file, "utf-8");
  }

  function setValue(name: string, v: string) {
    setEdited((prev) => ({ ...prev, [name]: v }));
    setErrors((prev) => {
      if (!(name in prev)) return prev;
      const next = { ...prev };
      delete next[name];
      return next;
    });
  }

  const groups = schema?.groups ?? [];
  const kw = q.trim().toLowerCase();
  const searching = kw.length > 0;

  const matches = useMemo(() => {
    if (!searching) return [] as { group: string; groupLabel: string; field: SettingFieldMeta }[];
    const out: { group: string; groupLabel: string; field: SettingFieldMeta }[] = [];
    const hit = (f: SettingFieldMeta, groupLabel: string) =>
      f.label.toLowerCase().includes(kw) ||
      (f.label_en ?? "").toLowerCase().includes(kw) ||
      f.name.toLowerCase().includes(kw) ||
      (f.hint ?? "").toLowerCase().includes(kw) ||
      groupLabel.toLowerCase().includes(kw);
    for (const g of groups) {
      for (const c of g.cards) {
        for (const f of c.fields) {
          if (hit(f, g.label)) out.push({ group: g.key, groupLabel: g.label, field: f });
        }
      }
    }
    return out;
  }, [searching, kw, groups]);

  function renderField(f: SettingFieldMeta) {
    return (
      <SettingField
        key={f.name}
        field={f}
        value={edited[f.name] ?? f.value}
        error={errors[f.name]}
        disabled={!editable}
        onChange={(v) => setValue(f.name, v)}
        onBlur={() => {}}
        onHistory={f.secret ? undefined : () => openHistory(f.name)}
      />
    );
  }

  const activeGroup = groups.find((g) => g.key === active);

  return (
    <div className="flex flex-col gap-4">
      {/* 顶部标题 + 全局操作条 */}
      <div className="sticky top-0 z-10 flex flex-wrap items-center gap-3 rounded-[var(--r-md)] border border-line bg-white/95 p-3 backdrop-blur shadow-[var(--shadow-card)]">
        <div>
          <h1 className="font-display text-xl">{s.title}</h1>
          <p className="text-[11px] text-sub">
            {schema ? `${schema.group_count} 分区 · ${schema.field_count} 字段` : "…"}
            {lastSaved && ` · ${fmt(s.lastSaved, { time: lastSaved.toLocaleTimeString(dateLocale(locale)) })}`}
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
                  checked={exportPlain}
                  onChange={(e) => {
                    if (e.target.checked) setAskPlain(true);
                    else setExportPlain(false);
                  }}
                />
                {s.exportPlain}
              </label>
            )}
            <button
              type="button"
              onClick={doExport}
              className="min-h-[44px] rounded-full border border-line bg-white px-4 text-xs font-bold text-ink"
            >
              {s.doExport}
            </button>
            {editable && (
              <button
                type="button"
                onClick={() => setImportOpen(true)}
                className="min-h-[44px] rounded-full border border-line bg-white px-4 text-xs font-bold text-ink"
              >
                {s.doImport}
              </button>
            )}
          </>
        )}
        {dirty.length > 0 && (
          <>
            <span className="rounded-full bg-sun/40 px-3 py-1 text-[11px] font-bold text-ink">
              {fmt(dict.settingsAdmin.unsavedTitle, { n: dirty.length })} · {dirty.length}
            </span>
            <button
              type="button"
              onClick={() => setEdited({})}
              className="min-h-[44px] rounded-full border border-line bg-white px-4 text-xs font-bold text-sub"
            >
              {s.discard}
            </button>
          </>
        )}
        {editable && (
          <button
            type="button"
            onClick={saveAll}
            disabled={saving || dirty.length === 0}
            className="min-h-[44px] rounded-full bg-sky-deep px-5 text-sm font-bold text-white disabled:opacity-40"
          >
            {saving
              ? s.saving
              : dirty.length > 0
                ? fmt(s.saveAllCount, { n: dirty.length })
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
      {askPlain && (
        <div className="flex flex-wrap items-center gap-2 rounded-[var(--r-md)] border border-danger/40 bg-danger/5 p-3">
          <p className="text-xs font-bold text-ink">{s.exportPlainConfirm}</p>
          <span className="flex-1" />
          <button
            type="button"
            onClick={() => {
              setExportPlain(true);
              setAskPlain(false);
            }}
            className="min-h-[44px] rounded-full bg-danger px-4 text-xs font-bold text-white"
          >
            {s.revealConfirmYes}
          </button>
          <button
            type="button"
            onClick={() => {
              setExportPlain(false);
              setAskPlain(false);
            }}
            className="min-h-[44px] rounded-full border border-line bg-white px-4 text-xs font-bold text-sub"
          >
            {s.cancel}
          </button>
        </div>
      )}

      {toast && (
        <div
          className={`rounded-[var(--r-md)] border p-3 text-sm ${
            toast.ok ? "border-mint/40 bg-mint/10 text-ink" : "border-danger/40 bg-danger/5 text-ink"
          }`}
          role="status"
        >
          <p className="font-bold">{toast.text}</p>
          {toast.ok && <p className="mt-0.5 text-[11px] text-sub">{s.cacheHint}</p>}
          {toast.effects && toast.effects.length > 0 && (
            <ul className="mt-2 list-disc pl-5 text-[11px] text-sub">
              {toast.effects.map((e) => (
                <li key={e}>{e}</li>
              ))}
            </ul>
          )}
        </div>
      )}

      <div className="flex flex-col gap-4 md:flex-row md:items-start">
        {/* 左导航（≤768px 变横向滚动 Chip 条） */}
        <nav className="flex gap-2 overflow-x-auto border border-line bg-white p-2 md:w-56 md:flex-none md:flex-col md:overflow-visible md:rounded-[var(--r-md)] md:shadow-[var(--shadow-card)]">
          <div className="hidden md:block">
            <input
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder={s.search}
              className="min-h-[44px] w-full rounded-[var(--r-sm)] border border-line bg-white px-3 text-xs"
            />
          </div>
          {groups.map((g) => (
            <button
              key={g.key}
              type="button"
              onClick={() => {
                setActive(g.key);
                if (typeof window !== "undefined") {
                  const u = new URL(window.location.href);
                  u.searchParams.set("group", g.key);
                  window.history.replaceState(null, "", u.toString());
                }
              }}
              aria-current={!searching && active === g.key}
              className={`flex min-h-[44px] flex-none items-center gap-2 rounded-[var(--r-sm)] px-3 text-xs font-bold md:w-full ${
                !searching && active === g.key
                  ? "bg-sky-deep text-white"
                  : "border border-line bg-white text-sub md:border-0 md:text-ink"
              }`}
            >
              <span className="whitespace-nowrap">{dict.admin.settingGroups[g.key] ?? g.label}</span>
              <span className="ml-auto text-[10px] font-normal">{g.count}</span>
            </button>
          ))}
        </nav>

        {/* 右表单 */}
        <div className="flex min-w-0 flex-1 flex-col gap-3">
          <input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder={s.search}
            className="min-h-[44px] rounded-[var(--r-sm)] border border-line bg-white px-3 text-sm md:hidden"
          />

          {searching ? (
            <section className="flex flex-col gap-3">
              <p className="text-xs text-sub">{fmt(s.searchHit, { n: matches.length })}</p>
              {matches.length === 0 && (
                <p className="rounded-[var(--r-md)] border border-line bg-white p-6 text-center text-sm text-sub">
                  {s.searchEmpty}
                </p>
              )}
              {matches.map((m) => (
                <div key={`${m.group}-${m.field.name}`}>
                  <p className="mb-1 text-[11px] font-bold text-sub">{m.groupLabel}</p>
                  {renderField(m.field)}
                </div>
              ))}
            </section>
          ) : activeGroup ? (
            <section className="flex flex-col gap-3">
              {activeGroup.cards.map((c, i) => (
                <div
                  key={c.key}
                  className="rounded-[var(--r-md)] border border-line bg-white p-3 shadow-[var(--shadow-card)]"
                >
                  <h2 className="mb-2 flex items-baseline gap-2 text-sm font-bold text-ink">
                    {c.key || fmt(dict.settingsAdmin.fieldCount, { n: i + 1 })}
                    <span className="text-[11px] font-normal text-sub">{c.fields.length}</span>
                  </h2>
                  <div className="flex flex-col gap-2">
                    {c.fields.map((f) => renderField(f))}
                  </div>
                </div>
              ))}
              {activeGroup.cards.length === 0 && (
                <p className="rounded-[var(--r-md)] border border-line bg-white p-6 text-center text-sm text-sub">
                  {s.searchEmpty}
                </p>
              )}
            </section>
          ) : (
            !schema && (
              <p className="rounded-[var(--r-md)] border border-line bg-white p-6 text-center text-sm text-sub">
                {s.loadFailed}
              </p>
            )
          )}
        </div>
      </div>

      {/* 导入对话框：先空跑出 diff，再强制确认落库 */}
      {importOpen && (
        <div
          className="fixed inset-0 z-30 flex items-center justify-center bg-black/30 p-3"
          role="dialog"
          aria-modal="true"
          onClick={() => setImportOpen(false)}
        >
          <div
            className="max-h-full w-full max-w-2xl overflow-y-auto rounded-[var(--r-md)] bg-white p-4 shadow-[var(--shadow-card)]"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="mb-3 flex items-center gap-2">
              <h2 className="flex-1 text-sm font-bold text-ink">{s.importTitle}</h2>
              <button
                type="button"
                onClick={() => setImportOpen(false)}
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
              value={importText}
              onChange={(e) => {
                setImportText(e.target.value);
                setImportDiff(null);
              }}
              rows={6}
              placeholder={s.importPlaceholder}
              className="w-full rounded-[var(--r-sm)] border border-line bg-white p-2 font-mono text-[11px] text-ink"
            />
            {importUnknown.length > 0 && (
              <p className="mt-2 rounded-[var(--r-sm)] border border-sun/60 bg-sun/10 p-2 text-[11px] text-ink">
                {fmt(s.importUnknown, { n: importUnknown.length })}：
                {importUnknown.slice(0, 12).join(", ")}
              </p>
            )}
            {importSkipped.length > 0 && (
              <p className="mt-2 rounded-[var(--r-sm)] border border-line bg-cloud p-2 text-[11px] text-sub">
                {fmt(s.importSkipped, { n: importSkipped.length })}：{importSkipped.join(", ")}
              </p>
            )}
            {importDiff && (
              <div className="mt-3">
                <p className="mb-1 text-xs font-bold text-ink">
                  {importDiff.length === 0
                    ? s.importNoChange
                    : fmt(s.importWillChange, { n: importDiff.length })}
                </p>
                {importDiff.length > 0 && (
                  <ul className="max-h-56 divide-y divide-line overflow-y-auto rounded-[var(--r-sm)] border border-line">
                    {importDiff.map((d) => (
                      <li key={d.name} className="p-2 text-[11px]">
                        <p className="flex flex-wrap items-baseline gap-1.5">
                          <span className="font-mono font-bold text-ink">{d.name}</span>
                          <span className="rounded-full bg-sky-soft px-1.5 py-0.5 text-[10px] text-sub">
                            {dict.admin.settingGroups[d.group] ?? d.group}
                          </span>
                        </p>
                        <p className="mt-0.5 break-all">
                          <span className="font-mono text-danger line-through">{d.old || "—"}</span>
                          <span className="mx-1 text-sky">→</span>
                          <span className="font-mono font-bold text-ink">{d.new || "—"}</span>
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
                onClick={importDryRun}
                disabled={importBusy || importText.trim() === ""}
                className="min-h-[44px] rounded-full border border-line bg-white px-4 text-xs font-bold text-ink disabled:opacity-40"
              >
                {s.importPreview}
              </button>
              <button
                type="button"
                onClick={importApply}
                disabled={importBusy || !importDiff || importDiff.length === 0}
                className="min-h-[44px] rounded-full bg-sky-deep px-5 text-xs font-bold text-white disabled:opacity-40"
              >
                {importBusy ? s.saving : s.importConfirm}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 修改历史抽屉 */}
      {drawer && (
        <div
          className="fixed inset-0 z-30 flex justify-end bg-black/30"
          role="dialog"
          aria-modal="true"
          onClick={() => setDrawer(null)}
        >
          <div
            className="h-full w-full max-w-md overflow-y-auto bg-white p-4 shadow-[var(--shadow-card)]"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="mb-3 flex items-center gap-2">
              <h2 className="flex-1 text-sm font-bold text-ink">
                {fmt(s.historyTitle, { name: drawer.name })}
              </h2>
              <button
                type="button"
                onClick={() => setDrawer(null)}
                className="min-h-[44px] rounded-full border border-line px-3 text-xs font-bold"
              >
                {s.close}
              </button>
            </div>
            {drawer.rows.length === 0 && <p className="py-6 text-center text-sm text-sub">{s.historyEmpty}</p>}
            <ul className="flex flex-col divide-y divide-line">
              {drawer.rows.map((r) => (
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
                      <span className="font-mono font-bold text-ink">{r.detail?.new ?? "—"}</span>
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
      )}
    </div>
  );
}
