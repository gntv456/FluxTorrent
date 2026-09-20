"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { ToolTab } from "@/components/staff-tools";

/** 运营域面板（从 staff-tools.tsx 按域拆出，300 行门禁）：
 *  种子促销（promo）/ 批量私信（staffmess）/ 添加用户（adduser）。 */

interface CatItem { id: number; name: string; torrents: number }
interface SitePromo { id: number; scope: string; kind: string; category_id: number | null; category_name: string | null; starts_at: string; ends_at: string }

/** ISO → datetime-local 输入值（本地时区） */
function toLocalInput(iso: string): string {
  const d = new Date(iso);
  return new Date(d.getTime() - d.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
}

export function StaffOpsPanel({ tab, flash }: { tab: ToolTab; flash: (m: string) => void }) {
  const { dict } = useI18n();
  const t = dict.stafftools;
  const [cats, setCats] = useState<CatItem[]>([]);
  const [busy, setBusy] = useState(false);

  // 运营工具状态
  const [promo, setPromo] = useState<SitePromo[]>([]);
  const [promoEditId, setPromoEditId] = useState<number | null>(null);
  const [promoScope, setPromoScope] = useState("global");
  const [promoCat, setPromoCat] = useState<number | "">("");
  const [promoStart, setPromoStart] = useState("");
  const [promoEnd, setPromoEnd] = useState("");
  const [promoKind, setPromoKind] = useState("free");
  const [promoHours, setPromoHours] = useState(24);
  const [smSubject, setSmSubject] = useState("");
  const [smBody, setSmBody] = useState("");
  const [smMinClass, setSmMinClass] = useState("");
  const [auName, setAuName] = useState("");
  const [auEmail, setAuEmail] = useState("");
  const [auPass, setAuPass] = useState("");

  const load = useCallback(async () => {
    api.get<SitePromo[]>("/api/v1/admin/freeleech").then(setPromo).catch(() => {});
    // 促销按分类时需分类列表（管理口径，无权限时为空）
    api.get<CatItem[]>("/api/v1/admin/categories").then(setCats).catch(() => setCats([]));
  }, []);
  useEffect(() => { load(); }, [load]);

  async function guard(fn: () => Promise<void>, ok: string) {
    setBusy(true);
    try { await fn(); flash(ok); await load(); }
    catch (e) { flash(e instanceof ApiError ? e.message : dict.common.networkError); }
    finally { setBusy(false); }
  }

  return (
    <>
      {/* 种子促销（freeleech 升级：全站/官种/非官种/分类） */}
      {tab === "promo" && (
        <>
          <section className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold text-ink">{t.promoNew}</h2>
            <div className="cmgmt-form">
              <label>
                {t.promoScope}
                <select value={promoScope} onChange={(e) => { setPromoScope(e.target.value); setPromoCat(""); }}>
                  <option value="global">{t.scopeGlobal}</option>
                  <option value="official">{t.scopeOfficial}</option>
                  <option value="non_official">{t.scopeNonOfficial}</option>
                  <option value="category">{t.scopeCategory}</option>
                </select>
              </label>
              {promoScope === "category" && (
                <label>
                  {t.promoCat}
                  <select value={promoCat} onChange={(e) => setPromoCat(e.target.value === "" ? "" : Number(e.target.value))}>
                    <option value="">{t.promoCat}</option>
                    {cats.map((c) => (
                      <option key={c.id} value={c.id}>{c.name}</option>
                    ))}
                  </select>
                </label>
              )}
              <label>
                {t.promoStart}
                <input type="datetime-local" value={promoStart} onChange={(e) => setPromoStart(e.target.value)} />
              </label>
              <label>
                {t.promoEnd}
                <input type="datetime-local" value={promoEnd} onChange={(e) => setPromoEnd(e.target.value)} />
              </label>
              <label>
                {t.promoKind}
                <select value={promoKind} onChange={(e) => setPromoKind(e.target.value)}>
                  <option value="free">Free 免费</option>
                  <option value="x2">2x 上传</option>
                  <option value="x2free">2x 免费</option>
                  <option value="half">50% 下载</option>
                  <option value="x2half">2x 50%</option>
                  <option value="p30">30% 下载</option>
                </select>
              </label>
              <label>{t.promoHours}（未填结束时间时生效）<input type="number" min={1} max={720} value={promoHours} onChange={(e) => setPromoHours(Number(e.target.value))} /></label>
              <div className="flex gap-2">
                <button className="baozi-button" disabled={busy || promoHours < 1 || (promoScope === "category" && promoCat === "")}
                  onClick={() => guard(async () => {
                    const payload = {
                      kind: promoKind, hours: promoHours, scope: promoScope,
                      ...(promoScope === "category" ? { category_id: promoCat } : {}),
                      ...(promoStart ? { starts_at: new Date(promoStart).toISOString() } : {}),
                      ...(promoEnd ? { ends_at: new Date(promoEnd).toISOString() } : {}),
                    };
                    if (promoEditId === null) {
                      await api.post("/api/v1/admin/freeleech", payload);
                    } else {
                      await api.put(`/api/v1/admin/freeleech/${promoEditId}`, payload);
                      setPromoEditId(null);
                    }
                    setPromoCat("");
                  }, promoEditId === null ? t.promoSet : "已保存")}>{promoEditId === null ? t.promoBtnSet : "保存修改"}</button>
                <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold" disabled={busy || promo.length === 0}
                  onClick={() => guard(async () => { await api.del("/api/v1/admin/freeleech"); }, t.promoCleared)}>{t.promoBtnClear}</button>
              </div>
            </div>
          </section>
          <table className="nexus-table">
            <thead>
              <tr><td className="colhead">{t.colScope}</td><td className="colhead">{t.promoKind}</td><td className="colhead">{t.promoStart}</td><td className="colhead">{t.promoEnd}</td><td className="colhead w-32" /></tr>
            </thead>
            <tbody>
              {promo.length > 0 ? (
                promo.map((p) => (
                  <tr key={p.id} className={promoEditId === p.id ? "bg-sky-soft" : ""}>
                    <td className="font-bold">
                      {p.scope === "global" ? t.scopeGlobal
                        : p.scope === "official" ? t.scopeOfficial
                        : p.scope === "non_official" ? t.scopeNonOfficial
                        : p.scope === "category" ? `${t.scopeCategory} · ${p.category_name ?? `#${p.category_id}`}`
                        : p.scope}
                    </td>
                    <td className="font-bold">{p.kind}</td>
                    <td className="text-xs text-sub">{new Date(p.starts_at).toLocaleString("zh-CN")}</td>
                    <td className="text-xs text-sub">{new Date(p.ends_at).toLocaleString("zh-CN")}</td>
                    <td>
                      <button className="min-h-[28px] rounded-full border border-line px-3 text-xs font-bold text-sky"
                        onClick={() => {
                          if (promoEditId === p.id) { setPromoEditId(null); return; }
                          setPromoEditId(p.id);
                          setPromoScope(p.scope);
                          setPromoCat(p.scope === "category" ? (p.category_id ?? "") : "");
                          setPromoKind(p.kind);
                          setPromoStart(toLocalInput(p.starts_at));
                          setPromoEnd(toLocalInput(p.ends_at));
                        }}>{promoEditId === p.id ? "取消" : "编辑"}</button>
                      <button className="ml-1 min-h-[28px] rounded-full border border-line px-3 text-xs font-bold text-danger"
                        onClick={() => guard(async () => {
                          await api.del(`/api/v1/admin/freeleech/${p.id}`);
                          if (promoEditId === p.id) setPromoEditId(null);
                        }, "已删除")}>删除</button>
                    </td>
                  </tr>
                ))
              ) : (
                <tr><td colSpan={5} className="py-6 text-center text-sub">{t.promoNone}</td></tr>
              )}
            </tbody>
          </table>
        </>
      )}

      {/* 批量私信（staffmess） */}
      {tab === "staffmess" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">{t.smNew}</h2>
          <div className="cmgmt-form">
            <label>{t.fldSubject}<input value={smSubject} onChange={(e) => setSmSubject(e.target.value)} /></label>
            <label>{t.fldBody}<textarea rows={5} value={smBody} onChange={(e) => setSmBody(e.target.value)} /></label>
            <label>
              {t.smMinClass}
              <select value={smMinClass} onChange={(e) => setSmMinClass(e.target.value)}>
                <option value="">{t.smAllUsers}</option>
                <option value="10">Power User+</option>
                <option value="50">Elite+</option>
                <option value="90">管理组</option>
              </select>
            </label>
            <button className="baozi-button self-start" disabled={busy || !smSubject.trim() || !smBody.trim()}
              onClick={() => guard(async () => {
                await api.post("/api/v1/admin/staffmess", {
                  subject: smSubject, body: smBody,
                  min_class: smMinClass ? Number(smMinClass) : null,
                });
                setSmSubject(""); setSmBody("");
              }, t.smSent)}>
              {t.btnSend}
            </button>
          </div>
        </section>
      )}

      {/* 添加用户（adduser） */}
      {tab === "adduser" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">{t.auNew}</h2>
          <div className="cmgmt-form">
            <label>{t.auUsername}<input value={auName} onChange={(e) => setAuName(e.target.value)} /></label>
            <label>{t.auEmail}<input type="email" value={auEmail} onChange={(e) => setAuEmail(e.target.value)} /></label>
            <label>{t.auPassword}<input type="password" value={auPass} onChange={(e) => setAuPass(e.target.value)} /></label>
            <button className="baozi-button self-start" disabled={busy || !auName.trim() || !auEmail.includes("@") || auPass.length < 8}
              onClick={() => guard(async () => {
                await api.post("/api/v1/admin/adduser", { username: auName, email: auEmail, password: auPass });
                setAuName(""); setAuEmail(""); setAuPass("");
              }, t.auCreated)}>
              {t.auBtnCreate}
            </button>
            <p className="text-xs text-sub">{t.auNote}</p>
          </div>
        </section>
      )}
    </>
  );
}
