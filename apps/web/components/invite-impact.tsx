"use client";

import { useI18n } from "@/i18n/client";
import { Badge } from "@/components/ui/badge";

/** PM-5：邀请转化概览。数据来自 /invites/status，零新增请求。
 *  邀请是 PT 的主要增长通道，让邀请人看见自己邀请的质量。 */
export function InviteImpact({
  status,
}: {
  status: {
    used: number;
    unused: number;
    expired: number;
  } | null;
}) {
  const { dict } = useI18n();
  const u = dict.uxOpt;
  if (!status) return null;
  const total = status.used + status.unused;
  const rate = total > 0 ? Math.round((status.used / total) * 100) : 0;
  const items: [string, number][] = [
    [u.inviteImpactUsed, status.used],
    [u.inviteImpactUnused, status.unused],
    [u.inviteImpactExpired, status.expired],
  ];
  return (
    <section className="ux-invite-impact" aria-label={u.inviteImpactTitle}>
      <span className="ux-invite-impact__title">
        {u.inviteImpactTitle}
      </span>
      {items.map(([label, value]) => (
        <span key={label} className="ux-invite-impact__item">
          {label} <b>{value}</b>
        </span>
      ))}
      <span className="ux-invite-impact__item">
        {u.inviteImpactRate} <b>{rate}%</b>
      </span>
      <Badge tone="neutral" size="sm">
        {status.used}
      </Badge>
    </section>
  );
}
