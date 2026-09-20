"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 自定义菜单面板（从 admin-p2-tools.tsx 按域拆出，300 门禁）：
 *  侧栏 / 页脚 / 顶栏菜单项的增删与启停。 */

interface MenuItem {
  id: number;
  location: string;
  label: string;
  url: string;
  sort: number;
  enabled: boolean;
}

const LOCATIONS: [string, string][] = [
  ["sidebar", "侧栏"],
  ["footer", "页脚"],
  ["topbar", "顶栏"],
];

export function MenuItems({ flash }: { flash: (m: string) => void }) {
  const [rows, setRows] = useState<MenuItem[]>([]);
  const [edit, setEdit] = useState<{ location: string; label: string; url: string }>({
    location: "sidebar",
    label: "",
    url: "",
  });
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setRows(await api.get("/api/v1/admin/menu-items"));
    } catch {
      setRows([]);
    }
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  const save = async () => {
    if (!edit.label.trim() || !edit.url.trim()) return;
    setBusy(true);
    try {
      await api.post("/api/v1/admin/menu-items", edit);
      flash("已新增菜单项");
      setEdit({ location: edit.location, label: "", url: "" });
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
        <h2 className="mb-2 text-base font-bold text-ink">新增自定义菜单</h2>
        <label>
          位置
          <select value={edit.location} onChange={(e) => setEdit({ ...edit, location: e.target.value })}>
            {LOCATIONS.map(([v, l]) => (
              <option key={v} value={v}>
                {l}
              </option>
            ))}
          </select>
        </label>
        <label>
          名称
          <input value={edit.label} onChange={(e) => setEdit({ ...edit, label: e.target.value })} />
        </label>
        <label>
          链接
          <input value={edit.url} onChange={(e) => setEdit({ ...edit, url: e.target.value })} placeholder="https://..." />
        </label>
        <button className="baozi-button" disabled={busy || !edit.label.trim() || !edit.url.trim()} onClick={save}>
          保存
        </button>
      </section>
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">ID</td>
            <td className="colhead">位置</td>
            <td className="colhead">名称</td>
            <td className="colhead">链接</td>
            <td className="colhead">排序</td>
            <td className="colhead">启用</td>
            <td className="colhead text-right">操作</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td>{r.id}</td>
              <td>{LOCATIONS.find(([v]) => v === r.location)?.[1] ?? r.location}</td>
              <td>{r.label}</td>
              <td className="max-w-[200px] truncate text-xs">{r.url}</td>
              <td>{r.sort}</td>
              <td>{r.enabled ? "是" : "否"}</td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  onClick={async () => {
                    try {
                      await api.put(`/api/v1/admin/menu-items/${r.id}`, { enabled: !r.enabled });
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
                      await api.del(`/api/v1/admin/menu-items/${r.id}`);
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
