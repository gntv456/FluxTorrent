"use client";

import { Fragment } from "react";
import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

import { RoleMatrix } from "./staff-tools-perm-matrix";

/** 权限配置面板（从 staff-tools.tsx 按域拆出，300 行门禁）：
 *  角色权限矩阵 + 用户级权限分配（对标 NexusPHP 角色插件）。
 *  角色权限矩阵拆至 ./staff-tools-perm-matrix.tsx。 */

export interface RoleDef {
  key: string;
  name: string;
  descr: string | null;
}
export interface PermDef {
  key: string;
  name: string;
  category: string;
  descr: string | null;
  implemented: boolean;
}
interface PermRole {
  role_type: string;
  role_key: string;
  name: string;
}
interface PermGrant {
  role_type: string;
  role_key: string;
  permission_key: string;
}
export interface PermMatrixData {
  permissions: PermDef[];
  roles: PermRole[];
  grants: PermGrant[];
}
interface UserPermData {
  user_id: number;
  class_id: number;
  roles: string[];
  effective: string[];
  overrides: { permission_key: string; granted: boolean }[];
}

// 用户级权限区：输入框 / 加载按钮 / 三态操作按钮底色
const UP_INPUT =
  "min-h-[40px] w-32 rounded-[var(--r-sm)] border border-line " +
  "bg-cloud px-3 text-sm outline-none focus:border-sky";
const UP_LOAD_BTN =
  "min-h-[40px] rounded-full bg-sky px-5 text-sm font-bold " +
  "text-white disabled:opacity-50";
const NOT_IMPL_TAG = "ml-1 rounded-full bg-sun/40 px-1.5 text-[10px] text-ink";

export function StaffPermPanel({ flash }: { flash: (m: string) => void }) {
  const { dict } = useI18n();
  const t = dict.adminPerm;
  const [permData, setPermData] = useState<PermMatrixData | null>(null);
  const [upUserId, setUpUserId] = useState("");
  const [upData, setUpData] = useState<UserPermData | null>(null);
  const [roleDefs, setRoleDefs] = useState<RoleDef[]>([]);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    api
      .get<PermMatrixData>("/api/v1/admin/permission-matrix")
      .then(setPermData)
      .catch(() => setPermData(null));
    api
      .get<RoleDef[]>("/api/v1/admin/roles")
      .then(setRoleDefs)
      .catch(() => setRoleDefs([]));
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
    <div className="flex flex-col gap-3">
      <RoleMatrix
        permData={permData}
        roleDefs={roleDefs}
        busy={busy}
        setPermData={setPermData}
        guard={guard}
      />

      <section className="baozi-panel p-4">
        <h2 className="mb-2 text-base font-bold">{t.userTitle}</h2>
        <p className="mb-3 text-xs text-sub">{t.userIntro}</p>

        <div className="mb-3 flex flex-wrap items-end gap-2">
          <label className="flex flex-col gap-1">
            <span className="text-xs text-sub">{t.fldUserId}</span>
            <input
              value={upUserId}
              onChange={(e) => setUpUserId(e.target.value)}
              className={UP_INPUT}
            />
          </label>
          <button
            disabled={busy || !upUserId.trim()}
            className={UP_LOAD_BTN}
            onClick={() =>
              guard(async () => {
                setUpData(
                  await api.get<UserPermData>(
                    `/api/v1/admin/user-permissions` +
                      `?user_id=${encodeURIComponent(upUserId.trim())}`,
                  ),
                );
              }, t.loaded)
            }
          >
            {t.loadBtn}
          </button>
          {upData && (
            <span className="pb-2 text-xs text-sub">
              {fmt(t.userMeta, { id: upData.user_id, cls: upData.class_id })}
              {upData.roles.length > 0
                ? fmt(t.userRoles, {
                    roles: upData.roles
                      .map((k) => roleDefs.find((d) => d.key === k)?.name ?? k)
                      .join("、"),
                  })
                : t.noRoles}
            </span>
          )}
        </div>

        {upData && (
          <div className="baozi-wide-table-scroll">
            <table className="nexus-table text-xs">
              <thead>
                <tr>
                  <td className="colhead" style={{ minWidth: 200 }}>
                    {t.colPerm}
                  </td>
                  <td className="colhead w-28">{t.colState}</td>
                  <td className="colhead" style={{ minWidth: 220 }}>
                    {t.colOverride}
                  </td>
                </tr>
              </thead>
              <tbody>
                {(permData?.permissions ?? []).map((p) => {
                  const ov = upData.overrides.find(
                    (o) => o.permission_key === p.key,
                  );
                  const eff = upData.effective.includes(p.key);
                  const state = ov
                    ? ov.granted
                      ? t.stateGrant
                      : t.stateDeny
                    : eff
                      ? t.stateInherit
                      : t.stateNone;
                  const btn = (label: string, g: boolean | null) => (
                    <button
                      key={label}
                      disabled={busy}
                      className={
                        "mr-1 min-h-[28px] rounded-full border px-3 " +
                        "font-bold disabled:opacity-50 " +
                        ((g === null && !ov) ||
                        (g === true && ov?.granted === true) ||
                        (g === false && ov?.granted === false)
                          ? "border-sky bg-sky text-white"
                          : "border-line")
                      }
                      onClick={() =>
                        guard(async () => {
                          await api.put("/api/v1/admin/user-permissions", {
                            user_id: upData.user_id,
                            permission_key: p.key,
                            granted: g,
                          });
                          setUpData(
                            await api.get<UserPermData>(
                              `/api/v1/admin/user-permissions` +
                                `?user_id=${upData.user_id}`,
                            ),
                          );
                        }, t.updated)
                      }
                    >
                      {label}
                    </button>
                  );
                  return (
                    <tr key={p.key}>
                      <td className="rowfollow">
                        <span className="font-bold">{p.name}</span>
                        {!p.implemented && (
                          <span className={NOT_IMPL_TAG}>{t.notImpl}</span>
                        )}
                        <span className="block font-mono text-[10px] text-sub">
                          {p.key}
                        </span>
                      </td>
                      <td className="rowfollow text-sub">{state}</td>
                      <td className="rowfollow">
                        {btn(t.btnInherit, null)}
                        {btn(t.btnGrant, true)}
                        {btn(t.btnDeny, false)}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </div>
  );
}
