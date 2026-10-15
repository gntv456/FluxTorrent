"use client";

/**
 * 反作弊规则准确率（0372 M5）：家族级误判率 + 处置结论分布。
 * 挂 in /cheaterbox（staff 反作弊控制台）——与案件队列、风险名单、申诉
 * 复核队列同屏，站长据此调权重/阈值。
 *
 * 口径：accuracy = 1 - 误判/已处置（已处置=0 时显示「样本不足」而不是
 * 100%——零样本给满分是误导）；false_positive 由申诉 overturned 置位，
 * 是误判的**下界**。
 */

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

interface FamilyRow {
  family: string;
  events: number;
  resolved: number;
  false_positive: number;
  hits: number;
  accuracy: number | null;
}

interface SanctionRow {
  kind: string;
  total: number;
  applied: number;
  reversed: number;
  expired: number;
  pending: number;
}

interface AccResp {
  days: number;
  families: FamilyRow[];
  sanctions: SanctionRow[];
}

const DAY_CHOICES = [30, 90, 365];

export function CheatAccuracyPanel() {
  const { dict } = useI18n();
  const t = dict.cheaterbox;
  const [days, setDays] = useState(90);
  const [data, setData] = useState<AccResp | null>(null);

  const load = useCallback(async () => {
    try {
      setData(
        await api.get<AccResp>(`/api/v1/admin/cheat-accuracy?days=${days}`),
      );
    } catch {
      setData(null);
    }
  }, [days]);
  useEffect(() => {
    void load();
  }, [load]);

  const th = "px-3 py-2 text-left text-xs font-bold whitespace-nowrap";
  const td = "px-3 py-2 text-sm align-middle";
  const acc = (v: number | null) =>
    v === null ? t.caNoSample : `${Math.round(v * 100)}%`;

  return (
    <section className="brand-panel mt-4">
      <header className="brand-panel__head">
        <h2>
          <span aria-hidden="true">🎯</span> {t.caTitle}
        </h2>
        <small>
          {fmt(t.caDays, { n: data?.days ?? days })}
        </small>
      </header>
      <p className="mb-2 text-xs text-sub">{t.caNote}</p>

      <div className="mb-2 flex gap-2">
        {DAY_CHOICES.map((d) => (
          <button
            key={d}
            type="button"
            className={
              "rounded-full border px-3 py-1 text-xs font-bold " +
              (days === d
                ? "border-[var(--brand-orange)] text-[var(--brand-orange-dark)]"
                : "border-[var(--border-soft)] text-sub")
            }
            onClick={() => setDays(d)}
          >
            {d}d
          </button>
        ))}
      </div>

      {data === null ? (
        <p className="funbox__empty">…</p>
      ) : (
        <>
          <div className="overflow-x-auto">
            <table className="w-full border-collapse">
              <thead>
                <tr className="border-b border-[var(--border-soft)]">
                  <th className={th}>{t.caColFamily}</th>
                  <th className={th}>{t.caColEvents}</th>
                  <th className={th}>{t.caColResolved}</th>
                  <th className={th}>{t.caColFp}</th>
                  <th className={th}>{t.caColHits}</th>
                  <th className={th}>{t.caColAccuracy}</th>
                </tr>
              </thead>
              <tbody>
                {data.families.map((r) => (
                  <tr
                    key={r.family}
                    className={
                      "border-b border-dashed " +
                      "border-[var(--border-soft)]"
                    }
                  >
                    <td className={`${td} font-bold`}>{r.family}</td>
                    <td className={td}>{r.events}</td>
                    <td className={td}>{r.resolved}</td>
                    <td
                      className={
                        r.false_positive > 0
                          ? `${td} font-bold ` +
                            "text-[var(--danger)]"
                          : td
                      }
                    >
                      {r.false_positive}
                    </td>
                    <td className={td}>{r.hits}</td>
                    <td className={td}>{acc(r.accuracy)}</td>
                  </tr>
                ))}
                {data.families.length === 0 && (
                  <tr>
                    <td colSpan={6} className="py-6 text-center text-sub">
                      {t.caEmpty}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>

          {data.sanctions.length > 0 && (
            <div className="mt-4 overflow-x-auto">
              <p className="mb-1 text-xs font-bold">
                {t.caSanctionTitle}
              </p>
              <table className="w-full border-collapse">
                <thead>
                  <tr className="border-b border-[var(--border-soft)]">
                    <th className={th}>{t.caColKind}</th>
                    <th className={th}>{t.caColTotal}</th>
                    <th className={th}>{t.caColApplied}</th>
                    <th className={th}>{t.caColReversed}</th>
                    <th className={th}>{t.caColExpired}</th>
                    <th className={th}>{t.caColPending}</th>
                  </tr>
                </thead>
                <tbody>
                  {data.sanctions.map((s) => (
                    <tr
                      key={s.kind}
                      className={
                        "border-b border-dashed " +
                        "border-[var(--border-soft)]"
                      }
                    >
                      <td className={`${td} font-bold`}>{s.kind}</td>
                      <td className={td}>{s.total}</td>
                      <td className={td}>{s.applied}</td>
                      <td className={td}>{s.reversed}</td>
                      <td className={td}>{s.expired}</td>
                      <td className={td}>{s.pending}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </>
      )}
    </section>
  );
}
