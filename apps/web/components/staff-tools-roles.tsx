"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

/** 职务管理面板（从 staff-tools.tsx 按域拆出，300 行门禁）：
 *  user_roles 口径——职务字典 CRUD + 用户职务授予/撤销。 */

interface RoleDef {
  key: string; name: string; descr: string | null;
}
interface UserRoleRow {
  user_id: number; role_key: string; granted_by: number | null;
  granted_at: string; expires_at: string | null;
}

export function StaffRolesPanel({ flash }: { flash: (m: string) => void }) {
  const { dict, locale } = useI18n();
  const [roleDefs, setRoleDefs] = useState<RoleDef[]>([]);
  const [roleEdit, setRoleEdit] = useState<{ key: string; name: string; descr: string; mode: "new" | "edit" } | null>(null);
  const [userRoles, setUserRoles] = useState<UserRoleRow[]>([]);
  const [rFilter, setRFilter] = useState("");
  const [rUserId, setRUserId] = useState("");
  const [rRoleKey, setRRoleKey] = useState("uploader");
  const [rExpires, setRExpires] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    api.get<RoleDef[]>("/api/v1/admin/roles").then(setRoleDefs).catch(() => setRoleDefs([]));
    api.get<UserRoleRow[]>("/api/v1/admin/user-roles").then(setUserRoles).catch(() => setUserRoles([]));
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
      <h2 className="mb-2 text-base font-bold">职务管理</h2>
      <p className="mb-3 text-xs text-sub">
        职务与等级正交，一人可兼任多个，权限取并集。授予与撤销都要求操作者等级严格高于目标用户；职务字典的新增/编辑/删除仅站长。
      </p>

      {/* 职务字典：卡片 + 编辑/删除；新增表单（sysop） */}
      <div className="mb-4 flex flex-col gap-2">
        <div className="flex flex-wrap gap-2">
          {roleDefs.map((r) => (
            <div
              key={r.key}
              className="min-w-[150px] rounded-[var(--r-md)] border border-line bg-[var(--surface-raised)] px-3 py-2"
            >
              <p className="text-sm font-bold text-ink">{r.name}</p>
              <p className="mt-0.5 text-xs text-sub">{r.descr}</p>
              <p className="mt-1 flex items-center justify-between">
                <code className="text-[10px] text-sub">{r.key}</code>
                <span className="flex gap-1">
                  <button
                    className="min-h-[24px] rounded-full border border-line px-2 text-[11px] font-bold text-sky"
                    title="编辑职务（仅站长）"
                    onClick={() => {
                      setRoleEdit({ key: r.key, name: r.name, descr: r.descr ?? "", mode: "edit" });
                    }}
                  >
                    编辑
                  </button>
                  <button
                    className="min-h-[24px] rounded-full border border-line px-2 text-[11px] font-bold text-danger"
                    title="删除职务（需先撤销全部授予；仅站长）"
                    onClick={() => guard(async () => {
                      if (!window.confirm(`确认删除职务「${r.name}」？需先撤销全部授予。`)) return;
                      await api.del(`/api/v1/admin/roles/${r.key}`);
                      setRoleDefs(await api.get<RoleDef[]>("/api/v1/admin/roles"));
                    }, "已删除")}
                  >
                    删除
                  </button>
                </span>
              </p>
            </div>
          ))}
          {roleDefs.length === 0 && (
            <p className="text-xs text-sub">职务字典为空（迁移 0054 未应用？）</p>
          )}
        </div>

        {/* 新增/编辑职务（sysop：roles.manage；无权限时后端拒绝，前端如实提示） */}
        {roleEdit && (
          <div className="cmgmt-form rounded-[var(--r-md)] border border-line p-3">
            <h3 className="mb-2 text-sm font-bold">
              {roleEdit.mode === "new" ? "新增职务" : `编辑职务 ${roleEdit.key}（key 不可改）`}
            </h3>
            <div className="flex flex-wrap items-end gap-2">
              {roleEdit.mode === "new" && (
                <label className="flex flex-col gap-1 text-xs">
                  Key（小写/下划线）
                  <input
                    value={roleEdit.key}
                    onChange={(e) => setRoleEdit({ ...roleEdit, key: e.target.value })}
                    placeholder="translator"
                    className="min-h-[40px] w-40 rounded-[var(--r-sm)] border border-line bg-cloud px-2 text-sm"
                  />
                </label>
              )}
              <label className="flex flex-col gap-1 text-xs">
                名称
                <input
                  value={roleEdit.name}
                  onChange={(e) => setRoleEdit({ ...roleEdit, name: e.target.value })}
                  placeholder="翻译员"
                  className="min-h-[40px] w-32 rounded-[var(--r-sm)] border border-line bg-cloud px-2 text-sm"
                />
              </label>
              <label className="flex flex-col gap-1 text-xs">
                说明（可选）
                <input
                  value={roleEdit.descr}
                  onChange={(e) => setRoleEdit({ ...roleEdit, descr: e.target.value })}
                  className="min-h-[40px] w-48 rounded-[var(--r-sm)] border border-line bg-cloud px-2 text-sm"
                />
              </label>
              <button
                className="baozi-button"
                disabled={busy || !roleEdit.name.trim() || (roleEdit.mode === "new" && !roleEdit.key.trim())}
                onClick={() => guard(async () => {
                  const payload = { key: roleEdit.key.trim(), name: roleEdit.name.trim(), descr: roleEdit.descr.trim() || undefined };
                  if (roleEdit.mode === "new") {
                    await api.post("/api/v1/admin/roles", payload);
                  } else {
                    await api.put(`/api/v1/admin/roles/${encodeURIComponent(roleEdit.key)}`, payload);
                  }
                  setRoleDefs(await api.get<RoleDef[]>("/api/v1/admin/roles"));
                  setRoleEdit(null);
                }, "已保存")}
              >
                保存
              </button>
              <button
                className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
                onClick={() => setRoleEdit(null)}
              >
                取消
              </button>
            </div>
            {roleEdit.mode === "new" && (
              <p className="mt-2 text-xs text-sub">
                新建后到「权限配置」为其勾选权限，再到下方给用户授予。
              </p>
            )}
          </div>
        )}
        {!roleEdit && (
          <button
            className="self-start min-h-[36px] rounded-full border border-dashed border-line px-4 text-xs font-bold text-sky"
            onClick={() => setRoleEdit({ key: "", name: "", descr: "", mode: "new" })}
          >
            ＋ 新增职务（仅站长）
          </button>
        )}
      </div>

      <div className="mb-2 flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">按用户 ID 过滤</span>
          <input
            value={rFilter}
            onChange={(e) => setRFilter(e.target.value)}
            placeholder="留空显示全部"
            className="min-h-[40px] w-40 rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky"
          />
        </label>
        <button
          disabled={busy}
          className="min-h-[40px] rounded-full border border-line px-4 text-sm font-bold disabled:opacity-50"
          onClick={() => guard(async () => {
            const q = rFilter.trim() ? `?user_id=${encodeURIComponent(rFilter.trim())}` : "";
            setUserRoles(await api.get<UserRoleRow[]>(`/api/v1/admin/user-roles${q}`));
          }, "已刷新")}
        >
          刷新
        </button>
      </div>

      <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead w-20">用户</td>
              <td className="colhead w-24">职务</td>
              <td className="colhead w-20">授予人</td>
              <td className="colhead">授予时间</td>
              <td className="colhead">到期</td>
              <td className="colhead w-20" />
            </tr>
          </thead>
          <tbody>
            {userRoles
              .filter((r) => !rFilter.trim() || String(r.user_id) === rFilter.trim())
              .map((r) => (
                <tr key={`${r.user_id}-${r.role_key}`}>
                  <td className="rowfollow">#{r.user_id}</td>
                  <td className="rowfollow font-bold">
                    {roleDefs.find((d) => d.key === r.role_key)?.name ?? r.role_key}
                  </td>
                  <td className="rowfollow text-sub">
                    {r.granted_by ? `#${r.granted_by}` : "—"}
                  </td>
                  <td className="rowfollow text-sub">
                    {new Date(r.granted_at).toLocaleString(dateLocale(locale))}
                  </td>
                  <td className="rowfollow text-sub">
                    {r.expires_at ? new Date(r.expires_at).toLocaleString(dateLocale(locale)) : "永久"}
                  </td>
                  <td className="rowfollow">
                    <button
                      disabled={busy}
                      className="min-h-[28px] rounded-full border border-line px-3 font-bold text-danger disabled:opacity-50"
                      onClick={() => guard(async () => {
                        await api.del(`/api/v1/admin/user-roles/${r.user_id}/${r.role_key}`);
                        setUserRoles(await api.get<UserRoleRow[]>("/api/v1/admin/user-roles"));
                      }, "已撤销")}
                    >
                      撤销
                    </button>
                  </td>
                </tr>
              ))}
            {userRoles.length === 0 && (
              <tr>
                <td colSpan={6} className="py-4 text-center text-sub">
                  暂无职务授予记录
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="mt-4 flex flex-wrap items-end gap-2">
        <h3 className="w-full text-sm font-bold">授予职务</h3>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">用户 ID</span>
          <input
            value={rUserId}
            onChange={(e) => setRUserId(e.target.value)}
            className="min-h-[40px] w-28 rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky"
          />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">职务</span>
          <select
            value={rRoleKey}
            onChange={(e) => setRRoleKey(e.target.value)}
            className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky"
          >
            {roleDefs.map((r) => (
              <option key={r.key} value={r.key}>{r.name}</option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">到期（可选，RFC3339）</span>
          <input
            value={rExpires}
            onChange={(e) => setRExpires(e.target.value)}
            placeholder="留空为永久"
            className="min-h-[40px] w-56 rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky"
          />
        </label>
        <button
          disabled={busy || !rUserId.trim()}
          className="min-h-[40px] rounded-full bg-sky px-5 text-sm font-bold text-white disabled:opacity-50"
          onClick={() => guard(async () => {
            const payload: Record<string, unknown> = {
              user_id: Number(rUserId.trim()),
              role_key: rRoleKey,
            };
            if (rExpires.trim()) payload.expires_at = rExpires.trim();
            await api.post("/api/v1/admin/user-roles", payload);
            setUserRoles(await api.get<UserRoleRow[]>("/api/v1/admin/user-roles"));
            setRUserId("");
            setRExpires("");
          }, "已授予")}
        >
          授予
        </button>
      </div>
    </section>
  );
}
