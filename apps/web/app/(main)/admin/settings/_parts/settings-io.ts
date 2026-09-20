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
  DiffRow,
  ExportPayload,
  ImportApplied,
  ImportDryRun,
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
      const blob = new Blob([JSON.stringify(r, null, 2)], { type: "application/json" });
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
      onToast({ ok: true, text: fmt(s.imported, { n: r.applied }), effects: r.effects });
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
  };
}
