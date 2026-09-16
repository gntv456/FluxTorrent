"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

interface MetricCheck {
  key: string;
  required: number;
  current: number;
  ok: boolean;
}

interface AssignedPost {
  type_id: number;
  name: string;
  base_pay: number;
  checks: MetricCheck[];
  all_ok: boolean;
  qualified_months: number;
  bonus: number;
  total: number;
  claimed: boolean;
}

interface JixiaoMe {
  period: string;
  metrics: Record<string, number>;
  assigned: AssignedPost[];
}

interface MyClaim {
  id: number;
  type_name: string;
  period: string;
  amount: number;
  claimed_at: string;
}

/** 绩效考核（NP jixiao.php 口径）：我的岗位卡片（实时指标 vs 要求 + 达标状态）
 *  + 指标自动采集说明 + 领取记录。未被分配岗位时给引导文案。 */
export default function JixiaoPage() {
  const { dict, locale, currency } = useI18n();
  const t = dict.jixiao2;
  const [me, setMe] = useState<JixiaoMe | null>(null);
  const [claims, setClaims] = useState<MyClaim[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<number | null>(null);

  const load = useCallback(() => {
    api
      .get<JixiaoMe>("/api/v1/jixiao/me")
      .then(setMe)
      .catch(() => setMe(null));
    api.get<MyClaim[]>("/api/v1/jixiao/my").then(setClaims).catch(() => setClaims([]));
  }, []);
  useEffect(load, [load]);

  function flash(m: string) {
    setMsg(m);
    setErr(null);
    setTimeout(() => setMsg(null), 3500);
  }
  function flashErr(m: string) {
    setErr(m);
    setMsg(null);
    setTimeout(() => setErr(null), 5000);
  }

  const reqLabel: Record<string, string> = {
    uploaded: t.mUploaded,
    downloaded: t.mDownloaded,
    uploads: t.mUploads,
    seeding_count: t.mSeeding,
    seed_size: t.mSeedSize,
    ops: t.mOps,
    seed_days: t.mSeedDays,
    seed_hours: t.mSeedHours,
    seed_size_tb: t.mSeedSize,
  };
  const fmtVal = (k: string, v: number) =>
    k === "uploaded" || k === "downloaded" || k === "seed_size"
      ? fmtBytes(v)
      : k === "seed_size_tb"
        ? `${v.toLocaleString()} TB`
        : v.toLocaleString();

  async function claim(p: AssignedPost) {
    setBusyId(p.type_id);
    try {
      const r = await api.post<{ total: number }>("/api/v1/jixiao/claim", {
        type_id: p.type_id,
      });
      flash(t.claimed.replace("{n}", r.total.toLocaleString()));
      load();
    } catch (e) {
      flashErr(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusyId(null);
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{t.title}</h1>
        <span className="text-sm text-sub">{t.subtitle}</span>
      </div>
      {msg && (
        <p className="rounded-[var(--r-md)] bg-mint/30 p-3 text-sm text-ink">{msg}</p>
      )}
      {err && <p className="rounded-[var(--r-md)] bg-sun/30 p-3 text-sm text-ink">{err}</p>}

      {/* 我的岗位卡片（NP「当前考核信息 + 考核数据统计」合并视图） */}
      {me && me.assigned.length > 0 ? (
        <div className="grid gap-3 md:grid-cols-2">
          {me.assigned.map((p) => (
            <article
              key={p.type_id}
              className="rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]"
            >
              <header className="flex flex-wrap items-center justify-between gap-2">
                <h2 className="font-display text-lg">{p.name}</h2>
                <span className={`sticker ${p.all_ok ? "bg-mint/40" : "bg-sun/50"}`}>
                  {p.all_ok ? t.stQualified : t.stNotQualified}
                </span>
              </header>
              <dl className="mt-3 flex flex-col gap-2">
                <div className="flex justify-between">
                  <dt className="text-xs text-sub">{t.colBasePay}</dt>
                  <dd className="num text-sm font-bold">
                    {currency} {p.base_pay.toLocaleString()}
                  </dd>
                </div>
                {p.checks.map((c) => (
                  <div key={c.key} className="flex items-center justify-between gap-2">
                    <dt className="text-xs text-sub">{reqLabel[c.key] ?? c.key}</dt>
                    <dd className={`num text-sm ${c.ok ? "text-emerald-600" : "text-sub"}`}>
                      {fmtVal(c.key, c.current)} / {fmtVal(c.key, c.required)}
                      <span className="ml-1">{c.ok ? "✓" : "✗"}</span>
                    </dd>
                  </div>
                ))}
                {p.qualified_months > 0 && (
                  <div className="flex justify-between">
                    <dt className="text-xs text-sub">{t.qualifiedMonths}</dt>
                    <dd className="num text-sm">
                      {p.qualified_months}（+{currency} {p.bonus.toLocaleString()}）
                    </dd>
                  </div>
                )}
                <div className="flex justify-between border-t border-line pt-2">
                  <dt className="text-xs text-sub">{t.stTotal}</dt>
                  <dd className="num text-sm font-bold">
                    {currency} {p.total.toLocaleString()}
                  </dd>
                </div>
              </dl>
              <button
                className="cmgmt-act cmgmt-act--ok mt-3 w-full"
                disabled={p.claimed || !p.all_ok || busyId === p.type_id}
                onClick={() => claim(p)}
              >
                {p.claimed ? t.stClaimed : p.all_ok ? t.btnClaim : t.stNotQualified}
              </button>
            </article>
          ))}
        </div>
      ) : (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-4 text-sm text-ink">
          {t.noAssignment}
        </p>
      )}

      {/* 领取记录 */}
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

function fmtBytes(n: number): string {
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v >= 100 ? v.toFixed(0) : v.toFixed(2)} ${units[i]}`;
}
