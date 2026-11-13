"use client";

import { useI18n } from "@/i18n/client";
import type { AuditEntryRow } from "./admin-user-detail-shared";

/** 审计现场 → 人话一行（0482）：理由 / 前后值 / 调整增量 / 警告周数，
 *  其余键原样带出（保留取证语义）；解析不了就回落原文。字节增量按 GB。
 *  理由前缀走 i18n（auditReason）；前缀经参传入保持函数纯。 */
function renderDetail(raw: string | null, reasonPrefix: string): string {
  if (!raw) return "—";
  let v: unknown;
  try {
    v = JSON.parse(raw);
  } catch {
    return raw;
  }
  if (typeof v !== "object" || v === null || Array.isArray(v)) return raw;
  const o = v as Record<string, unknown>;
  const compact = (x: unknown): string =>
    typeof x === "string" ? x : JSON.stringify(x);
  const gb = (n: unknown): string =>
    typeof n === "number"
      ? `${(n / 1073741824).toFixed(2).replace(/\.?0+$/, "")}GB`
      : compact(n);
  const parts: string[] = [];
  if (typeof o.reason === "string" && o.reason) {
    parts.push(`${reasonPrefix}${o.reason}`);
  }
  if ("before" in o || "after" in o) {
    const side = (x: unknown): string =>
      x !== null && typeof x === "object"
        ? compact(x)
        : typeof x === "number" && Math.abs(x) > 1073741824
          ? gb(x)
          : compact(x ?? "—");
    parts.push(`${side(o.before)} → ${side(o.after)}`);
  }
  const GB_KEYS = new Set(["up_delta", "dl_delta"]);
  for (const k of [
    "up_delta",
    "dl_delta",
    "spark_delta",
    "invite_delta",
    "weeks",
  ]) {
    if (k in o) {
      parts.push(`${k}=${GB_KEYS.has(k) ? gb(o[k]) : String(o[k] ?? "—")}`);
    }
  }
  const known = new Set([
    "id",
    "reason",
    "before",
    "after",
    "up_delta",
    "dl_delta",
    "spark_delta",
    "invite_delta",
    "weeks",
  ]);
  for (const [k, val] of Object.entries(o)) {
    if (!known.has(k)) parts.push(`${k}=${compact(val)}`);
  }
  return parts.join(" · ") || raw;
}

/** 0482 用户日志 tab（后台用户详情第五 tab）：audit_log 按 uid 回查的
 *  审计现场——这个号被改过什么、谁改的、什么时候改的。动作名走
 *  i18n `userDetail.auditActions` 人话映射，取不到回落原 key。 */
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
              <th scope="col" className="colhead">{u.auditColTime}</th>
              <th scope="col" className="colhead">{u.auditColAction}</th>
              <th scope="col" className="colhead">{u.auditColActor}</th>
              <th scope="col" className="colhead">{u.auditColDetail}</th>
            </tr>
          </thead>
          <tbody>
            {audit?.map((r) => {
              const detail = renderDetail(r.detail, u.auditReason);
              return (
                <tr key={r.id}>
                  <td className="text-sub">
                    {new Date(r.created_at).toLocaleString()}
                  </td>
                  <td className="font-bold">{actionLabel(r.action)}</td>
                  <td>
                    {r.actor_username ?? (r.actor_id ? `#${r.actor_id}` : "—")}
                  </td>
                  <td
                    className="max-w-[420px] break-all font-mono text-sub"
                    title={r.detail ?? ""}
                  >
                    {detail}
                  </td>
                </tr>
              );
            })}
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
