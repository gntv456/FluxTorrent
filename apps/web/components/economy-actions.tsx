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
    <div className="flex flex-col gap-4">
      {/* 火花总览（rowhead/rowfollow 经典表格） */}
      <section className="nexus-detail">
        <table className="nexus-table nexus-form">
          <thead>
            <tr>
              <td colSpan={2} className="colhead">
                {dict.common.spark}
              </td>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td className="rowhead">{dict.my.balance}</td>
              <td className="rowfollow">
                <span className="num">
                  {(spark?.balance ?? 0).toLocaleString(dateLocale(locale))}
                </span>
              </td>
            </tr>
            <tr>
              <td className="rowhead">{dict.my.seedingCount}</td>
              <td className="rowfollow">
                <span className="num text-mint">{spark?.seeding_count ?? 0}</span>
              </td>
            </tr>
            <tr>
              <td className="rowhead">{dict.my.hourly}</td>
              <td className="rowfollow">
                <span className="num text-sky">+{spark?.hourly_estimate ?? 0}</span>
              </td>
            </tr>
          </tbody>
        </table>
      </section>

      {/* 签到 / 登出 */}
      <section className="nexus-detail">
        <table className="nexus-table nexus-form">
          <thead>
            <tr>
              <td colSpan={2} className="colhead">
                {dict.my.dailyCheckin}
              </td>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td className="rowhead">{dict.my.streak.replace("{n}", String(att?.streak ?? 0))}</td>
              <td className="rowfollow">
                <div className="flex flex-wrap items-center gap-3">
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
                  <button
                    onClick={logout}
                    className="min-h-[44px] rounded-full border border-line px-5 text-sm text-sub transition-colors hover:text-coral"
                  >
                    {dict.my.logout}
                  </button>
                </div>
                {msg && <p className="mt-2 text-sm text-sub">{msg}</p>}
              </td>
            </tr>
          </tbody>
        </table>
      </section>

      {/* 火花流水 */}
      <section className="nexus-detail">
        <table className="nexus-table">
          <thead>
            <tr>
              <td className="colhead">{dict.my.ledger}</td>
              <td className="colhead w-36 text-right">±{dict.common.spark}</td>
            </tr>
          </thead>
          <tbody>
            {ledger.map((row, i) => (
              <tr key={i}>
                <td>
                  <p>{dict.my.kinds[row.kind] ?? row.kind}</p>
                  <p className="text-[11px] text-sub">
                    {new Date(row.created_at).toLocaleString(dateLocale(locale))}
                  </p>
                </td>
                <td
                  className={`num w-36 text-right font-bold ${row.amount >= 0 ? "text-mint" : "text-coral"}`}
                >
                  {row.amount >= 0 ? "+" : ""}
                  {row.amount.toLocaleString(dateLocale(locale))}
                </td>
              </tr>
            ))}
            {ledger.length === 0 && (
              <tr>
                <td colSpan={2} className="py-4 text-center text-sub">
                  {dict.my.noLedger}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </section>
    </div>
  );
}
