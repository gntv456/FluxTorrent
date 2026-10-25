import Link from "next/link";
import type { AccountHealthCopy } from "@/i18n/account-health";
import { fmt, type Locale } from "@/i18n/config";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";
import { AppealForm } from "./appeal-form";
import { day } from "./util";

export interface RiskReason {
  code: string;
  hits: number;
  torrent_ids: number[];
  first_seen: string | null;
  last_seen: string | null;
  detail: string;
}

export interface RiskCase {
  id: number;
  status: number;
  reason: string;
  priority: number;
  risk_score: number | null;
  created_at: string;
}

export interface RiskAppeal {
  id: number;
  case_id: number | null;
  status: string;
  outcome: string | null;
  result_note: string | null;
  created_at: string;
}

export interface MyRisk {
  score: number;
  band: string;
  ruleset_ver: number;
  updated_at: string | null;
  decay_half_life_days: number;
  reasons: RiskReason[];
  open_case: RiskCase | null;
  latest_appeal: RiskAppeal | null;
  appeal_available: boolean;
  /** 当前案件是否已有未结申诉（latest_appeal 可能属于别的案件） */
  case_pending: boolean;
  open_appeals: number;
  appeal_limit: number;
  appeal_path: string;
}

const BAND_TONE: Record<
  string,
  "brand" | "neutral" | "success" | "warn" | "danger"
> = {
  none: "success",
  low: "neutral",
  watch: "warn",
  high: "danger",
  critical: "danger",
};

function appealLabel(a: RiskAppeal, t: AccountHealthCopy): string {
  if (a.status === "open") return t.appealStatus.open ?? a.status;
  if (a.outcome) return t.outcome[a.outcome] ?? a.outcome;
  return t.appealStatus[a.status] ?? a.status;
}

/** 本人风险卡：分数 + 原因码聚合 + 进行中案件与申诉入口。
 *  数据来自 GET /me/risk；peer_ip、判定权重与阈值后端刻意不下发。 */
export function RiskCard({
  t,
  locale,
  risk,
}: {
  t: AccountHealthCopy;
  locale: Locale;
  risk: MyRisk;
}) {
  const caseStatus = risk.open_case
    ? t.caseStatus[String(risk.open_case.status)] ??
      `#${risk.open_case.status}`
    : null;
  return (
    <section className="brand-panel">
      <div className="brand-panel__head">
        <h2>{t.riskTitle}</h2>
        <Badge tone={BAND_TONE[risk.band] ?? "neutral"}>
          {t.band[risk.band] ?? risk.band}
        </Badge>
      </div>
      <div className="flex flex-col gap-4 p-4">
        <div className="flex flex-wrap items-end gap-x-6 gap-y-2">
          <div>
            <p className="text-xs text-sub">{t.scoreLabel}</p>
            <p className="font-display text-3xl leading-none">
              {risk.score}
            </p>
          </div>
          <p className="text-xs text-sub">
            {fmt(t.ruleset, { ruleset: risk.ruleset_ver })}
          </p>
          <p className="text-xs text-sub">
            {t.evaluated}: {day(locale, risk.updated_at)}
          </p>
          <p className="text-xs text-sub">
            {fmt(t.decay, { days: risk.decay_half_life_days })}
          </p>
        </div>
        <p className="-mt-2 text-xs text-sub">{t.riskSub}</p>

        <div>
          <h3 className="mb-2 font-display text-base">
            {t.reasonsTitle}
          </h3>
          {risk.reasons.length === 0 ? (
            <EmptyState
              compact
              icon="shield"
              title={t.reasonsEmpty}
              desc={t.reasonsEmptyDesc}
            />
          ) : (
            <ul className="flex flex-col divide-y divide-line">
              {risk.reasons.map((r) => (
                <li key={r.code} className="py-2">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="text-sm font-bold">
                      {t.reasons[r.code] ?? r.code}
                    </span>
                    <Badge tone="warn" size="sm">
                      {fmt(t.hits, { n: r.hits })}
                    </Badge>
                    <span className="ml-auto text-[11px] text-sub">
                      {t.lastSeen}: {day(locale, r.last_seen)}
                    </span>
                  </div>
                  {r.detail && (
                    <p className="mt-1 text-xs text-sub">{r.detail}</p>
                  )}
                  {r.torrent_ids.length > 0 && (
                    <p
                      className="mt-1 flex flex-wrap gap-x-2 text-[11px]
                        text-sub"
                    >
                      <span>{t.torrents}:</span>
                      {r.torrent_ids.slice(0, 8).map((id) => (
                        <Link
                          key={id}
                          href={`/torrent/${id}`}
                          className="text-link"
                        >
                          #{id}
                        </Link>
                      ))}
                      {r.torrent_ids.length > 8 && (
                        <span>+{r.torrent_ids.length - 8}</span>
                      )}
                    </p>
                  )}
                </li>
              ))}
            </ul>
          )}
        </div>

        <div
          id="appeal"
          className="flex flex-col gap-2 rounded-[var(--r-md)]
            border border-line p-3"
        >
          <div className="flex items-center justify-between gap-2">
            <h3 className="font-display text-base">{t.caseTitle}</h3>
            {caseStatus && (
              <Badge tone="warn" size="sm">
                {caseStatus}
              </Badge>
            )}
          </div>
          {risk.open_case ? (
            <>
              <p className="text-sm font-bold">
                {fmt(t.caseId, { id: risk.open_case.id })}
              </p>
              <p className="text-xs text-sub">
                {t.openedAt}: {day(locale, risk.open_case.created_at)}
              </p>
              <p className="text-sm whitespace-pre-wrap">
                {risk.open_case.reason}
              </p>
              {risk.appeal_available ? (
                <AppealForm
                  caseId={risk.open_case.id}
                  copy={t.appeal}
                />
              ) : (
                <p className="text-sm text-sub">
                  {risk.case_pending
                    ? t.appealPending
                    : fmt(t.appealLimit, {
                        open: risk.open_appeals,
                        limit: risk.appeal_limit,
                      })}
                </p>
              )}
            </>
          ) : (
            <p className="text-sm text-sub">{t.noCase}</p>
          )}
          {risk.latest_appeal && (
            <div className="border-t border-line pt-2">
              <p className="text-xs text-sub">{t.latestTitle}</p>
              <p className="text-sm">
                {appealLabel(risk.latest_appeal, t)}
                {" · "}
                {day(locale, risk.latest_appeal.created_at)}
              </p>
              {risk.latest_appeal.result_note && (
                <p className="mt-1 text-sm text-sub">
                  {risk.latest_appeal.result_note}
                </p>
              )}
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
