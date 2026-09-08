"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

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

const KIND_LABEL: Record<string, string> = {
  seeding_reward: "做种收益",
  attendance: "签到",
  forum: "论坛奖励",
  subtitle: "字幕奖励",
  shop: "商店消费",
  bank_deposit: "银行存款",
  bank_withdraw: "银行取款",
  pool_donate: "站免池捐赠",
  admin_grant: "运营发放",
  task_reward: "任务/考核奖励",
  vote: "投票",
};

/** 签到 + 我的火花总览（客户端叶子组件） */
export function CheckinCard() {
  const [spark, setSpark] = useState<SparkInfo | null>(null);
  const [att, setAtt] = useState<AttendanceInfo | null>(null);
  const [ledger, setLedger] = useState<LedgerRow[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

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
      setMsg(e instanceof ApiError && e.code === 2001 ? "请先登录" : "加载失败");
    }
  }

  useEffect(() => {
    refresh();
  }, []);

  async function checkin() {
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.post<{ streak: number; reward: number }>(
        "/api/v1/attendance/checkin",
        {},
      );
      setMsg(`签到成功！+${r.reward} 火花，已连签 ${r.streak} 天`);
      refresh();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : "网络异常");
    } finally {
      setBusy(false);
    }
  }

  const token =
    typeof window !== "undefined" && localStorage.getItem("flux.token");

  if (!token) {
    return (
      <div className="rounded-[var(--r-lg)] border border-line bg-white p-6 text-center shadow-[var(--shadow-card)]">
        <span aria-hidden className="text-[60px]">
          🦉
        </span>
        <p className="mt-2 text-sub">登录后查看你的火花与签到</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <section className="grid grid-cols-2 gap-3 sm:grid-cols-3">
        <div className="rounded-[var(--r-md)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
          <p className="text-xs text-sub">火花余额</p>
          <p className="num mt-1 text-xl">
            {(spark?.balance ?? 0).toLocaleString("zh-CN")}
          </p>
        </div>
        <div className="rounded-[var(--r-md)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
          <p className="text-xs text-sub">在做种</p>
          <p className="num mt-1 text-xl text-mint">{spark?.seeding_count ?? 0}</p>
        </div>
        <div className="col-span-2 rounded-[var(--r-md)] border border-line bg-white p-4 shadow-[var(--shadow-card)] sm:col-span-1">
          <p className="text-xs text-sub">预计时薪</p>
          <p className="num mt-1 text-xl text-sky">
            +{spark?.hourly_estimate ?? 0}
          </p>
        </div>
      </section>

      <section className="rounded-[var(--r-lg)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="font-display text-lg">每日签到</h2>
            {att && (
              <p className="text-xs text-sub">已连签 {att.streak} 天</p>
            )}
          </div>
          <button
            onClick={checkin}
            disabled={busy || att?.checked_today}
            className="min-h-[44px] rounded-full bg-sun px-6 font-bold text-ink active:scale-[0.97] disabled:opacity-50"
          >
            {att?.checked_today ? "今日已签 ✓" : busy ? "签到中…" : "签到 +火花"}
          </button>
        </div>
        {msg && <p className="mt-2 text-sm text-sub">{msg}</p>}
      </section>

      <section className="rounded-[var(--r-lg)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
        <h2 className="mb-2 font-display text-lg">火花流水</h2>
        <ul className="flex flex-col divide-y divide-line text-sm">
          {ledger.map((row, i) => (
            <li key={i} className="flex items-center justify-between py-2">
              <div>
                <p>{KIND_LABEL[row.kind] ?? row.kind}</p>
                <p className="text-[11px] text-sub">
                  {new Date(row.created_at).toLocaleString("zh-CN")}
                </p>
              </div>
              <span
                className={`num font-bold ${row.amount >= 0 ? "text-mint" : "text-coral"}`}
              >
                {row.amount >= 0 ? "+" : ""}
                {row.amount.toLocaleString("zh-CN")}
              </span>
            </li>
          ))}
          {ledger.length === 0 && (
            <li className="py-4 text-center text-sub">暂无流水</li>
          )}
        </ul>
      </section>
    </div>
  );
}
