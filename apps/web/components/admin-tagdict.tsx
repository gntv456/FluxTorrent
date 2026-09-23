"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

import { TagEditForm } from "./admin-tagdict-form";
import {
  EMPTY,
  PREVIEW_SHELL,
  type ModeRow,
  type TagRow,
} from "./admin-tagdict-shared";

/** 第八轮 P1-2：标签管理（好学站 torrent/tags 口径）
 *  字典 CRUD + 样式属性（背景/字体色/字号/边距/圆角）+ 分类模式作用域。
 *  新建/编辑表单拆至 ./admin-tagdict-form.tsx；
 *  类型与常量拆至 ./admin-tagdict-shared.ts。 */

export { EMPTY, PREVIEW_SHELL } from "./admin-tagdict-shared";
export type { TagRow, ModeRow } from "./admin-tagdict-shared";

export function AdminTagDict() {
  const { dict } = useI18n();
  const at = dict.adminTagdict;
  const [rows, setRows] = useState<TagRow[]>([]);
  const [modes, setModes] = useState<ModeRow[]>([]);
  const [edit, setEdit] = useState<{ id: number | null; f: typeof EMPTY }>({
    id: null,
    f: { ...EMPTY },
  });
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  };

  const load = useCallback(async () => {
    try {
      setRows(await api.get<TagRow[]>("/api/v1/admin/tags-dict"));
      setModes(await api.get<ModeRow[]>("/api/v1/admin/section-modes"));
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.loadFail);
    }
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  async function save() {
    if (!edit.f.name.trim()) return;
    setBusy(true);
    try {
      if (edit.id === null) await api.post("/api/v1/admin/tags-dict", edit.f);
      else await api.put(`/api/v1/admin/tags-dict/${edit.id}`, edit.f);
      flash(at.saved);
      setEdit({ id: null, f: { ...EMPTY } });
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.opFail);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      <TagEditForm
        edit={edit}
        setEdit={setEdit}
        modes={modes}
        busy={busy}
        save={save}
      />
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">ID</td>
            <td className="colhead">{at.thPreview}</td>
            <td className="colhead">{at.thKind}</td>
            <td className="colhead">{at.thScope}</td>
            <td className="colhead">{at.thLayer}</td>
            <td className="colhead">{at.thUsage}</td>
            <td className="colhead">{at.thStyle}</td>
            <td className="colhead">{at.thMode}</td>
            <td className="colhead">{at.thStatus}</td>
            <td className="colhead text-right">{at.thAction}</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id} className={r.enabled ? "" : "opacity-50"}>
              <td className="num">{r.id}</td>
              <td>
                <span className={PREVIEW_SHELL}>
                  <span
                    style={{
                      background: r.bg_color || "transparent",
                      color: r.color,
                      fontSize: r.font_size,
                      padding: r.padding,
                      margin: r.margin,
                      borderRadius: r.border_radius,
                      border: r.bg_color ? undefined : "1px solid #ccc",
                      display: "inline-block",
                    }}
                  >
                    {r.name}
                  </span>
                </span>
              </td>
              <td>
                {r.kind === "official" ? at.kindOfficial : at.kindNormal}
              </td>
              <td>
                {r.scope === "forum" ? at.scopeForum : at.scopeTorrent}
              </td>
              <td
                title={
                  r.scope_layer === "global"
                    ? at.tipGlobalLayer
                    : at.tipSiteLayer
                }
              >
                {r.scope === "forum"
                  ? "—"
                  : r.scope_layer === "global"
                    ? at.layerGlobal
                    : at.layerSite}
              </td>
              {/* 引用计数（0159 P1 治理）：僵尸标签（0 引用）一目了然 */}
              <td className="num" title={at.tipUsage}>
                {r.scope === "forum"
                  ? (r.forum_usage ?? 0)
                  : (r.torrent_usage ?? 0)}
              </td>
              <td className="font-mono">
                {r.bg_color || "—"} / {r.font_size}
              </td>
              <td>
                {r.mode_id
                  ? (modes.find((m) => m.id === r.mode_id)?.name ??
                    `#${r.mode_id}`)
                  : at.allModes}
              </td>
              <td>{r.enabled ? at.enabled : at.disabled}</td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  onClick={() => setEdit({ id: r.id, f: { ...r } })}
                >
                  {at.edit}
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  disabled={busy}
                  onClick={async () => {
                    try {
                      await api.del(`/api/v1/admin/tags-dict/${r.id}`);
                      flash(at.deleted);
                      await load();
                    } catch (e) {
                      flash(e instanceof ApiError ? e.message : at.delFail);
                    }
                  }}
                >
                  {at.del}
                </button>
              </td>
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td colSpan={10} className="py-6 text-center text-sub">
                {at.empty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
