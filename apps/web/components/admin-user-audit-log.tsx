"use client";

import { useI18n } from "@/i18n/client";
import type { AuditEntryRow } from "./admin-user-detail-shared";

/** 0482 用户日志 tab（后台用户详情第五 tab）：audit_log 按 uid 回查的
 *  审计现场——这个号被改过什么、谁改的、什么时候改的。动作名走
 *  i18n `userDetail.auditActions` 人话映射，取不到回落原 key；
 *  detail 是审计 ref 原文（目标 id / 理由 / 附加上下文，0285 口径）。 */
export function UserAuditLogPanel({
  audit,
}: {
  audit: AuditEntryRow[] | null;
}) {
  const { dict } = useI18n();
  const u = dict.userDetail;
  const actionLabel = (k: string) =>
    (u.auditActions as Record<string, string> | undefined)?.[k] ?? k;
  return (
    <section className="brand-panel p-4">
      <div className="brand-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead">{u.auditColTime}</td>
              <td className="colhead">{u.auditColAction}</td>
              <td className="colhead">{u.auditColActor}</td>
              <td className="colhead">{u.auditColDetail}</td>
            </tr>
          </thead>
          <tbody>
            {audit?.map((r) => (
              <tr key={r.id}>
                <td className="text-sub">
                  {new Date(r.created_at).toLocaleString()}
                </td>
                <td className="font-bold">{actionLabel(r.action)}</td>
                <td>
                  {r.actor_username ?? (r.actor_id ? `#${r.actor_id}` : "—")}
                </td>
                <td className="max-w-[420px] break-all font-mono text-sub">
                  {r.detail ?? "—"}
                </td>
              </tr>
            ))}
            {audit?.length === 0 && (
              <tr>
                <td colSpan={4} className="py-4 text-center text-sub">
                  {u.auditEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}
