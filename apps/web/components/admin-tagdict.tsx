"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 第八轮 P1-2：标签管理（好学站 torrent/tags 口径）
 *  字典 CRUD + 样式属性（背景/字体色/字号/边距/圆角）+ 分类模式作用域 */

interface TagRow {
  id: number;
  name: string;
  kind: string;
  /** 作用域（0138）：torrent=种子域 / forum=论坛域 */
  scope?: string;
  bg_color: string;
  color: string;
  font_size: string;
  margin: string;
  padding: string;
  border_radius: string;
  sort: number;
  enabled: boolean;
  mode_id: number | null;
}

interface ModeRow {
  id: number;
  name: string;
}

const EMPTY: Omit<TagRow, "id"> = {
  name: "",
  kind: "plain",
  scope: "torrent",
  bg_color: "#3b82f6",
  color: "#ffffff",
  font_size: "12px",
  margin: "0 4px 0 0",
  padding: "1px 4px",
  border_radius: "2px",
  sort: 0,
  enabled: true,
  mode_id: null,
};

export function AdminTagDict() {
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
      flash(e instanceof ApiError ? e.message : "加载失败");
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
      flash("已保存");
      setEdit({ id: null, f: { ...EMPTY } });
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  const inp =
    "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-2 text-sm outline-none focus:border-sky";
  /** 预览底色：标签透明背景时衬一块中性底，否则暗色主题下白字标签贴着暗底看不见 */
  const previewShell =
    "inline-block rounded-[var(--r-sm)] bg-[var(--surface-card)] px-2 py-1";

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      <section className="baozi-panel cmgmt-form p-4">
        <h2 className="mb-2 text-base font-bold">
          {edit.id === null ? "新建标签" : `编辑标签 #${edit.id}`}
        </h2>
        <div className="flex flex-wrap items-end gap-2">
          <label className="flex flex-col gap-1 text-xs">
            名称
            <input
              value={edit.f.name}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, name: e.target.value } })
              }
              className={`${inp} w-28`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            类型
            <select
              value={edit.f.kind}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, kind: e.target.value } })
              }
              className={inp}
            >
              <option value="plain">普通</option>
              <option value="official">官方</option>
            </select>
          </label>
          <label className="flex flex-col gap-1 text-xs">
            作用域
            <select
              value={edit.f.scope ?? "torrent"}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, scope: e.target.value } })
              }
              className={inp}
            >
              <option value="torrent">种子</option>
              <option value="forum">论坛</option>
            </select>
          </label>
          <label className="flex flex-col gap-1 text-xs">
            背景色
            <input
              value={edit.f.bg_color}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, bg_color: e.target.value } })
              }
              placeholder="#ff0000"
              className={`${inp} w-24`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            字体色
            <input
              value={edit.f.color}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, color: e.target.value } })
              }
              className={`${inp} w-24`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            字号
            <input
              value={edit.f.font_size}
              onChange={(e) =>
                setEdit({
                  ...edit,
                  f: { ...edit.f, font_size: e.target.value },
                })
              }
              className={`${inp} w-20`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            外边距
            <input
              value={edit.f.margin}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, margin: e.target.value } })
              }
              className={`${inp} w-28`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            内边距
            <input
              value={edit.f.padding}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, padding: e.target.value } })
              }
              className={`${inp} w-24`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            圆角
            <input
              value={edit.f.border_radius}
              onChange={(e) =>
                setEdit({
                  ...edit,
                  f: { ...edit.f, border_radius: e.target.value },
                })
              }
              className={`${inp} w-20`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            作用域模式
            <select
              value={edit.f.mode_id ?? ""}
              onChange={(e) =>
                setEdit({
                  ...edit,
                  f: {
                    ...edit.f,
                    mode_id: e.target.value ? Number(e.target.value) : null,
                  },
                })
              }
              className={inp}
            >
              <option value="">全部模式</option>
              {modes.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.name}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-xs">
            排序
            <input
              type="number"
              value={edit.f.sort}
              onChange={(e) =>
                setEdit({
                  ...edit,
                  f: { ...edit.f, sort: Number(e.target.value) },
                })
              }
              className={`${inp} w-16`}
            />
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
            disabled={busy || !edit.f.name.trim()}
            onClick={save}
          >
            保存
          </button>
          {edit.id !== null && (
            <button
              className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
              onClick={() => setEdit({ id: null, f: { ...EMPTY } })}
            >
              取消
            </button>
          )}
        </div>
        {/* 预览：name 为空时也显示占位字样，样式改了立刻能看到效果 */}
        <p className="mt-3 flex items-center gap-2 text-xs text-sub">
          预览：
          <span className={previewShell}>
            <span
              style={{
                background: edit.f.bg_color || "transparent",
                color: edit.f.color,
                fontSize: edit.f.font_size,
                margin: edit.f.margin,
                padding: edit.f.padding,
                borderRadius: edit.f.border_radius,
                border: edit.f.bg_color ? undefined : "1px solid #ccc",
              }}
            >
              {edit.f.name || "标签预览"}
            </span>
          </span>
        </p>
      </section>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">ID</td>
            <td className="colhead">预览</td>
            <td className="colhead">类型</td>
            <td className="colhead">作用域</td>
            <td className="colhead">样式</td>
            <td className="colhead">模式</td>
            <td className="colhead">状态</td>
            <td className="colhead text-right">操作</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id} className={r.enabled ? "" : "opacity-50"}>
              <td className="num">{r.id}</td>
              <td>
                <span className={previewShell}>
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
              <td>{r.kind === "official" ? "官方" : "普通"}</td>
              <td>{r.scope === "forum" ? "论坛" : "种子"}</td>
              <td className="font-mono">
                {r.bg_color || "—"} / {r.font_size}
              </td>
              <td>
                {r.mode_id
                  ? (modes.find((m) => m.id === r.mode_id)?.name ??
                    `#${r.mode_id}`)
                  : "全部"}
              </td>
              <td>{r.enabled ? "启用" : "停用"}</td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  onClick={() => setEdit({ id: r.id, f: { ...r } })}
                >
                  编辑
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  disabled={busy}
                  onClick={async () => {
                    try {
                      await api.del(`/api/v1/admin/tags-dict/${r.id}`);
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
              <td colSpan={8} className="py-6 text-center text-sub">
                暂无标签
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
