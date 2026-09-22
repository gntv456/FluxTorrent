"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

import type { PermDef, PermMatrixData, RoleDef } from "./staff-tools-perm";

/** 权限配置面板·角色权限矩阵（从 components/staff-tools-perm.tsx
 *  按域拆出）：勾选即生效（仅站长可改）。
 *  等级为累进式，勾选低档会同时作用于更高档；
 *  职务权限仅对持有该职务的用户生效。 */

// 「未接入」徽标
const NOT_IMPL_TAG = "ml-1 rounded-full bg-sun/40 px-1.5 text-[10px] text-ink";
// 角色列表头 / 权限类别的显示名在 i18n（adminPerm.roleHeads / adminPerm.cats）；
// 取不到时回落后端下发的 name / 原始 key。

export function RoleMatrix({
  permData,
  roleDefs: _roleDefs,
  busy,
  setPermData,
  guard,
}: {
  permData: PermMatrixData | null;
  roleDefs: RoleDef[];
  busy: boolean;
  setPermData: React.Dispatch<React.SetStateAction<PermMatrixData | null>>;
  guard: (fn: () => Promise<void>, ok: string) => void;
}) {
  const { dict } = useI18n();
  const t = dict.adminPerm;
  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-2 text-base font-bold">{t.matrixTitle}</h2>
      <p className="mb-3 text-xs text-sub">{t.matrixIntro}</p>
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead" style={{ minWidth: 210 }}>
                {t.colPerm}
              </td>
              {(permData?.roles ?? []).map((r) => (
                <td
                  key={`${r.role_type}:${r.role_key}`}
                  className="colhead text-center"
                  style={{ minWidth: 70 }}
                >
                  <span className="block">
                    {t.roleHeads[r.role_key] ?? r.name}
                  </span>
                  <span className="block text-[10px] font-normal">
                    {r.role_type === "class"
                      ? fmt(t.roleIsClass, { key: r.role_key })
                      : t.roleIsRole}
                  </span>
                </td>
              ))}
            </tr>
          </thead>
          <tbody>
            {Object.entries(
              (permData?.permissions ?? []).reduce<Record<string, PermDef[]>>(
                (m, x) => {
                  (m[x.category] ||= []).push(x);
                  return m;
                },
                {},
              ),
            ).map(([cat, items]) => (
              <>
                <tr key={cat}>
                  <td
                    colSpan={1 + (permData?.roles ?? []).length}
                    className="bg-sky-soft font-bold"
                  >
                    {t.cats[cat] ?? cat}
                  </td>
                </tr>
                {items.map((p) => (
                  <tr key={p.key}>
                    <td className="rowfollow">
                      <span className="font-bold">{p.name}</span>
                      {!p.implemented && (
                        <span
                          className={NOT_IMPL_TAG}
                          title={t.notImplTip}
                        >
                          {t.notImpl}
                        </span>
                      )}
                      <span className="block font-mono text-[10px] text-sub">
                        {p.key}
                      </span>
                    </td>
                    {(permData?.roles ?? []).map((r) => {
                      const on = (permData?.grants ?? []).some(
                        (g) =>
                          g.role_type === r.role_type &&
                          g.role_key === r.role_key &&
                          g.permission_key === p.key,
                      );
                      return (
                        <td
                          key={`${r.role_type}:${r.role_key}:${p.key}`}
                          className="rowfollow text-center"
                        >
                          <input
                            type="checkbox"
                            checked={on}
                            disabled={busy}
                            onChange={(e) =>
                              guard(
                                async () => {
                                  await api.put(
                                    "/api/v1/admin/permission-matrix",
                                    {
                                      items: [
                                        {
                                          role_type: r.role_type,
                                          role_key: r.role_key,
                                          permission_key: p.key,
                                          granted: e.target.checked,
                                        },
                                      ],
                                    },
                                  );
                                  setPermData((prev) => {
                                    if (!prev) return prev;
                                    const grants = e.target.checked
                                      ? [
                                          ...prev.grants,
                                          {
                                            role_type: r.role_type,
                                            role_key: r.role_key,
                                            permission_key: p.key,
                                          },
                                        ]
                                      : prev.grants.filter(
                                          (g) =>
                                            !(
                                              g.role_type === r.role_type &&
                                              g.role_key === r.role_key &&
                                              g.permission_key === p.key
                                            ),
                                        );
                                    return { ...prev, grants };
                                  });
                                },
                                e.target.checked ? t.granted : t.revoked,
                              )
                            }
                          />
                        </td>
                      );
                    })}
                  </tr>
                ))}
              </>
            ))}
            {(permData?.permissions ?? []).length === 0 && (
              <tr>
                <td colSpan={8} className="py-4 text-center text-sub">
                  {t.permEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}
