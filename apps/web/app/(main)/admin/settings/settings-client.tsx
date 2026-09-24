"use client";

import { PANEL_CENTER } from "@/lib/ui-classes";

/**
 * 站点设定（方案 §5）：从 /admin 页签升级为独立路由 /admin/settings。
 * 左侧 12 分区导航 + 字段搜索，右侧按卡片分组渲染类型化字段；
 * 右上角「保存全部修改」+ Ctrl/Cmd+S + 未保存离开提醒；administrator 只读模式。
 * 顶部操作条/提示条拆至 ./_parts/settings-toolbar.tsx；
 * 分组导航/搜索结果/卡片分组拆至 ./_parts/settings-panels.tsx；
 * 历史抽屉/导入对话框拆至 ./_parts/settings-dialogs.tsx；
 * 导出/导入域拆至 ./_parts/settings-io.ts；
 * 保存域（dirty/快捷键/归并提交）拆至 ./_parts/settings-save.ts；
 * 数据类型拆至 ./_parts/settings-types.ts。
 */

import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";
import {
  SettingField,
  type SettingFieldMeta,
  type SettingsSchema,
} from "@/components/setting-field";
import { SettingsHistoryDrawer, SettingsImportDialog } from "./_parts/settings-dialogs";
import { ContentPackDialog } from "./_parts/settings-pack-dialog";
import {
  SettingsGroupCards,
  SettingsGroupNav,
  SettingsSearchResults,
} from "./_parts/settings-panels";
import { useSettingsTransfer, type SettingsToast } from "./_parts/settings-io";
import { useSettingsSave } from "./_parts/settings-save";
import { SettingsToolbar } from "./_parts/settings-toolbar";
import type { HistoryRow } from "./_parts/settings-types";

export function SettingsClient({ initialSchema }: { initialSchema: SettingsSchema | null }) {
  const { dict } = useI18n();
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
  const [active, setActive] = useState(initialSchema?.groups[0]?.key ?? "");
  const [q, setQ] = useState("");
  const [toast, setToast] = useState<SettingsToast | null>(null);
  const [drawer, setDrawer] = useState<{ name: string; rows: HistoryRow[] } | null>(null);

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

  const editable = schema?.editable ?? false;

  const save = useSettingsSave({
    edited,
    original,
    fieldGroup,
    editable,
    dict,
    invalidMsg: s.invalidField,
    savedMsg: s.savedToast,
    reload: load,
    onToast: setToast,
  });
  const { errors, setErrors } = save;

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

  const io = useSettingsTransfer({ reload: load, onToast: setToast });

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
      <SettingsToolbar
        schema={schema}
        lastSaved={save.lastSaved}
        dirtyCount={save.dirty.length}
        saving={save.saving}
        editable={editable}
        toast={toast}
        io={io}
        onSave={save.saveAll}
        onDiscard={() => setEdited({})}
      />

      <div className="flex flex-col gap-4 md:flex-row md:items-start">
        {/* 左导航（≤768px 变横向滚动 Chip 条） */}
        <SettingsGroupNav
          schema={schema}
          active={active}
          searching={searching}
          q={q}
          onQ={setQ}
          onActive={setActive}
        />

        {/* 右表单 */}
        <div className="flex min-w-0 flex-1 flex-col gap-3">
          <input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder={s.search}
            className="min-h-[44px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-3 text-sm md:hidden"
          />

          {searching ? (
            <SettingsSearchResults matches={matches} renderField={renderField} />
          ) : activeGroup ? (
            <SettingsGroupCards cards={activeGroup.cards} renderField={renderField} />
          ) : (
            !schema && (
              <p className={PANEL_CENTER}>
                {s.loadFailed}
              </p>
            )
          )}
        </div>
      </div>

      {/* 导入对话框：先空跑出 diff，再强制确认落库 */}
      {io.importOpen && (
        <SettingsImportDialog
          open={io.importOpen}
          busy={io.importBusy}
          text={io.importText}
          unknown={io.importUnknown}
          skipped={io.importSkipped}
          diff={io.importDiff}
          onText={(v) => {
            io.setImportText(v);
            io.setImportDiff(null);
          }}
          onPickFile={io.onPickFile}
          onDryRun={io.importDryRun}
          onApply={io.importApply}
          onClose={() => io.setImportOpen(false)}
        />
      )}

      {/* 内容包对话框（生态商店 M1/M2/M3）：目录/规则/导出/导入/清单/回滚 */}
      {io.packOpen && (
        <ContentPackDialog
          busy={io.packBusy}
          text={io.packText}
          preview={io.packPreview}
          rows={io.packRows}
          catalog={io.catalog}
          installBusyId={io.installBusyId}
          ruleTryResult={io.ruleTryResult}
          onText={(v) => {
            io.setPackText(v);
            io.setPackPreview(null);
          }}
          onPickFile={io.onPickPackFile}
          onExportKind={io.packExportKind}
          onDryRun={io.packDryRun}
          onApply={io.packApply}
          onRollback={io.packRollback}
          onInstall={io.installFromCatalog}
          onTryRule={io.tryRule}
          onClose={() => io.setPackOpen(false)}
        />
      )}

      {/* 修改历史抽屉 */}
      {drawer && (
        <SettingsHistoryDrawer
          name={drawer.name}
          rows={drawer.rows}
          onClose={() => setDrawer(null)}
        />
      )}
    </div>
  );
}
