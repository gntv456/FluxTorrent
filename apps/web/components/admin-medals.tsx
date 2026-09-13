"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 第八轮 P2-7：勋章管理（好学站 system/medals 简化口径）
 *  字典 CRUD + 全站持有浏览 + 回收（授予入口在用户详情页） */

interface MedalRow {
  id: number;
  name: string;
  description: string | null;
  price: number | null;
  rarity: string | null;
  limited: boolean;
  get_type: number;
  duration_days: number | null;
  bonus_addition_factor: number | null;
  category_id: number;
  asset_ref: string | null;
  held_count: number;
}

interface UserMedalRow {
  user_id: number;
  username: string;
  medal_id: number;
  medal_name: string;
  source: string;
  wearing: boolean;
  granted_at: string | null;
}

const GET_TYPE: Record<number, string> = { 1: "兑换", 2: "授予", 3: "合成" };

export function AdminMedals() {
  const [rows, setRows] = useState<MedalRow[]>([]);
  const [held, setHeld] = useState<UserMedalRow[]>([]);
  const [heldUid, setHeldUid] = useState("");
  const [edit, setEdit] = useState<{ id: number | null; f: Partial<MedalRow> }>({ id: null, f: { name: "", get_type: 2, category_id: 0, limited: false } });
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const flash = (m: string) => { setMsg(m); setTimeout(() => setMsg(null), 3000); };

  const load = useCallback(async () => {
    try {
      setRows(await api.get<MedalRow[]>("/api/v1/admin/medals"));
      const q = heldUid.trim() ? `?uid=${encodeURIComponent(heldUid.trim())}` : "";
      setHeld(await api.get<UserMedalRow[]>(`/api/v1/admin/user-medals${q}`));
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "加载失败");
    }
  }, [heldUid]);
  useEffect(() => { load(); }, [load]);

  async function save() {
    setBusy(true);
    try {
      if (edit.id === null) await api.post("/api/v1/admin/medals", edit.f);
      else await api.put(`/api/v1/admin/medals/${edit.id}`, edit.f);
      flash("已保存");
      setEdit({ id: null, f: { name: "", get_type: 2, category_id: 0, limited: false } });
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  const inp = "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-2 text-sm outline-none focus:border-sky";

  return (
    <div className="flex flex-col gap-3">
      {msg && <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>}

      <section className="baozi-panel cmgmt-form p-4">
        <h2 className="mb-2 text-base font-bold">{edit.id === null ? "新建勋章" : `编辑勋章 #${edit.id}`}</h2>
        <div className="flex flex-wrap items-end gap-2">
          <label className="flex flex-col gap-1 text-xs">名称
            <input value={edit.f.name ?? ""} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, name: e.target.value } })} className={`${inp} w-32`} />
          </label>
          <label className="flex flex-col gap-1 text-xs">说明
            <input value={edit.f.description ?? ""} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, description: e.target.value } })} className={`${inp} w-48`} />
          </label>
          <label className="flex flex-col gap-1 text-xs">获取方式
            <select value={edit.f.get_type ?? 2} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, get_type: Number(e.target.value) } })} className={inp}>
              <option value={1}>兑换</option>
              <option value={2}>授予</option>
              <option value={3}>合成</option>
            </select>
          </label>
          <label className="flex flex-col gap-1 text-xs">价格(火花)
            <input type="number" value={edit.f.price ?? ""} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, price: e.target.value ? Number(e.target.value) : null } })} className={`${inp} w-24`} />
          </label>
          <label className="flex flex-col gap-1 text-xs">魔力加成(%)
            <input type="number" value={edit.f.bonus_addition_factor ?? ""} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, bonus_addition_factor: e.target.value ? Number(e.target.value) : null } })} className={`${inp} w-24`} />
          </label>
          <label className="flex flex-col gap-1 text-xs">有效期(天,空=永久)
            <input type="number" value={edit.f.duration_days ?? ""} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, duration_days: e.target.value ? Number(e.target.value) : null } })} className={`${inp} w-24`} />
          </label>
          <label className="flex flex-col gap-1 text-xs">稀有度
            <input value={edit.f.rarity ?? ""} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, rarity: e.target.value || null } })} placeholder="common/rare…" className={`${inp} w-28`} />
          </label>
          <label className="flex flex-col gap-1 text-xs">分组
            <input type="number" value={edit.f.category_id ?? 0} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, category_id: Number(e.target.value) } })} className={`${inp} w-16`} />
          </label>
          <label className="flex items-center gap-1 pb-2 text-xs">
            <input type="checkbox" checked={Boolean(edit.f.limited)} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, limited: e.target.checked } })} />限定
          </label>
          <button className="baozi-button" disabled={busy || !String(edit.f.name ?? "").trim()} onClick={save}>保存</button>
          {edit.id !== null && <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold" onClick={() => setEdit({ id: null, f: { name: "", get_type: 2, category_id: 0, limited: false } })}>取消</button>}
        </div>
      </section>

      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">ID</td><td className="colhead">名称</td><td className="colhead">获取</td>
            <td className="colhead">价格</td><td className="colhead">加成%</td><td className="colhead">有效期</td>
            <td className="colhead">持有数</td><td className="colhead text-right">操作</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((m) => (
            <tr key={m.id}>
              <td className="num">{m.id}</td>
              <td className="font-bold">{m.name}{m.limited && <span className="ml-1 rounded-full bg-coral/20 px-1.5 text-[10px] text-danger">限定</span>}</td>
              <td>{GET_TYPE[m.get_type] ?? m.get_type}</td>
              <td className="num">{m.price ?? "—"}</td>
              <td className="num">{m.bonus_addition_factor ?? 0}</td>
              <td className="num">{m.duration_days ?? "永久"}</td>
              <td className="num">{m.held_count}</td>
              <td className="text-right">
                <button className="cmgmt-act" onClick={() => setEdit({ id: m.id, f: { ...m } })}>编辑</button>
                <button className="cmgmt-act cmgmt-act--danger" disabled={busy}
                  onClick={async () => {
                    try { await api.del(`/api/v1/admin/medals/${m.id}`); flash("已删除"); await load(); }
                    catch (e) { flash(e instanceof ApiError ? e.message : "删除失败"); }
                  }}>删除</button>
              </td>
            </tr>
          ))}
          {rows.length === 0 && <tr><td colSpan={8} className="py-6 text-center text-sub">暂无勋章</td></tr>}
        </tbody>
      </table>

      <section className="baozi-panel p-4">
        <div className="mb-2 flex items-end gap-2">
          <h3 className="text-sm font-bold">持有浏览 / 回收</h3>
          <input value={heldUid} onChange={(e) => setHeldUid(e.target.value)} placeholder="按用户 UID 过滤" className="min-h-[32px] w-40 rounded-full border border-line px-3 text-xs" />
        </div>
        <table className="nexus-table text-xs">
          <thead>
            <tr><td className="colhead">用户</td><td className="colhead">勋章</td><td className="colhead">来源</td><td className="colhead">佩戴</td><td className="colhead text-right">操作</td></tr>
          </thead>
          <tbody>
            {held.map((h) => (
              <tr key={`${h.user_id}-${h.medal_id}`}>
                <td><a href={`/admin/users/${h.user_id}`} className="font-bold text-link">{h.username}</a></td>
                <td>{h.medal_name}</td>
                <td>{h.source}</td>
                <td>{h.wearing ? "佩戴中" : "—"}</td>
                <td className="text-right">
                  <button className="cmgmt-act cmgmt-act--danger" disabled={busy}
                    onClick={async () => {
                      try {
                        await api.post("/api/v1/admin/user-medals/delete", { user_id: h.user_id, medal_id: h.medal_id });
                        flash("已回收");
                        await load();
                      } catch (e) { flash(e instanceof ApiError ? e.message : "回收失败"); }
                    }}>回收</button>
                </td>
              </tr>
            ))}
            {held.length === 0 && <tr><td colSpan={5} className="py-4 text-center text-sub">暂无持有记录</td></tr>}
          </tbody>
        </table>
      </section>
    </div>
  );
}
