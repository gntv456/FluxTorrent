"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 第八轮 P3-16：Tracker URL 管理（NexusPHP tracker_urls 口径）
 *  多 announce 地址：默认位 / 启用 / 优先级 */

interface TrackerUrlRow {
  id: number;
  url: string;
  is_default: boolean;
  enabled: boolean;
  priority: number;
  updated_at: string;
}

export function AdminTrackers() {
  const [rows, setRows] = useState<TrackerUrlRow[]>([]);
  const [edit, setEdit] = useState<{
    id: number | null;
    f: { url: string; is_default: boolean; enabled: boolean; priority: string };
  }>({
    id: null,
    f: { url: "", is_default: false, enabled: true, priority: "0" },
  });
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  };

  const load = useCallback(async () => {
    try {
      setRows(await api.get<TrackerUrlRow[]>("/api/v1/admin/tracker-urls"));
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "加载失败");
    }
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  async function save() {
    setBusy(true);
    try {
      const payload = {
        url: edit.f.url,
        is_default: edit.f.is_default,
        enabled: edit.f.enabled,
        priority: Number(edit.f.priority) || 0,
      };
      if (edit.id === null)
        await api.post("/api/v1/admin/tracker-urls", payload);
      else await api.put(`/api/v1/admin/tracker-urls/${edit.id}`, payload);
      flash("已保存");
      setEdit({
        id: null,
        f: { url: "", is_default: false, enabled: true, priority: "0" },
      });
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  const inp =
    "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-2 text-sm outline-none focus:border-sky";

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      <section className="baozi-panel cmgmt-form p-4">
        <h2 className="mb-2 text-base font-bold">
          {edit.id === null ? "新增 Tracker URL" : `编辑 #${edit.id}`}
        </h2>
        <p className="mb-2 text-xs text-sub">
          默认地址用于新种子 announce 注入；备用地址供下载端容灾切换。
        </p>
        <div className="flex flex-wrap items-end gap-2">
          <label className="flex flex-1 flex-col gap-1 text-xs">
            URL
            <input
              value={edit.f.url}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, url: e.target.value } })
              }
              placeholder="https://tracker.example.com/announce"
              className={inp}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            优先级
            <input
              type="number"
              value={edit.f.priority}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, priority: e.target.value } })
              }
              className={`${inp} w-20`}
            />
          </label>
          <label className="flex items-center gap-1 pb-2 text-xs">
            <input
              type="checkbox"
              checked={edit.f.is_default}
              onChange={(e) =>
                setEdit({
                  ...edit,
                  f: { ...edit.f, is_default: e.target.checked },
                })
              }
            />
            默认
          </label>
          <label className="flex items-center gap-1 pb-2 text-xs">
            <input
              type="checkbox"
              checked={edit.f.enabled}
              onChange={(e) =>
                setEdit({
                  ...edit,
                  f: { ...edit.f, enabled: e.target.checked },
                })
              }
            />
            启用
          </label>
          <button
            className="baozi-button"
            disabled={busy || !edit.f.url.trim()}
            onClick={save}
          >
            保存
          </button>
          {edit.id !== null && (
            <button
              className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
              onClick={() =>
                setEdit({
                  id: null,
                  f: {
                    url: "",
                    is_default: false,
                    enabled: true,
                    priority: "0",
                  },
                })
              }
            >
              取消
            </button>
          )}
        </div>
      </section>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">ID</td>
            <td className="colhead">URL</td>
            <td className="colhead">默认</td>
            <td className="colhead">启用</td>
            <td className="colhead">优先级</td>
            <td className="colhead">更新时间</td>
            <td className="colhead text-right">操作</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td className="num">{r.id}</td>
              <td className="font-mono">{r.url}</td>
              <td className="text-center">{r.is_default ? "✅" : "—"}</td>
              <td>{r.enabled ? "启用" : "停用"}</td>
              <td className="num">{r.priority}</td>
              <td className="text-sub">
                {new Date(r.updated_at).toLocaleString()}
              </td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  onClick={() =>
                    setEdit({
                      id: r.id,
                      f: {
                        url: r.url,
                        is_default: r.is_default,
                        enabled: r.enabled,
                        priority: String(r.priority),
                      },
                    })
                  }
                >
                  编辑
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  disabled={busy}
                  onClick={async () => {
                    try {
                      await api.del(`/api/v1/admin/tracker-urls/${r.id}`);
                      flash("已删除");
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
          {rows.length === 0 && (
            <tr>
              <td colSpan={7} className="py-6 text-center text-sub">
                暂无 Tracker 地址
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
