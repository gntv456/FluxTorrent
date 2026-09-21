"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

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
      flash("模板已保存");
      setEditing(null);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
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
      flash("模板已创建");
      setCreating(null);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
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
            username: "示例用户",
            torrent_name: "示例种子",
            reason: "重复发布",
            count: "2",
          },
        },
      );
      setPreview(r);
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "预览失败");
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
          ＋ 新增模板
        </button>
      </div>
      {creating && (
        <section className="baozi-panel cmgmt-form p-4">
          <h2 className="mb-2 text-base font-bold text-ink">新增模板</h2>
          <label>
            场景键（小写字母/数字/下划线，如 contest_win）
            <input
              value={creating.scene_key}
              onChange={(e) =>
                setCreating({ ...creating, scene_key: e.target.value })
              }
            />
          </label>
          <label>
            主题
            <input
              value={creating.subject}
              onChange={(e) =>
                setCreating({ ...creating, subject: e.target.value })
              }
            />
          </label>
          <label>
            正文（支持 {"{{username}}"} 等占位符）
            <textarea
              rows={5}
              value={creating.body}
              onChange={(e) =>
                setCreating({ ...creating, body: e.target.value })
              }
            />
          </label>
          <label>
            说明（可选）
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
              创建
            </button>
            <button
              className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
              onClick={() => setCreating(null)}
            >
              取消
            </button>
          </div>
        </section>
      )}
      {editing && (
        <section className="baozi-panel cmgmt-form p-4">
          <h2 className="mb-2 text-base font-bold text-ink">
            编辑模板 #{editing.id}
          </h2>
          <label>
            主题
            <input
              value={editing.subject}
              onChange={(e) =>
                setEditing({ ...editing, subject: e.target.value })
              }
            />
          </label>
          <label>
            正文（支持 {"{{username}}"} 等占位符）
            <textarea
              rows={5}
              value={editing.body}
              onChange={(e) => setEditing({ ...editing, body: e.target.value })}
            />
          </label>
          <div className="flex gap-2">
            <button className="baozi-button" disabled={busy} onClick={save}>
              保存
            </button>
            <button
              className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
              onClick={() => setEditing(null)}
            >
              取消
            </button>
          </div>
        </section>
      )}
      {preview && (
        <section className="baozi-panel p-4 text-sm">
          <div className="mb-1 flex items-center justify-between">
            <b>预览</b>
            <button
              className="min-h-[32px] rounded-full border border-line px-3 text-xs"
              onClick={() => setPreview(null)}
            >
              关闭
            </button>
          </div>
          <p className="font-bold">{preview.subject}</p>
          <p className="whitespace-pre-wrap">{preview.body}</p>
        </section>
      )}
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">场景键</td>
            <td className="colhead">主题</td>
            <td className="colhead">说明</td>
            <td className="colhead text-right">操作</td>
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
                  编辑
                </button>
                <button
                  className="cmgmt-act"
                  onClick={() => doPreview(r.scene_key)}
                >
                  预览
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  disabled={busy}
                  onClick={async () => {
                    if (!window.confirm(`确认删除模板「${r.scene_key}」？`))
                      return;
                    try {
                      await api.del(`/api/v1/admin/message-templates/${r.id}`);
                      flash("模板已删除");
                      await load();
                    } catch (e) {
                      flash(e instanceof ApiError ? e.message : "删除失败");
                    }
                  }}
                >
                  删除
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
