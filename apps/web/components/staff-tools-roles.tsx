"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

import { RoleDictSection } from "./staff-tools-roles-dict";

/** 职务管理面板（从 staff-tools.tsx 按域拆出，300 行门禁）：
 *  user_roles 口径——职务字典 CRUD + 用户职务授予/撤销。
 *  职务字典卡片区 + 新增/编辑表单拆至 ./staff-tools-roles-dict.tsx。 */

export interface RoleDef {
  key: string;
  name: string;
  descr: string | null;
}
interface UserRoleRow {
  user_id: number;
  role_key: string;
  granted_by: number | null;
  granted_at: string;
  expires_at: string | null;
}

// 过滤/授予区的输入框与主按钮
const INPUT_CLS =
  "min-h-[40px] rounded-[var(--r-sm)] border border-line " +
  "bg-cloud px-3 text-sm outline-none focus:border-sky";
const BTN_OUTLINE_M =
  "min-h-[40px] rounded-full border border-line px-4 text-sm " +
  "font-bold disabled:opacity-50";
const BTN_SKY =
  "min-h-[40px] rounded-full bg-sky px-5 text-sm font-bold " +
  "text-white disabled:opacity-50";

export function StaffRolesPanel({ flash }: { flash: (m: string) => void }) {
  const { dict, locale } = useI18n();
  const t = dict.adminRoles;
  const [roleDefs, setRoleDefs] = useState<RoleDef[]>([]);
  const [roleEdit, setRoleEdit] = useState<{
    key: string;
    name: string;
    descr: string;
    mode: "new" | "edit";
  } | null>(null);
  const [userRoles, setUserRoles] = useState<UserRoleRow[]>([]);
  const [rFilter, setRFilter] = useState("");
  const [rUserId, setRUserId] = useState("");
  const [rRoleKey, setRRoleKey] = useState("uploader");
  const [rExpires, setRExpires] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    api
      .get<RoleDef[]>("/api/v1/admin/roles")
      .then(setRoleDefs)
      .catch(() => setRoleDefs([]));
    api
      .get<UserRoleRow[]>("/api/v1/admin/user-roles")
      .then(setUserRoles)
      .catch(() => setUserRoles([]));
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  async function guard(fn: () => Promise<void>, ok: string) {
    setBusy(true);
    try {
      await fn();
      flash(ok);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-2 text-base font-bold">{t.title}</h2>
      <p className="mb-3 text-xs text-sub">{t.intro}</p>

      <RoleDictSection
        roleDefs={roleDefs}
        roleEdit={roleEdit}
        setRoleEdit={setRoleEdit}
        setRoleDefs={setRoleDefs}
        busy={busy}
        guard={guard}
      />

      <div className="mb-2 flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">{t.filterLabel}</span>
          <input
            value={rFilter}
            onChange={(e) => setRFilter(e.target.value)}
            placeholder={t.filterPh}
            className={`${INPUT_CLS} w-40`}
          />
        </label>
        <button
          disabled={busy}
          className={BTN_OUTLINE_M}
          onClick={() =>
            guard(async () => {
              const q = rFilter.trim()
                ? `?user_id=${encodeURIComponent(rFilter.trim())}`
                : "";
              setUserRoles(
                await api.get<UserRoleRow[]>(`/api/v1/admin/user-roles${q}`),
              );
            }, t.refreshed)
          }
        >
          {t.refreshBtn}
        </button>
      </div>

      <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead w-20">{t.colUser}</td>
              <td className="colhead w-24">{t.colRole}</td>
              <td className="colhead w-20">{t.colGrantor}</td>
              <td className="colhead">{t.colGrantedAt}</td>
              <td className="colhead">{t.colExpires}</td>
              <td className="colhead w-20" />
            </tr>
          </thead>
          <tbody>
            {userRoles
              .filter(
                (r) => !rFilter.trim() || String(r.user_id) === rFilter.trim(),
              )
              .map((r) => (
                <tr key={`${r.user_id}-${r.role_key}`}>
                  <td className="rowfollow">#{r.user_id}</td>
                  <td className="rowfollow font-bold">
                    {roleDefs.find((d) => d.key === r.role_key)?.name ??
                      r.role_key}
                  </td>
                  <td className="rowfollow text-sub">
                    {r.granted_by ? `#${r.granted_by}` : "—"}
                  </td>
                  <td className="rowfollow text-sub">
                    {new Date(r.granted_at).toLocaleString(dateLocale(locale))}
                  </td>
                  <td className="rowfollow text-sub">
                    {r.expires_at
                      ? new Date(r.expires_at).toLocaleString(
                          dateLocale(locale),
                        )
                      : t.forever}
                  </td>
                  <td className="rowfollow">
                    <button
                      disabled={busy}
                      className={
                        "min-h-[28px] rounded-full border border-line " +
                        "px-3 font-bold text-danger disabled:opacity-50"
                      }
                      onClick={() =>
                        guard(async () => {
                          await api.del(
                            `/api/v1/admin/user-roles/` +
                              `${r.user_id}/${r.role_key}`,
                          );
                          setUserRoles(
                            await api.get<UserRoleRow[]>(
                              "/api/v1/admin/user-roles",
                            ),
                          );
                        }, t.revoked)
                      }
                    >
                      {t.revokeBtn}
                    </button>
                  </td>
                </tr>
              ))}
            {userRoles.length === 0 && (
              <tr>
                <td colSpan={6} className="py-4 text-center text-sub">
                  {t.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="mt-4 flex flex-wrap items-end gap-2">
        <h3 className="w-full text-sm font-bold">{t.grantTitle}</h3>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">{t.fldUserId}</span>
          <input
            value={rUserId}
            onChange={(e) => setRUserId(e.target.value)}
            className={`${INPUT_CLS} w-28`}
          />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">{t.fldRole}</span>
          <select
            value={rRoleKey}
            onChange={(e) => setRRoleKey(e.target.value)}
            className={INPUT_CLS}
          >
            {roleDefs.map((r) => (
              <option key={r.key} value={r.key}>
                {r.name}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">{t.fldExpires}</span>
          <input
            value={rExpires}
            onChange={(e) => setRExpires(e.target.value)}
            placeholder={t.expiresPh}
            className={`${INPUT_CLS} w-56`}
          />
        </label>
        <button
          disabled={busy || !rUserId.trim()}
          className={BTN_SKY}
          onClick={() =>
            guard(async () => {
              const payload: Record<string, unknown> = {
                user_id: Number(rUserId.trim()),
                role_key: rRoleKey,
              };
              if (rExpires.trim()) payload.expires_at = rExpires.trim();
              await api.post("/api/v1/admin/user-roles", payload);
              setUserRoles(
                await api.get<UserRoleRow[]>("/api/v1/admin/user-roles"),
              );
              setRUserId("");
              setRExpires("");
            }, t.granted)
          }
        >
          {t.grantBtn}
        </button>
      </div>
    </section>
  );
}
