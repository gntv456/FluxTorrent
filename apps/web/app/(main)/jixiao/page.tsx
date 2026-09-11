"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

interface JixiaoType {
  id: number;
  name: string;
  base_pay: number;
  metrics: Record<string, unknown>;
  min_requirements: Record<string, number>;
}

interface MyClaim {
  id: number;
  type_name: string;
  period: string;
  amount: number;
  claimed_at: string;
}

/** 绩效考核（jixiao 口径）：指标全系统自动采集，按月领取工资 */
export default function JixiaoPage() {
  const { dict, locale } = useI18n();
  const t = dict.jixiao2;
  const [types, setTypes] = useState<JixiaoType[]>([]);
  const [claims, setClaims] = useState<MyClaim[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    api.get<JixiaoType[]>("/api/v1/jixiao/types").then(setTypes).catch(() => setTypes([]));
    api.get<MyClaim[]>("/api/v1/jixiao/my").then(setClaims).catch(() => setClaims([]));
  }, []);
  useEffect(load, [load]);

  function flash(m: string) {
    setMsg(m);
    setTimeout(() => setMsg(null), 3500);
  }

  const reqLabel: Record<string, string> = {
    uploaded: t.mUploaded,
    downloaded: t.mDownloaded,
    uploads: t.mUploads,
    seeding_count: t.mSeeding,
    seed_size: t.mSeedSize,
    ops: t.mOps,
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{t.title}</h1>
        <span className="text-sm text-sub">{t.subtitle}</span>
      </div>
      {msg && <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>}

      <table className="nexus-table">
        <tbody>
          <tr>
            <td className="colhead" colSpan={3}>
              <h2 className="font-display">{t.typesTitle}</h2>
            </td>
          </tr>
          <tr>
            <td className="colhead">{t.colName}</td>
            <td className="colhead">{t.colBasePay}</td>
            <td className="colhead text-right">{t.colAction}</td>
          </tr>
          {types.map((ty) => (
            <tr key={ty.id}>
              <td>
                <span className="font-bold">{ty.name}</span>
                <ul className="mt-1 text-xs text-sub">
                  {Object.entries(ty.min_requirements)
                    .filter(([, v]) => v > 0)
                    .map(([k, v]) => (
                      <li key={k}>
                        {reqLabel[k] ?? k} ≥ {v.toLocaleString()}
                      </li>
                    ))}
                </ul>
              </td>
              <td className="num">✨ {ty.base_pay.toLocaleString()}</td>
              <td className="text-right">
                <button
                  className="cmgmt-act cmgmt-act--ok"
                  disabled={busy}
                  onClick={async () => {
                    setBusy(true);
                    try {
                      const r = await api.post<{ total: number }>("/api/v1/jixiao/claim", {
                        type_id: ty.id,
                      });
                      flash(t.claimed.replace("{n}", r.total.toLocaleString()));
                      load();
                    } catch (e) {
                      flash(e instanceof ApiError ? e.message : dict.common.networkError);
                    } finally {
                      setBusy(false);
                    }
                  }}
                >
                  {t.btnClaim}
                </button>
              </td>
            </tr>
          ))}
          {types.length === 0 && (
            <tr>
              <td colSpan={3} className="py-6 text-center text-sub">
                {t.typesEmpty}
              </td>
            </tr>
          )}
        </tbody>
      </table>

      <table className="nexus-table">
        <tbody>
          <tr>
            <td className="colhead" colSpan={4}>
              <h2 className="font-display">{t.myTitle}</h2>
            </td>
          </tr>
          <tr>
            <td className="colhead">{t.colName}</td>
            <td className="colhead">{t.colPeriod}</td>
            <td className="colhead">{t.colAmount}</td>
            <td className="colhead">{t.colAt}</td>
          </tr>
          {claims.map((c) => (
            <tr key={c.id}>
              <td>{c.type_name}</td>
              <td className="num">{c.period}</td>
              <td className="num font-bold">+{c.amount.toLocaleString()}</td>
              <td className="text-xs text-sub">
                {new Date(c.claimed_at).toLocaleString(dateLocale(locale))}
              </td>
            </tr>
          ))}
          {claims.length === 0 && (
            <tr>
              <td colSpan={4} className="py-6 text-center text-sub">
                {t.myEmpty}
              </td>
            </tr>
          )}
        </tbody>
      </table>

      <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-xs text-ink">{t.note}</p>
    </div>
  );
}
