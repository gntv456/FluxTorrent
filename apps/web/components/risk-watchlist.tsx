"use client";

import { useCallback, useEffect, useState, Fragment } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { ErrorPanel } from "@/components/ui/error-panel";

/** 观察名单行（GET /admin/risk-scores，0368 M2） */
interface RiskRow {
  user_id: number;
  username: string;
  score: number;
  band: string;
  ruleset_ver: number;
  updated_at: string;
  open_case: number | null;
  live_evidence: number;
  live_hits: number;
  shadow_breakdown: {
    families?: Record<
      string,
      { hits: number; days: number; weight: number; points: number }
    >;
    norm?: number;
    half_life_days?: number;
    ruleset_ver?: number;
  } | null;
}

/// watch+ = 观察名单（staff 可见，用户无感）；low/none 只是候选池
const WATCH_BANDS = ["watch", "high", "critical"];

/**
 * 风险观察名单（0368 M2）：worker/jobs/risk_score.rs 周期重算落库的
 * 用户风险分（0-100）+ 档位 + 分数构成。与案件队列互补——案件是
 * 「已立案待办」，名单是「按可疑度排序的巡检面」，high/critical 行
 * 多数已自动开案（open_case 列可见）。
 */
export function RiskWatchlist() {
  const { dict, locale } = useI18n();
  const t = dict.cheaterbox;
  const [watchOnly, setWatchOnly] = useState(true);
  const [rows, setRows] = useState<RiskRow[] | null>(null);
  const [openId, setOpenId] = useState<number | null>(null);
  const [err, setErr] = useState<unknown>(null);

  const load = useCallback(() => {
    setRows(null);
    setErr(null);
    api
      .get<{ items: RiskRow[] }>("/api/v1/admin/risk-scores?limit=200")
      .then((r) => setRows(r.items))
      .catch((e) => setErr(e));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  // 服务端只支持精确 band 过滤，watch+ 三档合一在前侧做（入册用户量小）
  const items = (rows ?? []).filter(
    (r) => !watchOnly || WATCH_BANDS.includes(r.band),
  );

  const bandLabel = (b: string) =>
    b === "critical"
      ? t.bandCritical
      : b === "high"
        ? t.bandHigh
        : b === "watch"
          ? t.bandWatch
          : b === "low"
            ? t.bandLow
            : t.bandNone;

  const bandCls = (b: string) =>
    b === "critical"
      ? "font-bold text-[var(--danger)]"
      : b === "high"
        ? "font-bold text-[var(--baozi-orange-dark)]"
        : b === "watch"
          ? "text-[var(--baozi-orange-dark)]"
          : "text-sub";

  const th = "px-3 py-2 text-left text-xs font-bold whitespace-nowrap";
  const td = "px-3 py-2 text-sm align-middle";
  const filterBtn = (w: boolean) =>
    "rounded-full border px-3 py-1 text-xs font-bold " +
    (watchOnly === w
      ? "border-[var(--baozi-orange)] text-[var(--baozi-orange-dark)]"
      : "border-[var(--border-soft)] text-sub");

  return (
    <section className="baozi-panel mt-4">
      <header className="baozi-panel__head">
        <h2>
          <span aria-hidden="true">🧭</span> {t.riskTitle}
        </h2>
        {rows !== null && <small>{items.length}</small>}
      </header>
      <p className="mb-2 text-xs text-sub">{t.riskNote}</p>

      <div className="mb-2 flex gap-2">
        {[true, false].map((w) => (
          <button
            key={String(w)}
            type="button"
            className={filterBtn(w)}
            onClick={() => setWatchOnly(w)}
          >
            {w ? t.riskFilterWatch : t.riskFilterAll}
          </button>
        ))}
      </div>

      {err !== null ? (
        <ErrorPanel error={err} onRetry={load} />
      ) : rows === null ? (
        <p className="funbox__empty">…</p>
      ) : items.length === 0 ? (
        <p className="funbox__empty">{t.riskEmpty}</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full border-collapse">
            <thead>
              <tr className="border-b border-[var(--border-soft)]">
                <th className={th}>{t.colUser}</th>
                <th className={th}>{t.colRiskScore}</th>
                <th className={th}>{t.colRiskBand}</th>
                <th className={th}>{t.colRiskEvidence}</th>
                <th className={th}>{t.colRiskUpdated}</th>
                <th className={th}>{t.colCaseId}</th>
                <th className={th} aria-hidden="true" />
              </tr>
            </thead>
            <tbody>
              {items.map((r) => (
                <Fragment key={r.user_id}>
                  <tr
                    className={
                      "border-b border-dashed " +
                      "border-[var(--border-soft)]"
                    }
                  >
                    <td className={td}>
                      <a
                        className={
                          "font-bold " +
                          "text-[var(--baozi-orange-dark)] " +
                          "hover:underline"
                        }
                        href={`/users/${r.user_id}`}
                      >
                        {r.username}
                      </a>
                    </td>
                    <td className={`${td} font-bold`}>{r.score}</td>
                    <td className={td}>
                      <span className={bandCls(r.band)}>
                        {bandLabel(r.band)}
                      </span>
                    </td>
                    <td className={td}>{r.live_evidence}</td>
                    <td
                      className={
                        `${td} whitespace-nowrap text-xs ` +
                        "text-[var(--text-faint)]"
                      }
                    >
                      {new Date(r.updated_at).toLocaleString(locale)}
                    </td>
                    <td className={`${td} whitespace-nowrap text-xs`}>
                      {r.open_case ? `#${r.open_case}` : "—"}
                    </td>
                    <td className={`${td} whitespace-nowrap`}>
                      <button
                        type="button"
                        className="text-xs text-sub hover:underline"
                        onClick={() =>
                          setOpenId(openId === r.user_id ? null : r.user_id)
                        }
                      >
                        {openId === r.user_id ? t.collapse : t.detail}
                      </button>
                    </td>
                  </tr>
                  {openId === r.user_id && (
                    <tr>
                      <td
                        colSpan={7}
                        className="bg-[var(--bg-soft)] px-3 py-2"
                      >
                        <RiskBreakdown row={r} />
                      </td>
                    </tr>
                  )}
                </Fragment>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

/** 展开行：shadow_breakdown 的各家族贡献（可解释性）。 */
function RiskBreakdown({ row }: { row: RiskRow }) {
  const { dict } = useI18n();
  const t = dict.cheaterbox;
  const fams = Object.entries(row.shadow_breakdown?.families ?? {}).sort(
    (a, b) => b[1].points - a[1].points,
  );
  if (fams.length === 0) {
    return <p className="text-xs text-sub">{t.riskNoCompose}</p>;
  }
  return (
    <div>
      <p className="mb-1 text-xs font-bold">{t.colRiskCompose}</p>
      <ul className="text-xs text-sub">
        {fams.map(([fam, v]) => (
          <li key={fam}>
            {fam} × {v.hits}（{v.days.toFixed(1)} 天前 · 权重 {v.weight} ·
            贡献 {v.points.toFixed(1)} 分）
          </li>
        ))}
      </ul>
    </div>
  );
}
