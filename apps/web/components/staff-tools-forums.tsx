"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 论坛版块管理面板（从 staff-tools.tsx 按域拆出，300 行门禁）：
 *  forummanage 口径——三档门槛 + 受保护 + 版主任免 + 分区管理。 */

interface ForumAdminForum { id: number; name: string; descr: string | null; minclassread: number; minclasswrite: number; minclasscreate: number; protected: boolean; topics: number; category_id?: number | null; category_name?: string | null }
interface ForumCategory { id: number; name: string; sort: number; visible: boolean }
interface ForumAdminData { forums: ForumAdminForum[]; mods: [number, number, string][]; categories?: ForumCategory[] }

export function StaffForumsPanel({ flash }: { flash: (m: string) => void }) {
  const { dict } = useI18n();
  const [forumData, setForumData] = useState<ForumAdminData | null>(null);
  const [fEditId, setFEditId] = useState<number | null>(null);
  const [fName, setFName] = useState("");
  const [fDescr, setFDescr] = useState("");
  const [fMr, setFMr] = useState(0);
  const [fMw, setFMw] = useState(0);
  const [fMc, setFMc] = useState(0);
  const [fProt, setFProt] = useState(false);
  const [fCatId, setFCatId] = useState<number | "">("");
  const [fNewCat, setFNewCat] = useState("");
  const [fModName, setFModName] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    api.get<ForumAdminData | null>("/api/v1/admin/forums").then(setForumData).catch(() => setForumData(null));
  }, []);
  useEffect(() => { load(); }, [load]);

  async function guard(fn: () => Promise<void>, ok: string) {
    setBusy(true);
    try { await fn(); flash(ok); await load(); }
    catch (e) { flash(e instanceof ApiError ? e.message : dict.common.networkError); }
    finally { setBusy(false); }
  }

  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-1 text-base font-bold">论坛版块管理</h2>
      <p className="mb-3 text-xs text-sub">
        三档门槛需满足 读 ≤ 回 ≤ 发（0 = 所有人）；受保护版块 2 楼起正文对普通用户隐藏；版主任免无需等级，仅在本版块有效。
      </p>
      <table className="nexus-table text-xs">
        <thead><tr>
          <td className="colhead">版块</td>
          <td className="colhead w-28">分区</td>
          <td className="colhead w-36">门槛 读/回/发</td>
          <td className="colhead w-14">保护</td>
          <td className="colhead w-14">主题</td>
          <td className="colhead">版主</td>
          <td className="colhead w-24" />
        </tr></thead>
        <tbody>
          {(forumData?.forums ?? []).map((f) => {
            const mods = (forumData?.mods ?? []).filter((m) => m[0] === f.id);
            return (
              <tr key={f.id}>
                <td>
                  <Link href={`/forums/${f.id}`} className="font-bold text-sky">{f.name}</Link>
                  {f.descr && <p className="text-sub">{f.descr}</p>}
                </td>
                <td className="text-sub">{f.category_name ?? "—"}</td>
                <td className="num">{f.minclassread} / {f.minclasswrite} / {f.minclasscreate}</td>
                <td className="text-center">{f.protected ? "🛡" : "—"}</td>
                <td className="num">{f.topics}</td>
                <td>
                  {mods.length === 0 ? (
                    <span className="text-sub">—</span>
                  ) : (
                    mods.map((m) => (
                      <span key={m[1]} className="mr-1 inline-flex items-center gap-1 rounded-full border border-line px-2 py-0.5">
                        {m[2]}
                        <button
                          className="font-bold text-danger"
                          title="移除版主"
                          onClick={() => guard(async () => {
                            await api.del(`/api/v1/admin/forums/${f.id}/mods/${m[1]}`);
                            setForumData(await api.get("/api/v1/admin/forums"));
                          }, "OK")}
                        >×</button>
                      </span>
                    ))
                  )}
                  <input
                    value={fModName}
                    onChange={(e) => setFModName(e.target.value)}
                    placeholder="用户名"
                    className="ml-1 min-h-[28px] w-24 rounded-full border border-line bg-cloud px-2 text-xs outline-none focus:border-sky"
                  />
                  <button
                    className="ml-1 min-h-[28px] rounded-full bg-sky px-2 text-xs font-bold text-white"
                    onClick={() => guard(async () => {
                      await api.post(`/api/v1/admin/forums/${f.id}/mods`, { username: fModName.trim() });
                      setFModName("");
                      setForumData(await api.get("/api/v1/admin/forums"));
                    }, "已任命")}
                  >+版主</button>
                </td>
                <td>
                  <button
                    className="min-h-[28px] rounded-full border border-line px-3 font-bold text-sky"
                    onClick={() => { setFEditId(f.id); setFName(f.name); setFDescr(f.descr ?? ""); setFMr(f.minclassread); setFMw(f.minclasswrite); setFMc(f.minclasscreate); setFProt(f.protected); setFCatId(f.category_id ?? ""); }}
                  >编辑</button>
                  <button
                    className="ml-1 min-h-[28px] rounded-full border border-line px-3 font-bold text-danger"
                    onClick={() => guard(async () => {
                      await api.del(`/api/v1/admin/forums/${f.id}`);
                      setForumData(await api.get("/api/v1/admin/forums"));
                    }, "已删除")}
                  >删除</button>
                </td>
              </tr>
            );
          })}
          {(forumData?.forums ?? []).length === 0 && (
            <tr><td colSpan={7} className="py-4 text-center text-sub">
              {forumData === null ? "Load 失败或无权限" : "暂无版块"}
            </td></tr>
          )}
        </tbody>
      </table>

      <div className="mt-4 flex flex-wrap items-end gap-2">
        <h3 className="w-full text-sm font-bold">{fEditId === null ? "新建版块" : `编辑版块 #${fEditId}`}</h3>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">名称</span>
          <input value={fName} onChange={(e) => setFName(e.target.value)}
            className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky" />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">描述</span>
          <input value={fDescr} onChange={(e) => setFDescr(e.target.value)}
            className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky" />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">minclassread</span>
          <input type="number" min={0} value={fMr} onChange={(e) => setFMr(Number(e.target.value))}
            className="min-h-[40px] w-24 rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky" />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">minclasswrite</span>
          <input type="number" min={0} value={fMw} onChange={(e) => setFMw(Number(e.target.value))}
            className="min-h-[40px] w-24 rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky" />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">minclasscreate</span>
          <input type="number" min={0} value={fMc} onChange={(e) => setFMc(Number(e.target.value))}
            className="min-h-[40px] w-24 rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky" />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">分区</span>
          <select
            value={fCatId}
            onChange={(e) => setFCatId(e.target.value === "" ? "" : Number(e.target.value))}
            className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky"
          >
            <option value="">未分组</option>
            {(forumData?.categories ?? []).map((c) => (
              <option key={c.id} value={c.id}>{c.name}</option>
            ))}
          </select>
        </label>
        <label className="flex items-center gap-2 pb-2 text-sm">
          <input type="checkbox" checked={fProt} onChange={(e) => setFProt(e.target.checked)} />
          受保护版块
        </label>
        <button
          disabled={busy}
          className="min-h-[40px] rounded-full bg-sky px-5 text-sm font-bold text-white disabled:opacity-50"
          onClick={() => guard(async () => {
            const payload = { name: fName.trim(), descr: fDescr.trim() || null, minclassread: fMr, minclasswrite: fMw, minclasscreate: fMc, protected: fProt, category_id: fCatId === "" ? null : Number(fCatId) };
            if (fEditId === null) {
              await api.post("/api/v1/admin/forums", payload);
            } else {
              await api.put(`/api/v1/admin/forums/${fEditId}`, payload);
              setFEditId(null);
            }
            setFName(""); setFDescr(""); setFMr(0); setFMw(0); setFMc(0); setFProt(false); setFCatId("");
            setForumData(await api.get("/api/v1/admin/forums"));
          }, fEditId === null ? "已创建" : "已保存")}
        >{fEditId === null ? "创建" : "保存"}</button>
        {fEditId !== null && (
          <button className="min-h-[40px] rounded-full border border-line px-4 text-sm text-sub"
            onClick={() => { setFEditId(null); setFName(""); setFDescr(""); setFMr(0); setFMw(0); setFMc(0); setFProt(false); setFCatId(""); }}>
            取消
          </button>
        )}
      </div>

      {/* 分区/节点管理（0115）：列表 + 新建 + 删除（删分区不删版块，版块回落未分组） */}
      <div className="mt-4 border-t border-line pt-4">
        <h3 className="mb-2 text-sm font-bold">分区管理</h3>
        <div className="mb-2 flex flex-wrap gap-2">
          {(forumData?.categories ?? []).map((c) => (
            <span key={c.id} className="inline-flex items-center gap-1 rounded-full border border-line px-2 py-1 text-xs">
              {c.name}
              <button
                className="font-bold text-danger"
                title="删除分区（版块回落未分组）"
                onClick={() => guard(async () => {
                  await api.del(`/api/v1/admin/forum-categories/${c.id}`);
                  setForumData(await api.get("/api/v1/admin/forums"));
                }, "已删除分区")}
              >×</button>
            </span>
          ))}
          {(forumData?.categories ?? []).length === 0 && <span className="text-xs text-sub">暂无分区</span>}
        </div>
        <div className="flex items-end gap-2">
          <label className="flex flex-col gap-1">
            <span className="text-xs text-sub">新分区名</span>
            <input value={fNewCat} onChange={(e) => setFNewCat(e.target.value)}
              className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky" />
          </label>
          <button
            disabled={busy || !fNewCat.trim()}
            className="min-h-[40px] rounded-full bg-sky px-5 text-sm font-bold text-white disabled:opacity-50"
            onClick={() => guard(async () => {
              await api.post("/api/v1/admin/forum-categories", { name: fNewCat.trim() });
              setFNewCat("");
              setForumData(await api.get("/api/v1/admin/forums"));
            }, "已创建分区")}
          >新建分区</button>
        </div>
      </div>
    </section>
  );
}
