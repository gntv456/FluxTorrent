"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 第八轮 P2-8：道具管理（好学站 prop/props + prop/user-props 口径）
 *  道具 CRUD（上下架）+ 用户背包浏览与回收 */

interface ShopItemRow {
  id: number;
  name: string;
  kind: string;
  price: number;
  config: Record<string, unknown>;
  active: boolean;
}

interface UserPropRow {
  order_id: number;
  user_id: number;
  username: string;
  item_id: number;
  item_name: string;
  kind: string;
  price: number;
  created_at: string;
}

const KIND_LABEL: Record<string, string> = {
  upload_credit: "上传量", invite: "邀请", temp_invite: "临时邀请", gift_spark: "火花",
  custom_title: "头衔卡", rename_card: "改名卡", makeup_card: "补签卡", rainbow_name: "彩虹名",
  rainbow_id: "彩虹ID", avatar_frame: "头像框", animated_avatar: "动态头像",
  vip: "VIP", app_vip: "APP VIP", ad_free: "去广告", charity: "公益",
};

export function AdminProps() {
  const [items, setItems] = useState<ShopItemRow[]>([]);
  const [props, setProps] = useState<UserPropRow[]>([]);
  const [uid, setUid] = useState("");
  const [edit, setEdit] = useState<{ id: number | null; f: { name: string; kind: string; price: string; config: string; active: boolean } }>({ id: null, f: { name: "", kind: "custom_title", price: "0", config: "{}", active: true } });
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const flash = (m: string) => { setMsg(m); setTimeout(() => setMsg(null), 3000); };

  const load = useCallback(async () => {
    try {
      setItems(await api.get<ShopItemRow[]>("/api/v1/admin/shop-items"));
      const q = uid.trim() ? `?uid=${encodeURIComponent(uid.trim())}` : "";
      setProps(await api.get<UserPropRow[]>(`/api/v1/admin/user-props${q}`));
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "加载失败");
    }
  }, [uid]);
  useEffect(() => { load(); }, [load]);

  async function save() {
    let config: unknown;
    try { config = JSON.parse(edit.f.config || "{}"); } catch { flash("config 需为合法 JSON"); return; }
    setBusy(true);
    try {
      const payload = { name: edit.f.name, kind: edit.f.kind, price: Number(edit.f.price) || 0, config, active: edit.f.active };
      if (edit.id === null) await api.post("/api/v1/admin/shop-items", payload);
      else await api.put(`/api/v1/admin/shop-items/${edit.id}`, payload);
      flash("已保存");
      setEdit({ id: null, f: { name: "", kind: "custom_title", price: "0", config: "{}", active: true } });
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
        <h2 className="mb-2 text-base font-bold">{edit.id === null ? "新建道具" : `编辑道具 #${edit.id}`}</h2>
        <div className="flex flex-wrap items-end gap-2">
          <label className="flex flex-col gap-1 text-xs">名称
            <input value={edit.f.name} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, name: e.target.value } })} className={`${inp} w-36`} />
          </label>
          <label className="flex flex-col gap-1 text-xs">类型
            <select value={edit.f.kind} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, kind: e.target.value } })} className={inp}>
              {Object.entries(KIND_LABEL).map(([k, l]) => <option key={k} value={k}>{l}（{k}）</option>)}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-xs">价格(火花)
            <input type="number" value={edit.f.price} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, price: e.target.value } })} className={`${inp} w-24`} />
          </label>
          <label className="flex flex-col gap-1 text-xs">config(JSON)
            <input value={edit.f.config} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, config: e.target.value } })} placeholder='{"gb":10} / {"amount":5000}' className={`${inp} w-64 font-mono`} />
          </label>
          <label className="flex items-center gap-1 pb-2 text-xs">
            <input type="checkbox" checked={edit.f.active} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, active: e.target.checked } })} />上架
          </label>
          <button className="baozi-button" disabled={busy || !edit.f.name.trim()} onClick={save}>保存</button>
          {edit.id !== null && <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold" onClick={() => setEdit({ id: null, f: { name: "", kind: "custom_title", price: "0", config: "{}", active: true } })}>取消</button>}
        </div>
        <p className="mt-2 text-xs text-sub">即时生效类（上传量/火花/邀请）发放直接入账；卡牌/装饰类入背包待用户使用。</p>
      </section>

      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">ID</td><td className="colhead">名称</td><td className="colhead">类型</td>
            <td className="colhead">价格</td><td className="colhead">config</td><td className="colhead">状态</td><td className="colhead text-right">操作</td>
          </tr>
        </thead>
        <tbody>
          {items.map((it) => (
            <tr key={it.id} className={it.active ? "" : "opacity-50"}>
              <td className="num">{it.id}</td>
              <td className="font-bold">{it.name}</td>
              <td>{KIND_LABEL[it.kind] ?? it.kind}</td>
              <td className="num">{it.price}</td>
              <td className="max-w-[220px] truncate font-mono">{JSON.stringify(it.config)}</td>
              <td>{it.active ? "上架" : "下架"}</td>
              <td className="text-right">
                <button className="cmgmt-act" onClick={() => setEdit({ id: it.id, f: { name: it.name, kind: it.kind, price: String(it.price), config: JSON.stringify(it.config), active: it.active } })}>编辑</button>
                <button className="cmgmt-act cmgmt-act--danger" disabled={busy}
                  onClick={async () => {
                    try {
                      const r = await api.del<{ deleted?: number; disabled?: boolean }>(`/api/v1/admin/shop-items/${it.id}`);
                      flash(r?.disabled ? "已有持有记录，已改为下架" : "已删除");
                      await load();
                    } catch (e) { flash(e instanceof ApiError ? e.message : "删除失败"); }
                  }}>删除/下架</button>
              </td>
            </tr>
          ))}
          {items.length === 0 && <tr><td colSpan={7} className="py-6 text-center text-sub">暂无道具</td></tr>}
        </tbody>
      </table>

      <section className="baozi-panel p-4">
        <div className="mb-2 flex items-end gap-2">
          <h3 className="text-sm font-bold">用户背包（购买 + 发放）</h3>
          <input value={uid} onChange={(e) => setUid(e.target.value)} placeholder="按用户 UID 过滤" className="min-h-[32px] w-40 rounded-full border border-line px-3 text-xs" />
        </div>
        <table className="nexus-table text-xs">
          <thead>
            <tr><td className="colhead">单号</td><td className="colhead">用户</td><td className="colhead">道具</td><td className="colhead">类型</td><td className="colhead">价格</td><td className="colhead">时间</td><td className="colhead text-right">操作</td></tr>
          </thead>
          <tbody>
            {props.map((p) => (
              <tr key={p.order_id}>
                <td className="num">{p.order_id}</td>
                <td><a href={`/admin/users/${p.user_id}`} className="font-bold text-link">{p.username}</a></td>
                <td>{p.item_name}</td>
                <td>{KIND_LABEL[p.kind] ?? p.kind}</td>
                <td className="num">{p.price}</td>
                <td className="text-sub">{new Date(p.created_at).toLocaleString()}</td>
                <td className="text-right">
                  {["upload_credit", "gift_spark", "invite", "temp_invite"].includes(p.kind) ? (
                    <span className="text-sub">即时生效</span>
                  ) : (
                    <button className="cmgmt-act cmgmt-act--danger" disabled={busy}
                      onClick={async () => {
                        try { await api.del(`/api/v1/admin/user-props/${p.order_id}`); flash("已回收"); await load(); }
                        catch (e) { flash(e instanceof ApiError ? e.message : "回收失败"); }
                      }}>回收</button>
                  )}
                </td>
              </tr>
            ))}
            {props.length === 0 && <tr><td colSpan={7} className="py-4 text-center text-sub">暂无持有记录</td></tr>}
          </tbody>
        </table>
      </section>
    </div>
  );
}
