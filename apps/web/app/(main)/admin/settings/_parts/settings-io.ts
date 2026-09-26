"use client";

/**
 * 站点设定导出/导入域（P2 §4.3，从 app/(main)/admin/settings/settings-client.tsx
 * 按域拆出）：明文导出二次确认、导出下载、导入解析 / 空跑 diff / 强制确认落库
 * 的状态与动作。提示与刷新通过回调交还主组件。
 */

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import type {
  AdapterRow,
  AdapterTryResult,
  CatalogResponse,
  ContentPackFile,
  ContentPackRow,
  DiffRow,
  ExportPayload,
  ImportApplied,
  ImportDryRun,
  PackImportResult,
  RuleTryResult,
} from "./settings-types";

/** 主组件的提示条状态形状（成功/失败文案 + 生效列表） */
export interface SettingsToast {
  ok: boolean;
  text: string;
  effects?: string[];
}

export function useSettingsTransfer({
  reload,
  onToast,
}: {
  reload: () => Promise<void>;
  onToast: (t: SettingsToast | null) => void;
}) {
  const { dict } = useI18n();
  const s = dict.settingsAdmin;
  // 导出 / 导入（P2 §4.3）
  const [exportPlain, setExportPlain] = useState(false);
  const [askPlain, setAskPlain] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const [importText, setImportText] = useState("");
  const [importUnknown, setImportUnknown] = useState<string[]>([]);
  const [importSkipped, setImportSkipped] = useState<string[]>([]);
  const [importDiff, setImportDiff] = useState<DiffRow[] | null>(null);
  const [importBusy, setImportBusy] = useState(false);

  // ---- 导出（密文默认掩码；明文导出需二次确认且仅 sysop） ----
  async function doExport() {
    try {
      const r = await api.get<ExportPayload>(
        `/api/v1/admin/settings/export${exportPlain ? "?plaintext=1" : ""}`,
      );
      const blob = new Blob([JSON.stringify(r, null, 2)], {
        type: "application/json",
      });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `fluxtorrent-settings-${new Date().toISOString().slice(0, 10)}.json`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(url);
      onToast({ ok: true, text: fmt(s.exported, { n: r.count }), effects: [] });
    } catch (e) {
      onToast({ ok: false, text: apiErrorMessage(dict, e) });
    }
  }

  /** 兼容「导出文件原样导入」与「仅 settings 映射」两种输入。
   *  `plaintext` 标记原样透传后端，决定密文空值是"保持不变"还是"字面还原"：
   *  明文快照里空密钥应当被还原为空，掩码快照里空密钥应当保持原值。 */
  function parseImport(
    raw: string,
  ): { settings: Record<string, string>; plaintext: boolean } | null {
    try {
      const j = JSON.parse(raw);
      const hasWrap = j && typeof j === "object" && "settings" in j;
      const settings = hasWrap ? j.settings : j;
      if (!settings || typeof settings !== "object") return null;
      const out: Record<string, string> = {};
      for (const [k, v] of Object.entries(
        settings as Record<string, unknown>,
      )) {
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
      onToast({ ok: false, text: s.importBadJson });
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
      onToast({ ok: false, text: apiErrorMessage(dict, e) });
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
      onToast({
        ok: true,
        text: fmt(s.imported, { n: r.applied }),
        effects: r.effects,
      });
      setImportOpen(false);
      setImportText("");
      setImportDiff(null);
      setImportUnknown([]);
      setImportSkipped([]);
      setExportPlain(false);
      await reload();
    } catch (e) {
      onToast({ ok: false, text: apiErrorMessage(dict, e) });
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

  /** 内容包文件选择（与设定导入同一读取方式） */
  function onPickPackFile(file: File | null) {
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      setPackText(String(reader.result ?? ""));
      setPackPreview(null);
    };
    reader.readAsText(file, "utf-8");
  }

  // ---- 商店目录（M2）：浏览 / 一键安装 / 已装对照 ----
  const [catalog, setCatalog] = useState<CatalogResponse | null>(null);
  const [installBusyId, setInstallBusyId] = useState<string | null>(null);

  async function reloadCatalog() {
    try {
      setCatalog(
        await api.get<CatalogResponse>("/api/v1/admin/content-packs/catalog"),
      );
    } catch {
      setCatalog(null);
    }
  }

  async function installFromCatalog(packId: string) {
    setInstallBusyId(packId);
    try {
      const r = await api.post<PackImportResult>(
        "/api/v1/admin/content-packs/install",
        { pack_id: packId, confirm: true },
      );
      const n = r.applied?.categories ?? r.applied?.settings ?? 0;
      onToast({ ok: true, text: fmt(s.packImported, { n }) });
      await Promise.all([reloadPacks(), reloadCatalog(), reload()]);
    } catch (e) {
      onToast({ ok: false, text: apiErrorMessage(dict, e) });
    } finally {
      setInstallBusyId(null);
    }
  }

  // ---- 规则试算（M3）：表达式 lint + 变量代入，不落库 ----
  const [ruleTryResult, setRuleTryResult] = useState<RuleTryResult | null>(
    null,
  );

  async function tryRule(key: string, expr: string, termDays: number) {
    setRuleTryResult(null);
    try {
      const r = await api.post<RuleTryResult>(
        "/api/v1/admin/content-packs/rule-try",
        { key, expr, vars: { term_days: termDays } },
      );
      setRuleTryResult(r);
    } catch (e) {
      onToast({ ok: false, text: apiErrorMessage(dict, e) });
    }
  }

  // ---- 适配器管理（M4）：列表 / 启停 / 试调 ----
  const [adapters, setAdapters] = useState<AdapterRow[] | null>(null);
  const [adapterTryId, setAdapterTryId] = useState<string | null>(null);

  async function reloadAdapters() {
    try {
      setAdapters(await api.get<AdapterRow[]>("/api/v1/admin/adapters"));
    } catch {
      setAdapters([]);
    }
  }

  async function toggleAdapter(adapterId: string, enabled: boolean) {
    try {
      await api.post("/api/v1/admin/adapters/toggle", {
        adapter_id: adapterId,
        enabled,
      });
      onToast({
        ok: true,
        text: fmt(s.adapterToggled, { id: adapterId, state: enabled ? s.adapterOn : s.adapterOff }),
      });
      await reloadAdapters();
    } catch (e) {
      onToast({ ok: false, text: apiErrorMessage(dict, e) });
    }
  }

  async function deleteAdapter(adapterId: string) {
    try {
      await api.post("/api/v1/admin/adapters/delete", {
        adapter_id: adapterId,
      });
      onToast({ ok: true, text: fmt(s.adapterDeleted, { id: adapterId }) });
      await reloadAdapters();
    } catch (e) {
      onToast({ ok: false, text: apiErrorMessage(dict, e) });
    }
  }

  async function tryAdapter(adapterId: string, url: string) {
    setAdapterTryId(adapterId);
    try {
      const r = await api.post<AdapterTryResult>(
        "/api/v1/admin/adapters/try",
        { adapter_id: adapterId, url },
      );
      onToast({
        ok: true,
        text: fmt(s.adapterTryOk, { id: adapterId }),
        effects: [JSON.stringify(r.result).slice(0, 200)],
      });
      await reloadAdapters();
    } catch (e) {
      onToast({ ok: false, text: apiErrorMessage(dict, e) });
      await reloadAdapters(); // 熔断可能已更新
    } finally {
      setAdapterTryId(null);
    }
  }

  // ---- 内容包（生态商店 M1）：本站导出 / 导入（空跑→确认）/ 清单 / 回滚 ----
  const [packOpen, setPackOpen] = useState(false);
  const [packText, setPackText] = useState("");
  const [packBusy, setPackBusy] = useState(false);
  const [packPreview, setPackPreview] = useState<PackImportResult | null>(
    null,
  );
  const [packRows, setPackRows] = useState<ContentPackRow[] | null>(null);

  async function reloadPacks() {
    try {
      setPackRows(await api.get<ContentPackRow[]>("/api/v1/admin/content-packs"));
    } catch {
      setPackRows([]);
    }
  }

  function openPacks() {
    setPackOpen(true);
    if (packRows === null) void reloadPacks();
    if (catalog === null) void reloadCatalog();
    if (adapters === null) void reloadAdapters();
  }

  async function packExportKind(kind: "taxonomy" | "theme") {
    try {
      const r = await api.get<ContentPackFile>(
        `/api/v1/admin/content-packs/export?kind=${kind}`,
      );
      const blob = new Blob([JSON.stringify(r, null, 2)], {
        type: "application/json",
      });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `fluxtorrent-${kind}-pack-${new Date()
        .toISOString()
        .slice(0, 10)}.json`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(url);
      onToast({ ok: true, text: fmt(s.packExported, { kind }), effects: [] });
    } catch (e) {
      onToast({ ok: false, text: apiErrorMessage(dict, e) });
    }
  }

  async function packDryRun() {
    let pack: unknown;
    try {
      pack = JSON.parse(packText);
    } catch {
      onToast({ ok: false, text: s.importBadJson });
      return;
    }
    setPackBusy(true);
    try {
      const r = await api.post<PackImportResult>(
        "/api/v1/admin/content-packs/import",
        { pack, confirm: false },
      );
      setPackPreview(r);
      if ((r.errors?.length ?? 0) > 0) {
        onToast({ ok: false, text: s.packHasErrors });
      }
    } catch (e) {
      onToast({ ok: false, text: apiErrorMessage(dict, e) });
    } finally {
      setPackBusy(false);
    }
  }

  async function packApply() {
    let pack: unknown;
    try {
      pack = JSON.parse(packText);
    } catch {
      return;
    }
    setPackBusy(true);
    try {
      const r = await api.post<PackImportResult>(
        "/api/v1/admin/content-packs/import",
        { pack, confirm: true },
      );
      const n = r.applied?.categories ?? r.applied?.settings ?? 0;
      onToast({ ok: true, text: fmt(s.packImported, { n }) });
      setPackText("");
      setPackPreview(null);
      await Promise.all([reloadPacks(), reload()]);
    } catch (e) {
      onToast({ ok: false, text: apiErrorMessage(dict, e) });
    } finally {
      setPackBusy(false);
    }
  }

  async function packRollback(id: number) {
    setPackBusy(true);
    try {
      const r = await api.post<{ rolled_back: string }>(
        "/api/v1/admin/content-packs/rollback",
        { id },
      );
      onToast({ ok: true, text: fmt(s.packRolledBack, { id: r.rolled_back }) });
      await Promise.all([reloadPacks(), reload()]);
    } catch (e) {
      onToast({ ok: false, text: apiErrorMessage(dict, e) });
    } finally {
      setPackBusy(false);
    }
  }

  return {
    exportPlain,
    setExportPlain,
    askPlain,
    setAskPlain,
    importOpen,
    setImportOpen,
    importText,
    setImportText,
    importUnknown,
    importSkipped,
    importDiff,
    setImportDiff,
    importBusy,
    doExport,
    importDryRun,
    importApply,
    onPickFile,
    // 内容包域（M1）
    packOpen,
    setPackOpen,
    openPacks,
    packText,
    setPackText,
    packBusy,
    packPreview,
    setPackPreview,
    packRows,
    packExportKind,
    packDryRun,
    packApply,
    packRollback,
    onPickPackFile,
    // 商店目录域（M2）
    catalog,
    reloadCatalog,
    installBusyId,
    installFromCatalog,
    // 规则试算域（M3）
    ruleTryResult,
    setRuleTryResult,
    tryRule,
    // 适配器域（M4）
    adapters,
    reloadAdapters,
    toggleAdapter,
    deleteAdapter,
    tryAdapter,
    adapterTryId,
  };
}
