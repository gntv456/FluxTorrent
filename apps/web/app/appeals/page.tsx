"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

interface AppealRow {
  id: number;
  ref_id: number | null;
  kind: string;
  body: string;
  status: string;
  result_note: string | null;
  created_at: string;
}

/** 申诉通道（NexusPHP complains.php 口径）：提交申诉 + 我的申诉记录。需登录（middleware 保证）。 */
export default function AppealsPage() {
  const { dict, locale } = useI18n();
  const [rows, setRows] = useState<AppealRow[] | null>(null);
  const [kind, setKind] = useState("ban");
  const [body, setBody] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [okMsg, setOkMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function load() {
    setRows(null);
    try {
      setRows(await api.get<AppealRow[]>("/api/v1/me/appeals"));
    } catch (e) {
      setMsg(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : dict.common.loadFailed);
    }
  }
  useEffect(() => {
    load();
  }, []);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setMsg(null);
    setOkMsg(null);
    try {
      await api.post("/api/v1/appeals", { kind, body: body.trim() });
      setOkMsg(dict.appeals.submitOk);
      setBody("");
      load();
    } catch (e2) {
      setMsg(
        e2 instanceof ApiError ? (dict.errors[e2.code] ?? e2.message) : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  const inputCls =
    "min-h-[44px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky";
  const kinds: [string, string][] = [
    ["ban", dict.appeals.kindBan],
    ["hr", dict.appeals.kindHr],
    ["warn", dict.appeals.kindWarn],
    ["other", dict.appeals.kindOther],
  ];

  return (
    <div className="mx-auto flex max-w-lg flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.appeals.title}</h1>
      <p className="-mt-2 text-sm text-sub">{dict.appeals.subtitle}</p>

      <form
        onSubmit={submit}
        className="flex flex-col gap-3 rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]"
      >
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.appeals.kind}</span>
          <select
            value={kind}
            onChange={(e) => setKind(e.target.value)}
            className={inputCls}
          >
            {kinds.map(([k, label]) => (
              <option key={k} value={k}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.appeals.body}</span>
          <textarea
            value={body}
            onChange={(e) => setBody(e.target.value)}
            required
            minLength={10}
            maxLength={2000}
            rows={5}
            placeholder={dict.appeals.bodyPlaceholder}
            className="min-h-[100px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 py-2 text-sm outline-none focus:border-sky"
          />
        </label>
        {msg && (
          <p role="alert" className="text-sm text-danger">
            {msg}
          </p>
        )}
        {okMsg && (
          <p role="status" className="text-sm text-mint">
            {okMsg}
          </p>
        )}
        <button
          type="submit"
          disabled={busy}
          className="min-h-[44px] rounded-full bg-sky font-bold text-white active:scale-[0.97] disabled:opacity-50"
        >
          {busy ? dict.appeals.busy : dict.appeals.submit}
        </button>
      </form>

      <section className="flex flex-col gap-2 rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
        <h2 className="font-display text-lg">{dict.appeals.myRecords}</h2>
        {rows === null && <p className="py-2 text-sm text-sub">{dict.appeals.loading}</p>}
        {rows?.length === 0 && (
          <p className="py-2 text-sm text-sub">{dict.appeals.empty}</p>
        )}
        {rows && rows.length > 0 && (
          <ul className="flex flex-col divide-y divide-line">
            {rows.map((r) => (
              <li key={r.id} className="py-2.5">
                <div className="flex items-center justify-between gap-2">
                  <span className="text-sm font-bold">
                    {kinds.find(([k]) => k === r.kind)?.[1] ?? r.kind}
                  </span>
                  <span
                    className={`sticker ${
                      r.status === "open" ? "bg-sun text-ink" : "bg-mint text-white"
                    }`}
                  >
                    {r.status === "open" ? dict.appeals.statusOpen : dict.appeals.statusDone}
                  </span>
                </div>
                <p className="mt-1 text-sm whitespace-pre-wrap">{r.body}</p>
                {r.result_note && (
                  <p className="mt-1 text-sm text-sub">
                    {dict.appeals.resultNote}: {r.result_note}
                  </p>
                )}
                <p className="mt-1 text-[11px] text-sub">
                  {new Date(r.created_at).toLocaleString(dateLocale(locale))}
                </p>
              </li>
            ))}
          </ul>
        )}
      </section>

      <p className="text-sm text-sub">
        <Link href="/login" className="font-bold text-sky">
          {dict.appeals.backToLogin}
        </Link>
      </p>
    </div>
  );
}
