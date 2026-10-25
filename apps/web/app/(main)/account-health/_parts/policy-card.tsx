import Link from "next/link";
import type { AccountHealthCopy } from "@/i18n/account-health";
import { fmt, type Locale } from "@/i18n/config";
import { day } from "./util";

export interface PolicyHr {
  seed_hours: number;
  days: number;
  warn_enable: boolean;
  prewarn_hours: number;
  violation_limit: number;
  warn_base_days: number;
  warn_max_shift_days: number;
}

export interface SitePolicy {
  schema: number;
  version: string;
  updated_at: string | null;
  hr: PolicyHr;
  risk_decay_days: number;
  risk_explanation_path: string;
  lottery_verification_path: string;
  lottery_algorithm: string;
}

/** 站点公开策略卡：H&R 与风险衰减的**生效值**由后端统一归一后下发，
 *  版本号是生效值的 SHA-256 前 8 字节——同一份口径供三个出口共用。 */
export function PolicyCard({
  t,
  locale,
  policy,
}: {
  t: AccountHealthCopy;
  locale: Locale;
  policy: SitePolicy;
}) {
  const hr = policy.hr;
  // worker 侧只在 max_shift_days > 0 时夹取，0 表示顺延不封顶
  let warnLine = t.policyWarnOff;
  if (hr.warn_enable) {
    warnLine =
      hr.warn_max_shift_days > 0
        ? fmt(t.policyWarn, {
            base: hr.warn_base_days,
            max: hr.warn_max_shift_days,
          })
        : fmt(t.policyWarnNoCap, { base: hr.warn_base_days });
  }
  return (
    <section className="brand-panel">
      <div className="brand-panel__head">
        <h2>{t.policyTitle}</h2>
      </div>
      <div className="flex flex-col gap-2 p-4 text-sm">
        <p className="-mt-1 text-xs text-sub">{t.policySub}</p>
        <p className="font-mono text-[11px] text-sub">
          {fmt(t.policyVersion, { version: policy.version })}
        </p>
        <p className="text-xs text-sub">
          {fmt(t.policyUpdated, {
            date: day(locale, policy.updated_at),
          })}
        </p>
        <ul className="flex list-disc flex-col gap-1 pl-5">
          <li>
            {fmt(t.policyHr, {
              hours: hr.seed_hours,
              days: hr.days,
            })}
          </li>
          {hr.prewarn_hours > 0 && (
            <li>
              {fmt(t.policyPrewarn, { hours: hr.prewarn_hours })}
            </li>
          )}
          <li>{fmt(t.policyLimit, { n: hr.violation_limit })}</li>
          <li>{warnLine}</li>
          <li>
            {fmt(t.policyDecay, { days: policy.risk_decay_days })}
          </li>
        </ul>
        <Link href="/myhr" className="text-link text-xs">
          {t.policyHrLink}
        </Link>
      </div>
    </section>
  );
}
