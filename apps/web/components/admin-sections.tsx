"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 第八轮 P3-12：维度管理（好学站 Section 组口径）
 *  分类模式（维度开关 + 归属分类 + 自动过审）+ 七维字典 CRUD */

interface ModeRow {
  id: number;
  name: string;
  show_source: boolean;
  show_medium: boolean;
  show_codec: boolean;
  show_audio_codec: boolean;
  show_standard: boolean;
  show_processing: boolean;
  show_team: boolean;
  categories: number;
}

interface DictRow {
  id: number;
  kind: string;
  name: string;
  sort: number;
  mode_id: number | null;
}

interface CategoryRow {
  id: number;
  name: string;
  torrents: number;
  mode_id: number | null;
  auto_approve?: boolean;
}

const KINDS: [string, string][] = [
  ["codec", "Codec"], ["audio_codec", "Audio Codec"], ["standard", "Standard"],
  ["team", "Team"], ["source", "Source"], ["processing", "Processing"],
  ["media", "Media"], ["grades", "Grade"], ["editions", "Edition"],
];

const FLAGS: [keyof ModeRow, string][] = [
  ["show_source", "Source"], ["show_medium", "Media"], ["show_codec", "Codec"],
  ["show_audio_codec", "Audio"], ["show_standard", "Standard"],
  ["show_processing", "Processing"], ["show_team", "Team"],
];

export function AdminSections() {
  const [modes, setModes] = useState<ModeRow[]>([]);
  const [kind, setKind] = useState("codec");
  const [dicts, setDicts] = useState<DictRow[]>([]);
  const [cats, setCats] = useState<CategoryRow[]>([]);
  const [newMode, setNewMode] = useState("");
  const [dName, setDName] = useState("");
  const [dSort, setDSort] = useState(0);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const flash = (m: string) => { setMsg(m); setTimeout(() => setMsg(null), 3000); };

  const load = useCallback(async () => {
    try {
      setModes(await api.get<ModeRow[]>("/api/v1/admin/section-modes"));
      setDicts(await api.get<DictRow[]>(`/api/v1/admin/section-dict?kind=${kind}`));
      setCats(await api.get<CategoryRow[]>("/api/v1/admin/categories"));
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "加载失败");
    }
  }, [kind]);
  useEffect(() => { load(); }, [load]);

  async function act(fn: () => Promise<unknown>, okMsg: string) {
    setBusy(true);
    try { await fn(); flash(okMsg); await load(); }
    catch (e) { flash(e instanceof ApiError ? e.message : "操作失败"); }
    finally { setBusy(false); }
  }

  return (
    <div className="flex flex-col gap-3">
      {msg && <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>}

      {/* 分类模式 */}
      <section className="baozi-panel p-4">
        <h2 className="mb-2 text-base font-bold">分类模式</h2>
        <p className="mb-3 text-xs text-sub">每个分类归属一个模式；模式内的开关决定发布表单与筛选启用哪些子维度。</p>
        <div className="flex flex-wrap items-end gap-2">
          <input value={newMode} onChange={(e) => setNewMode(e.target.value)} placeholder="新模式名称" className="min-h-[40px] w-40 rounded-[var(--r-sm)] border border-line px-2 text-sm" />
          <button disabled={busy || !newMode.trim()} className="baozi-button"
            onClick={() => act(async () => { await api.post("/api/v1/admin/section-modes", { name: newMode }); setNewMode(""); }, "已创建")}>新建模式</button>
        </div>
        <table className="nexus-table mt-3 text-xs">
          <thead>
            <tr>
              <td className="colhead">ID</td>
              <td className="colhead">名称</td>
              {FLAGS.map(([k, l]) => <td key={k} className="colhead">{l}</td>)}
              <td className="colhead">分类数</td>
              <td className="colhead text-right">操作</td>
            </tr>
          </thead>
          <tbody>
            {modes.map((m) => (
              <tr key={m.id}>
                <td className="num">{m.id}</td>
                <td className="font-bold">{m.name}</td>
                {FLAGS.map(([k]) => (
                  <td key={k} className="text-center">
                    <input type="checkbox" checked={Boolean(m[k])} disabled={busy}
                      onChange={(e) => act(() => api.put(`/api/v1/admin/section-modes/${m.id}`, { name: m.name, [k]: e.target.checked }), "已保存")} />
                  </td>
                ))}
                <td className="num">{m.categories}</td>
                <td className="text-right">
                  {m.id !== 1 && (
                    <button className="cmgmt-act cmgmt-act--danger" disabled={busy}
                      onClick={() => act(() => api.del(`/api/v1/admin/section-modes/${m.id}`), "已删除（归属分类已回退默认模式）")}>删除</button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      {/* 维度字典 */}
      <section className="baozi-panel p-4">
        <h2 className="mb-2 text-base font-bold">维度字典</h2>
        <div className="mb-2 flex flex-wrap items-end gap-2">
          <label className="flex flex-col gap-1 text-xs">维度
            <select value={kind} onChange={(e) => setKind(e.target.value)} className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
              {KINDS.map(([k, l]) => <option key={k} value={k}>{l}</option>)}
            </select>
          </label>
          <input value={dName} onChange={(e) => setDName(e.target.value)} placeholder="名称" className="min-h-[40px] w-36 rounded-[var(--r-sm)] border border-line px-2 text-sm" />
          <input type="number" value={dSort} onChange={(e) => setDSort(Number(e.target.value))} placeholder="排序" className="min-h-[40px] w-20 rounded-[var(--r-sm)] border border-line px-2 text-sm" />
          <button disabled={busy || !dName.trim()} className="baozi-button"
            onClick={() => act(async () => { await api.post("/api/v1/admin/section-dict", { kind, name: dName, sort: dSort }); setDName(""); }, "已添加")}>添加</button>
        </div>
        <table className="nexus-table text-xs">
          <thead>
            <tr><td className="colhead">ID</td><td className="colhead">名称</td><td className="colhead">排序</td><td className="colhead text-right">操作</td></tr>
          </thead>
          <tbody>
            {dicts.map((d) => (
              <tr key={d.id}>
                <td className="num">{d.id}</td>
                <td>{d.name}</td>
                <td className="num">{d.sort}</td>
                <td className="text-right">
                  <button className="cmgmt-act" disabled={busy}
                    onClick={() => {
                      const nn = window.prompt("新名称", d.name);
                      if (nn && nn !== d.name) act(() => api.put(`/api/v1/admin/section-dict/${d.id}`, { kind, name: nn, sort: d.sort }), "已保存");
                    }}>重命名</button>
                  <button className="cmgmt-act cmgmt-act--danger" disabled={busy}
                    onClick={() => act(() => api.del(`/api/v1/admin/section-dict/${d.id}?kind=${kind}`), "已删除")}>删除</button>
                </td>
              </tr>
            ))}
            {dicts.length === 0 && <tr><td colSpan={4} className="py-6 text-center text-sub">暂无字典项</td></tr>}
          </tbody>
        </table>
      </section>

      {/* 分类归属与自动过审 */}
      <section className="baozi-panel p-4">
        <h2 className="mb-2 text-base font-bold">分类归属模式 / 自动过审</h2>
        <p className="mb-3 text-xs text-sub">「自动过审」打开后，发布到该分类的种子直接过审（Auto Approval Settings 口径）。</p>
        <table className="nexus-table text-xs">
          <thead>
            <tr><td className="colhead">ID</td><td className="colhead">分类</td><td className="colhead">种子数</td><td className="colhead">归属模式</td><td className="colhead">自动过审</td></tr>
          </thead>
          <tbody>
            {cats.map((c) => (
              <tr key={c.id}>
                <td className="num">{c.id}</td>
                <td className="font-bold">{c.name}</td>
                <td className="num">{c.torrents}</td>
                <td>
                  <select value={c.mode_id ?? 1} disabled={busy} className="min-h-[32px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-1"
                    onChange={(e) => act(() => api.put(`/api/v1/admin/categories/${c.id}/flags`, { mode_id: Number(e.target.value) }), "已保存")}>
                    {modes.map((m) => <option key={m.id} value={m.id}>{m.name}</option>)}
                  </select>
                </td>
                <td className="text-center">
                  <input type="checkbox" disabled={busy} checked={Boolean(c.auto_approve)}
                    onChange={(e) => act(() => api.put(`/api/v1/admin/categories/${c.id}/flags`, { auto_approve: e.target.checked }), e.target.checked ? "已开启自动过审" : "已关闭自动过审")} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </div>
  );
}
