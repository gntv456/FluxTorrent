"use client";

import { useEffect, useState } from "react";
import { api, setSessionCookie, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";

interface SparkInfo {
  balance: number;
  seeding_count: number;
  hourly_estimate: number;
}

interface AttendanceInfo {
  checked_today: boolean;
  streak: number;
}

interface LedgerRow {
  amount: number;
  kind: string;
  balance_after: number | null;
  created_at: string;
}

/** 签到 + 我的火花总览（客户端叶子组件） */
export function CheckinCard() {
  const { dict, locale } = useI18n();
  const [spark, setSpark] = useState<SparkInfo | null>(null);
  const [att, setAtt] = useState<AttendanceInfo | null>(null);
  const [ledger, setLedger] = useState<LedgerRow[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [hasToken, setHasToken] = useState(false);

  async function refresh() {
    try {
      const [s, a, l] = await Promise.all([
        api.get<SparkInfo>("/api/v1/me/spark"),
        api.get<AttendanceInfo>("/api/v1/attendance"),
        api.get<LedgerRow[]>("/api/v1/me/spark/ledger?limit=10"),
      ]);
      setSpark(s);
      setAtt(a);
      setLedger(l);
    } catch (e) {
      setMsg(
        e instanceof ApiError && e.code === 2001
          ? dict.errors[2001]
          : dict.common.loadFailed,
      );
    }
  }

  useEffect(() => {
    setHasToken(Boolean(localStorage.getItem("flux.token")));
    refresh();
  }, []);

  async function logout() {
    try {
      await api.post("/api/v1/auth/logout", {});
    } catch {
      // 后端撤销失败也照常清理本地凭证
    }
    localStorage.removeItem("flux.token");
    setSessionCookie(null);
    location.href = "/login";
  }

  async function checkin() {
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.post<{ streak: number; reward: number }>(
        "/api/v1/attendance/checkin",
        {},
      );
      setMsg(fmt(dict.my.checkinOk, { reward: r.reward, streak: r.streak }));
      refresh();
    } catch (e) {
      setMsg(
        e instanceof ApiError
          ? (dict.errors[e.code] ?? e.message)
          : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  if (!hasToken) {
    return (
      <div className="rounded-[var(--r-lg)] border border-line bg-white p-6 text-center shadow-[var(--shadow-card)]">
        <span aria-hidden className="text-[60px]">
          🦉
        </span>
        <p className="mt-2 text-sub">{dict.my.loginToView}</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <section className="grid grid-cols-2 gap-3 sm:grid-cols-3">
        <div className="rounded-[var(--r-md)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
          <p className="text-xs text-sub">{dict.my.balance}</p>
          <p className="num mt-1 text-xl">
            {(spark?.balance ?? 0).toLocaleString(dateLocale(locale))}
          </p>
        </div>
        <div className="rounded-[var(--r-md)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
          <p className="text-xs text-sub">{dict.my.seedingCount}</p>
          <p className="num mt-1 text-xl text-mint">{spark?.seeding_count ?? 0}</p>
        </div>
        <div className="col-span-2 rounded-[var(--r-md)] border border-line bg-white p-4 shadow-[var(--shadow-card)] sm:col-span-1">
          <p className="text-xs text-sub">{dict.my.hourly}</p>
          <p className="num mt-1 text-xl text-sky">
            +{spark?.hourly_estimate ?? 0}
          </p>
        </div>
      </section>

      <section className="rounded-[var(--r-lg)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="font-display text-lg">{dict.my.dailyCheckin}</h2>
            {att && (
              <p className="text-xs text-sub">
                {fmt(dict.my.streak, { n: att.streak })}
              </p>
            )}
          </div>
          <button
            onClick={checkin}
            disabled={busy || att?.checked_today}
            className="min-h-[44px] rounded-full bg-sun px-6 font-bold text-ink active:scale-[0.97] disabled:opacity-50"
          >
            {att?.checked_today
              ? dict.my.checked
              : busy
                ? dict.my.checkinBusy
                : dict.my.checkin}
          </button>
        </div>
        {msg && <p className="mt-2 text-sm text-sub">{msg}</p>}
        <div className="mt-2 flex justify-end">
          <button
            onClick={logout}
            className="min-h-[44px] rounded-full border border-line px-5 text-sm text-sub transition-colors hover:text-coral"
          >
            {dict.my.logout}
          </button>
        </div>
      </section>

      <section className="rounded-[var(--r-lg)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
        <h2 className="mb-2 font-display text-lg">{dict.my.ledger}</h2>
        <ul className="flex flex-col divide-y divide-line text-sm">
          {ledger.map((row, i) => (
            <li key={i} className="flex items-center justify-between py-2">
              <div>
                <p>{dict.my.kinds[row.kind] ?? row.kind}</p>
                <p className="text-[11px] text-sub">
                  {new Date(row.created_at).toLocaleString(dateLocale(locale))}
                </p>
              </div>
              <span
                className={`num font-bold ${row.amount >= 0 ? "text-mint" : "text-coral"}`}
              >
                {row.amount >= 0 ? "+" : ""}
                {row.amount.toLocaleString(dateLocale(locale))}
              </span>
            </li>
          ))}
          {ledger.length === 0 && (
            <li className="py-4 text-center text-sub">{dict.my.noLedger}</li>
          )}
        </ul>
      </section>
    </div>
  );
}
