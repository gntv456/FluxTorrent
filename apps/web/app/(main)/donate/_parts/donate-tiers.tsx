/**
 * 捐赠回馈档位表（E7 从 _inner.tsx 拆出，300 行门禁）：
 * 累计实付达档自动发放（回调即时），档位配置来自后台 settings donation_tiers。
 */

import type { Dict } from "@/i18n/zh-CN";

export interface DonationTier {
  min_usd: number;
  label: string;
  spark: number;
  upload_gb: number;
  invites: number;
  medal_id: number;
}

export function DonateTiers({
  tiers,
  t,
}: {
  tiers: DonationTier[];
  t: Dict["donate"];
}) {
  if (tiers.length === 0) return null;
  return (
    <section className="baozi-panel">
      <div className="baozi-panel__head">
        <h2>{t.tiersTitle}</h2>
      </div>
      <p className="px-4 pt-2 text-xs text-sub">{t.tiersNote}</p>
      <div className="grid gap-3 p-4 sm:grid-cols-2 lg:grid-cols-3">
        {tiers.map((tier) => {
          const parts = [
            tier.spark > 0 && `${tier.spark} ${t.tierSpark}`,
            tier.upload_gb > 0 && `${tier.upload_gb}G ${t.tierUpload}`,
            tier.invites > 0 && `×${tier.invites} ${t.tierInvites}`,
            tier.medal_id > 0 && t.tierMedal,
          ].filter(Boolean);
          return (
            <div
              key={tier.label}
              className={"donate-tier rounded-[var(--r-sm)] "
                + "border border-line p-3"}
            >
              <p className="text-sm font-bold">
                {tier.label}
                <span className="ml-2 num text-xs text-sub">
                  ${tier.min_usd}
                </span>
              </p>
              <p className="mt-1 text-xs text-sub">{parts.join(" · ")}</p>
            </div>
          );
        })}
      </div>
    </section>
  );
}
