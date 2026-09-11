"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 第五轮 P2：置顶促销 / 自定义菜单 / 消息模板（好学站 Other 组口径） */
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

interface MenuItem {
  id: number;
  location: string;
  label: string;
  url: string;
  sort: number;
  enabled: boolean;
}

interface MessageTemplate {
  id: number;
  scene_key: string;
  subject: string;
  body: string;
  note: string | null;
  updated_at: string;
}

const LOCATIONS: [string, string][] = [
  ["sidebar", "侧栏"],
  ["footer", "页脚"],
  ["topbar", "顶栏"],
];

function useFlash() {
  const [msg, setMsg] = useState<string | null>(null);
  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  };
  const node = msg ? (
    <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>
  ) : null;
  return { flash, node };
}

export function AdminP2Tools() {
  const [sub, setSub] = useState<"promos" | "menus" | "templates" | "claims">("promos");
  const { flash, node } = useFlash();
  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap gap-2" role="tablist">
        {([
          ["promos", "置顶促销"],
          ["menus", "自定义菜单"],
          ["templates", "消息模板"],
          ["claims", "保种认领"],
        ] as [typeof sub, string][]).map(([k, label]) => (
          <button
            key={k}
            role="tab"
            aria-selected={sub === k}
            onClick={() => setSub(k)}
            className={`min-h-[40px] rounded-full px-4 text-sm font-bold ${sub === k ? "bg-sky text-white" : "border border-line bg-white text-sub"}`}
          >
            {label}
          </button>
        ))}
      </div>
      {node}
      {sub === "promos" && <StickyPromos flash={flash} />}
      {sub === "menus" && <MenuItems flash={flash} />}
      {sub === "templates" && <MsgTemplates flash={flash} />}
      {sub === "claims" && <Claims flash={flash} />}
    </div>
  );
}

function StickyPromos({ flash }: { flash: (m: string) => void }) {
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

function MenuItems({ flash }: { flash: (m: string) => void }) {
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

function MsgTemplates({ flash }: { flash: (m: string) => void }) {
  const [rows, setRows] = useState<MessageTemplate[]>([]);
  const [editing, setEditing] = useState<{ id: number; subject: string; body: string } | null>(null);
  const [preview, setPreview] = useState<{ subject: string; body: string } | null>(null);
  const [busy, setBusy] = useState(false);

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

  const doPreview = async (sceneKey: string) => {
    try {
      const r = await api.post<{ subject: string; body: string }>("/api/v1/admin/message-templates/preview", {
        scene_key: sceneKey,
        vars: { username: "示例用户", torrent_name: "示例种子", reason: "重复发布", count: "2" },
      });
      setPreview(r);
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "预览失败");
    }
  };

  return (
    <div className="flex flex-col gap-3">
      {editing && (
        <section className="baozi-panel cmgmt-form p-4">
          <h2 className="mb-2 text-base font-bold text-ink">编辑模板 #{editing.id}</h2>
          <label>
            主题
            <input value={editing.subject} onChange={(e) => setEditing({ ...editing, subject: e.target.value })} />
          </label>
          <label>
            正文（支持 {"{{username}}"} 等占位符）
            <textarea rows={5} value={editing.body} onChange={(e) => setEditing({ ...editing, body: e.target.value })} />
          </label>
          <div className="flex gap-2">
            <button className="baozi-button" disabled={busy} onClick={save}>
              保存
            </button>
            <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold" onClick={() => setEditing(null)}>
              取消
            </button>
          </div>
        </section>
      )}
      {preview && (
        <section className="baozi-panel p-4 text-sm">
          <div className="mb-1 flex items-center justify-between">
            <b>预览</b>
            <button className="min-h-[32px] rounded-full border border-line px-3 text-xs" onClick={() => setPreview(null)}>
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
                <button className="cmgmt-act" onClick={() => setEditing({ id: r.id, subject: r.subject, body: r.body })}>
                  编辑
                </button>
                <button className="cmgmt-act" onClick={() => doPreview(r.scene_key)}>
                  预览
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

interface ClaimRow {
  torrent_id: number;
  torrent_name: string | null;
  seeders: number;
  claimed_by: string | null;
  claimed_at: string | null;
  seed_time_delta: number;
  uploaded_delta: number;
  exited_at: string | null;
  exit_reason: string | null;
}

function fmtDeltaSec(sec: number): string {
  if (sec <= 0) return "0 小时";
  return `${(sec / 3600).toFixed(1)} 小时`;
}

function fmtBytes(n: number): string {
  if (n >= 1073741824) return `${(n / 1073741824).toFixed(2)} GB`;
  if (n >= 1048576) return `${(n / 1048576).toFixed(2)} MB`;
  return `${(n / 1024).toFixed(2)} KB`;
}

function Claims({ flash }: { flash: (m: string) => void }) {
  const [rows, setRows] = useState<ClaimRow[]>([]);
  const [state, setState] = useState("all");
  const [q, setQ] = useState("");
  const [page, setPage] = useState(1);
  const [total, setTotal] = useState(0);

  const load = useCallback(async () => {
    const params = new URLSearchParams();
    params.set("state", state);
    if (q.trim()) params.set("q", q.trim());
    params.set("page", String(page));
    try {
      const r = await api.get<{ rows: ClaimRow[]; total: number }>(`/api/v1/admin/claims?${params.toString()}`);
      setRows(r.rows);
      setTotal(r.total);
    } catch {
      setRows([]);
    }
  }, [state, q, page]);
  useEffect(() => {
    load();
  }, [load]);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap gap-2">
        <select value={state} onChange={(e) => { setState(e.target.value); setPage(1); }} className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-white px-2">
          <option value="all">全部</option>
          <option value="active">认领中</option>
          <option value="unclaimed">待认领</option>
          <option value="exited">已移出</option>
        </select>
        <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="种子名 / 认领人" className="min-h-[40px] flex-1 rounded-[var(--r-sm)] border border-line px-2" />
      </div>
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">种子</td>
            <td className="colhead">做种数</td>
            <td className="colhead">认领人</td>
            <td className="colhead">认领时间</td>
            <td className="colhead">认领以来做种</td>
            <td className="colhead">认领以来上传</td>
            <td className="colhead">状态</td>
            <td className="colhead text-right">操作</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.torrent_id}>
              <td className="max-w-[200px] truncate">
                <a className="text-link" href={`/torrents?id=${r.torrent_id}`}>{r.torrent_name ?? `#${r.torrent_id}`}</a>
              </td>
              <td>{r.seeders}</td>
              <td>{r.claimed_by ?? "—"}</td>
              <td className="text-xs">{r.claimed_at ? new Date(r.claimed_at).toLocaleString() : "—"}</td>
              <td>{r.claimed_by ? fmtDeltaSec(r.seed_time_delta) : "—"}</td>
              <td>{r.claimed_by ? fmtBytes(r.uploaded_delta) : "—"}</td>
              <td className="text-xs">{r.exited_at ? `已移出（${r.exit_reason ?? "manual"}）` : r.claimed_by ? "认领中" : "待认领"}</td>
              <td className="text-right">
                {!r.exited_at && (
                  <button
                    className="cmgmt-act cmgmt-act--danger"
                    onClick={async () => {
                      try {
                        await api.post("/api/v1/admin/claims/release", { torrent_id: r.torrent_id });
                        flash(`已移出 #${r.torrent_id}`);
                        load();
                      } catch (e) {
                        flash(e instanceof ApiError ? e.message : "操作失败");
                      }
                    }}
                  >
                    移出保种区
                  </button>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="flex items-center justify-between text-sm text-sub">
        <span>共 {total} 条</span>
        <div className="flex gap-2">
          <button disabled={page <= 1} onClick={() => setPage(page - 1)} className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40">上一页</button>
          <span>第 {page} 页</span>
          <button disabled={rows.length < 20} onClick={() => setPage(page + 1)} className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40">下一页</button>
        </div>
      </div>
    </div>
  );
}
