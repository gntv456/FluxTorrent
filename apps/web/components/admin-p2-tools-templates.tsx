"use client";

import { BTN_SM_BOLD } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

/** 消息模板面板（从 admin-p2-tools.tsx 按域拆出，300 门禁）：
 *  场景键模板的增删改 + 变量占位预览（第八轮 P2-11 支持新增）。 */

interface MessageTemplate {
  id: number;
  scene_key: string;
  subject: string;
  body: string;
  note: string | null;
  updated_at: string;
}

export function MsgTemplates({ flash }: { flash: (m: string) => void }) {
  const { dict } = useI18n();
  const at = dict.adminTemplates;
  const [rows, setRows] = useState<MessageTemplate[]>([]);
  const [editing, setEditing] = useState<{
    id: number;
    subject: string;
    body: string;
  } | null>(null);
  const [preview, setPreview] = useState<{
    subject: string;
    body: string;
  } | null>(null);
  const [busy, setBusy] = useState(false);
  // 第八轮 P2-11：新增模板
  const [creating, setCreating] = useState<{
    scene_key: string;
    subject: string;
    body: string;
    note: string;
  } | null>(null);

  const load = useCallback(async () => {
    try {
      setRows(await api.get("/api/v1/admin/message-templates"));
    } catch {
      setRows([]);
    }
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  const save = async () => {
    if (!editing) return;
    setBusy(true);
    try {
      await api.put(`/api/v1/admin/message-templates/${editing.id}`, {
        subject: editing.subject,
        body: editing.body,
      });
      flash(at.saved);
      setEditing(null);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.opFail);
    } finally {
      setBusy(false);
    }
  };

  const create = async () => {
    if (!creating) return;
    setBusy(true);
    try {
      await api.post("/api/v1/admin/message-templates", {
        scene_key: creating.scene_key.trim(),
        subject: creating.subject.trim(),
        body: creating.body.trim(),
        note: creating.note.trim() || undefined,
      });
      flash(at.created);
      setCreating(null);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.opFail);
    } finally {
      setBusy(false);
    }
  };

  const doPreview = async (sceneKey: string) => {
    try {
      const r = await api.post<{ subject: string; body: string }>(
        "/api/v1/admin/message-templates/preview",
        {
          scene_key: sceneKey,
          vars: {
            username: at.pvUser,
            torrent_name: at.pvTorrent,
            reason: at.pvReason,
            count: "2",
          },
        },
      );
      setPreview(r);
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.previewFail);
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex justify-end">
        <button
          className="min-h-[40px] rounded-full bg-sky px-5 text-sm font-bold text-white"
          onClick={() =>
            setCreating({ scene_key: "", subject: "", body: "", note: "" })
          }
        >
          {at.newBtn}
        </button>
      </div>
      {creating && (
        <section className="baozi-panel cmgmt-form p-4">
          <h2 className="mb-2 text-base font-bold text-ink">{at.newTitle}</h2>
          <label>
            {at.fSceneKey}
            <input
              value={creating.scene_key}
              onChange={(e) =>
                setCreating({ ...creating, scene_key: e.target.value })
              }
            />
          </label>
          <label>
            {at.fSubject}
            <input
              value={creating.subject}
              onChange={(e) =>
                setCreating({ ...creating, subject: e.target.value })
              }
            />
          </label>
          <label>
            {at.fBody}
            <textarea
              rows={5}
              value={creating.body}
              onChange={(e) =>
                setCreating({ ...creating, body: e.target.value })
              }
            />
          </label>
          <label>
            {at.fNote}
            <input
              value={creating.note}
              onChange={(e) =>
                setCreating({ ...creating, note: e.target.value })
              }
            />
          </label>
          <div className="flex gap-2">
            <button
              className="baozi-button"
              disabled={
                busy ||
                !creating.scene_key.trim() ||
                !creating.subject.trim() ||
                !creating.body.trim()
              }
              onClick={create}
            >
              {at.create}
            </button>
            <button
              className={BTN_SM_BOLD}
              onClick={() => setCreating(null)}
            >
              {at.cancel}
            </button>
          </div>
        </section>
      )}
      {editing && (
        <section className="baozi-panel cmgmt-form p-4">
          <h2 className="mb-2 text-base font-bold text-ink">
            {fmt(at.editTitle, { id: editing.id })}
          </h2>
          <label>
            {at.fSubject}
            <input
              value={editing.subject}
              onChange={(e) =>
                setEditing({ ...editing, subject: e.target.value })
              }
            />
          </label>
          <label>
            {at.fBody}
            <textarea
              rows={5}
              value={editing.body}
              onChange={(e) => setEditing({ ...editing, body: e.target.value })}
            />
          </label>
          <div className="flex gap-2">
            <button className="baozi-button" disabled={busy} onClick={save}>
              {at.save}
            </button>
            <button
              className={BTN_SM_BOLD}
              onClick={() => setEditing(null)}
            >
              {at.cancel}
            </button>
          </div>
        </section>
      )}
      {preview && (
        <section className="baozi-panel p-4 text-sm">
          <div className="mb-1 flex items-center justify-between">
            <b>{at.preview}</b>
            <button
              className="min-h-[32px] rounded-full border border-line px-3 text-xs"
              onClick={() => setPreview(null)}
            >
              {at.close}
            </button>
          </div>
          <p className="font-bold">{preview.subject}</p>
          <p className="whitespace-pre-wrap">{preview.body}</p>
        </section>
      )}
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">{at.thSceneKey}</td>
            <td className="colhead">{at.thSubject}</td>
            <td className="colhead">{at.thNote}</td>
            <td className="colhead text-right">{at.thAction}</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td className="font-mono text-xs">{r.scene_key}</td>
              <td>{r.subject}</td>
              <td className="text-xs text-sub">{r.note ?? "—"}</td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  onClick={() =>
                    setEditing({ id: r.id, subject: r.subject, body: r.body })
                  }
                >
                  {at.edit}
                </button>
                <button
                  className="cmgmt-act"
                  onClick={() => doPreview(r.scene_key)}
                >
                  {at.preview}
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  disabled={busy}
                  onClick={async () => {
                    if (!window.confirm(fmt(at.delConfirm, { key: r.scene_key })))
                      return;
                    try {
                      await api.del(`/api/v1/admin/message-templates/${r.id}`);
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
        </tbody>
      </table>
    </div>
  );
}
