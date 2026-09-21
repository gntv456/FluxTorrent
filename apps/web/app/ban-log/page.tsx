"use client";

import { useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

interface BanLog {
  status: number;
  changed_at: string | null;
}

/** 封禁记录查询（NexusPHP user-ban-log.php 口径，公开）：按用户名查当前状态与最近变更时间 */
export default function BanLogPage() {
  const { dict, locale } = useI18n();
  const [username, setUsername] = useState("");
  const [result, setResult] = useState<BanLog | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setMsg(null);
    setResult(null);
    try {
      setResult(
        await api.get<BanLog>(
          `/api/v1/auth/ban-log?username=${encodeURIComponent(username.trim())}`,
        ),
      );
    } catch (err) {
      setMsg(
        err instanceof ApiError
          ? (dict.errors[err.code] ?? err.message)
          : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  const statusLabel = (s: number) =>
    s === 2
      ? dict.banlog.banned
      : s === 1
        ? dict.banlog.muted
        : dict.banlog.normal;

  return (
    <div className="mx-auto flex max-w-sm flex-col items-center gap-6 py-12">
      <span aria-hidden className="text-[80px] leading-none">
        🦉
      </span>
      <h1 className="font-display text-3xl">{dict.banlog.title}</h1>
      <p className="-mt-4 text-sm text-sub">{dict.banlog.subtitle}</p>
      <form onSubmit={submit} className="flex w-full flex-col gap-3">
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.banlog.username}</span>
          <input
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            required
            maxLength={24}
            className="min-h-[44px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-3 outline-none focus:ring-2 focus:ring-sky/40"
          />
        </label>
        {msg && (
          <p role="alert" className="text-sm text-danger">
            {msg}
          </p>
        )}
        <button
          type="submit"
          disabled={busy}
          className="min-h-[44px] rounded-full bg-sky font-bold text-white active:scale-[0.97] disabled:opacity-50"
        >
          {busy ? dict.banlog.busy : dict.banlog.submit}
        </button>
      </form>
      {result && (
        <div
          role="status"
          className="w-full rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 text-sm shadow-[var(--shadow-card)]"
        >
          <p className="flex justify-between">
            <span className="text-sub">{dict.banlog.currentStatus}</span>
            <span
              className={`font-bold ${
                result.status === 2 ? "text-danger" : "text-mint"
              }`}
            >
              {statusLabel(result.status)}
            </span>
          </p>
          {result.changed_at && (
            <p className="mt-1 flex justify-between">
              <span className="text-sub">{dict.banlog.changedAt}</span>
              <span className="num">
                {new Date(result.changed_at).toLocaleString(dateLocale(locale))}
              </span>
            </p>
          )}
          {result.status === 2 && (
            <p className="mt-2 text-xs text-sub">
              {dict.banlog.appealHint}{" "}
              <Link href="/appeals" className="font-bold text-sky">
                {dict.login.appealLink}
              </Link>
            </p>
          )}
        </div>
      )}
      <p className="text-xs text-sub">{dict.banlog.note}</p>
      <p className="text-sm text-sub">
        <Link href="/login" className="font-bold text-sky">
          {dict.forgot.backToLogin}
        </Link>
      </p>
    </div>
  );
}
