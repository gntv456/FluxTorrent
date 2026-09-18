"use client";

import { useCallback, useEffect, useState } from "react";
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

interface Captcha {
  captcha_id: string;
  question: string;
}

/**
 * 申诉通道（NexusPHP complains.php 口径）：提交申诉 + 我的申诉记录。
 * 未登录（含被封禁用户）可凭「用户名 + 图形验证码」提交封禁申诉（P0 修复）；
 * 已登录用户提交任意类型申诉。
 */
export default function AppealsPage() {
  const { dict, locale } = useI18n();
  const [rows, setRows] = useState<AppealRow[] | null>(null);
  const [kind, setKind] = useState("ban");
  const [body, setBody] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [okMsg, setOkMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // 未登录（被封）申诉字段
  const [guest, setGuest] = useState(false);
  const [username, setUsername] = useState("");
  const [captcha, setCaptcha] = useState<Captcha | null>(null);
  const [captchaAnswer, setCaptchaAnswer] = useState("");

  const refreshCaptcha = useCallback(async () => {
    setCaptcha(null);
    try {
      setCaptcha(await api.get<Captcha>("/api/v1/auth/captcha"));
    } catch {
      /* 限流时保留 null，提交会被后端校验拦下 */
    }
  }, []);

  useEffect(() => {
    // 尝试拉取自己的申诉记录：被封/未登录会 401/403，转入游客封禁申诉模式
    (async () => {
      try {
        setRows(await api.get<AppealRow[]>("/api/v1/me/appeals"));
      } catch {
        setGuest(true);
        refreshCaptcha();
      }
    })();
  }, [refreshCaptcha]);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setMsg(null);
    setOkMsg(null);
    try {
      const payload: Record<string, unknown> = { kind, body: body.trim() };
      if (guest) {
        payload.username = username.trim();
        payload.captcha_id = captcha?.captcha_id ?? "";
        payload.captcha_answer = captchaAnswer ? Number(captchaAnswer) : 0;
      }
      await api.post("/api/v1/appeals", payload);
      setOkMsg(dict.appeals.submitOk);
      setBody("");
      setCaptchaAnswer("");
      if (guest) refreshCaptcha();
    } catch (e2) {
      setMsg(
        e2 instanceof ApiError ? (dict.errors[e2.code] ?? e2.message) : dict.common.networkError,
      );
      if (guest) refreshCaptcha();
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
        {guest ? (
          <p className="rounded-[var(--r-sm)] bg-cloud p-2 text-xs text-sub">
            {dict.appeals.guestHint}
          </p>
        ) : null}
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.appeals.kind}</span>
          <select
            value={kind}
            onChange={(e) => setKind(e.target.value)}
            className={inputCls}
            disabled={guest}
          >
            {kinds.map(([k, label]) => (
              <option key={k} value={k}>
                {label}
              </option>
            ))}
          </select>
          {guest ? (
            <span className="text-[11px] text-sub">{dict.appeals.guestKindLocked}</span>
          ) : null}
        </label>
        {guest ? (
          <label className="flex flex-col gap-1">
            <span className="text-sm text-sub">{dict.appeals.bannedUsername}</span>
            <input
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              required
              autoComplete="username"
              className={inputCls}
              placeholder={dict.appeals.bannedUsernamePlaceholder}
            />
          </label>
        ) : null}
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
        {guest ? (
          <label className="flex flex-col gap-1">
            <span className="text-sm text-sub">{dict.appeals.captcha}</span>
            <div className="flex items-center gap-2">
              <span className="min-w-24 rounded-[var(--r-sm)] border border-line bg-cloud px-3 py-2 text-sm font-mono">
                {captcha?.question ?? "…"}
              </span>
              <button
                type="button"
                onClick={refreshCaptcha}
                className="text-xs font-bold text-sky"
                aria-label={dict.appeals.captchaRefresh}
              >
                ↻
              </button>
              <input
                value={captchaAnswer}
                onChange={(e) => setCaptchaAnswer(e.target.value)}
                required
                inputMode="numeric"
                className={`${inputCls} flex-1`}
                placeholder={dict.appeals.captchaAnswer}
              />
            </div>
          </label>
        ) : null}
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

      {!guest ? (
        <section className="flex flex-col gap-2 rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
          <h2 className="font-display text-lg">{dict.appeals.myRecords}</h2>
          {rows === null && <p className="py-2 text-sm text-sub">{dict.appeals.loading}</p>}
          {rows?.length === 0 && <p className="py-2 text-sm text-sub">{dict.appeals.empty}</p>}
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
      ) : null}

      <p className="text-sm text-sub">
        <Link href="/login" className="font-bold text-sky">
          {dict.appeals.backToLogin}
        </Link>
      </p>
    </div>
  );
}
