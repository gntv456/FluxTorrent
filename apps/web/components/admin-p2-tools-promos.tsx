"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 置顶促销面板（从 admin-p2-tools.tsx 按域拆出，300 门禁）：
 *  首页公告条的增删与启停。 */

interface StickyPromo {
  id: number;
  title: string;
  url: string | null;
  badge: string | null;
  starts_at: string;
  ends_at: string;
  enabled: boolean;
  sort: number;
}

export function StickyPromos({ flash }: { flash: (m: string) => void }) {
  const [rows, setRows] = useState<StickyPromo[]>([]);
  const [edit, setEdit] = useState<{ title: string; url: string; badge: string }>({
    title: "",
    url: "",
    badge: "",
  });
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setRows(await api.get("/api/v1/admin/sticky-promos"));
    } catch {
      setRows([]);
    }
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  const save = async () => {
    if (!edit.title.trim()) return;
    setBusy(true);
    try {
      await api.post("/api/v1/admin/sticky-promos", {
        title: edit.title,
        url: edit.url || undefined,
        badge: edit.badge || undefined,
      });
      flash("已新增置顶促销");
      setEdit({ title: "", url: "", badge: "" });
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <section className="baozi-panel cmgmt-form p-4">
        <h2 className="mb-2 text-base font-bold text-ink">新增置顶促销（首页公告条）</h2>
        <label>
          标题
          <input value={edit.title} onChange={(e) => setEdit({ ...edit, title: e.target.value })} />
        </label>
        <label>
          链接（可选）
          <input value={edit.url} onChange={(e) => setEdit({ ...edit, url: e.target.value })} placeholder="/torrents?official=1" />
        </label>
        <label>
          角标（可选）
          <input value={edit.badge} onChange={(e) => setEdit({ ...edit, badge: e.target.value })} placeholder="活动" />
        </label>
        <button className="baozi-button" disabled={busy || !edit.title.trim()} onClick={save}>
          保存（默认 7 天有效）
        </button>
      </section>
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">ID</td>
            <td className="colhead">标题</td>
            <td className="colhead">角标</td>
            <td className="colhead">起止</td>
            <td className="colhead">启用</td>
            <td className="colhead text-right">操作</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td>{r.id}</td>
              <td>{r.url ? <a className="text-link" href={r.url}>{r.title}</a> : r.title}</td>
              <td>{r.badge ?? "—"}</td>
              <td className="text-xs">
                {new Date(r.starts_at).toLocaleDateString()} ~ {new Date(r.ends_at).toLocaleDateString()}
              </td>
              <td>{r.enabled ? "是" : "否"}</td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  onClick={async () => {
                    try {
                      await api.put(`/api/v1/admin/sticky-promos/${r.id}`, { title: r.title, enabled: !r.enabled });
                      flash(r.enabled ? "已停用" : "已启用");
                      load();
                    } catch {
                      flash("操作失败");
                    }
                  }}
                >
                  {r.enabled ? "停用" : "启用"}
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  onClick={async () => {
                    try {
                      await api.del(`/api/v1/admin/sticky-promos/${r.id}`);
                      flash("已删除");
                      load();
                    } catch {
                      flash("删除失败");
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
