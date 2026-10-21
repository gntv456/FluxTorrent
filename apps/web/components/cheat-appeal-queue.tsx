"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";

/** 申诉行（GET /admin/cheat-appeals，0369 M3） */
interface AppealRow {
  id: number;
  user_id: number;
  username: string;
  case_id: number | null;
  body: string;
  status: string;
  outcome: string | null;
  result_note: string | null;
  by_token: boolean;
  created_at: string;
  sanction: Record<string, unknown> | null;
}

const OUTCOMES = ["upheld", "mitigated", "overturned"] as const;

/**
 * 反作弊申诉队列（0369 M3，staff 侧）。复核结论三选一：维持 / 减轻 /
 * 推翻；推翻由后端反向解除该案处置（停权/封禁/警告/回收流量）并把
 * 本案证据标 false_positive（准确率统计）。四眼：决定人不得复核。
 */
export function CheatAppealQueue() {
  const { dict } = useI18n();
  const t = dict.cheaterbox;
  const [rows, setRows] = useState<AppealRow[] | null>(null);
  const [onlyOpen, setOnlyOpen] = useState(true);
  const [reviewId, setReviewId] = useState<number | null>(null);
  const [outcome, setOutcome] = useState<string>("upheld");
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [noPerm, setNoPerm] = useState(false);

  const load = useCallback(() => {
    setRows(null);
    setErr(null);
    api
      .get<AppealRow[]>("/api/v1/admin/cheat-appeals")
      .then(setRows)
      .catch((e) => {
        // 信封码 2003 = 无权限（403 是 HTTP 状态，P-501）。
        if (e instanceof ApiError && e.code === 2003) setNoPerm(true);
        else setErr(apiErrorMessage(dict, e));
        setRows([]);
      });
  }, [dict]);

  useEffect(() => {
    load();
  }, [load]);

  const items = (rows ?? []).filter((r) => !onlyOpen || r.status === "open");

  const outcomeLabel = (o: string) =>
    o === "overturned"
      ? t.outcomeOverturned
      : o === "mitigated"
        ? t.outcomeMitigated
        : t.outcomeUpheld;

  async function submit(id: number) {
    setBusy(true);
    setErr(null);
    setMsg(null);
    try {
      await api.post(`/api/v1/admin/cheat-appeals/${id}/resolve`, {
        outcome,
        note: note.trim(),
      });
      setMsg(t.appealDone);
      setReviewId(null);
      setNote("");
      setOutcome("upheld");
      load();
    } catch (e) {
      setErr(
        e instanceof ApiError
          ? apiErrorMessage(dict, e)
          : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  const th = "px-3 py-2 text-left text-xs font-bold whitespace-nowrap";
  const td = "px-3 py-2 text-sm align-middle";
  const inputCls =
    "min-h-[36px] rounded-[var(--r-sm)] border border-line bg-cloud px-2 " +
    "text-sm outline-none focus:border-sky";

  if (noPerm) {
    return (
      <section className="brand-panel mt-4">
        <header className="brand-panel__head">
          <h2>⚖️ {t.appealTitle}</h2>
        </header>
        <p className="funbox__empty">{t.appealNoPerm}</p>
      </section>
    );
  }

  return (
    <section className="brand-panel mt-4">
      <header className="brand-panel__head">
        <h2>
          <span aria-hidden="true">⚖️</span> {t.appealTitle}
        </h2>
        {rows !== null && <small>{items.length}</small>}
      </header>
      <p className="mb-2 text-xs text-sub">{t.appealNote}</p>

      {msg && (
        <p role="status" className="mb-2 text-sm text-mint">
          {msg}
        </p>
      )}
      {err && (
        <p role="alert" className="mb-2 text-sm text-danger">
          {err}
        </p>
      )}

      <label className="mb-2 flex items-center gap-2 text-xs text-sub">
        <input
          type="checkbox"
          checked={onlyOpen}
          onChange={(e) => setOnlyOpen(e.target.checked)}
        />
        {t.appealFilterOpen}
      </label>

      {rows === null ? (
        <p className="funbox__empty">…</p>
      ) : err === null && items.length === 0 ? (
        <p className="funbox__empty">{t.appealEmpty}</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full border-collapse">
            <thead>
              <tr className="border-b border-[var(--border-soft)]">
                <th className={th}>{t.colAppealUser}</th>
                <th className={th}>{t.colAppealCase}</th>
                <th className={th}>{t.colAppealKind}</th>
                <th className={th}>{t.colAppealBody}</th>
                <th className={th}>{t.colAppealStatus}</th>
                <th className={th}>{t.colAppealTime}</th>
                <th className={th} aria-hidden="true" />
              </tr>
            </thead>
            <tbody>
              {items.map((r) => (
                <tr
                  key={r.id}
                  className={
                    "border-b border-dashed border-[var(--border-soft)]"
                  }
                >
                  <td className={td}>
                    <a
                      className={
                        "font-bold text-[var(--brand-orange-dark)] " +
                        "hover:underline"
                      }
                      href={`/users/${r.user_id}`}
                    >
                      {r.username}
                    </a>
                  </td>
                  <td className={`${td} whitespace-nowrap`}>
                    {r.case_id ? `#${r.case_id}` : "—"}
                  </td>
                  <td className={`${td} whitespace-nowrap text-xs`}>
                    {r.by_token ? t.appealByToken : t.appealByLogin}
                  </td>
                  <td className={`${td} max-w-[320px]`}>
                    <span className="line-clamp-2 whitespace-pre-wrap">
                      {r.body}
                    </span>
                  </td>
                  <td className={`${td} whitespace-nowrap text-xs`}>
                    {r.status === "open"
                      ? t.appealStatusOpen
                      : r.outcome
                        ? outcomeLabel(r.outcome)
                        : t.appealStatusDone}
                  </td>
                  <td
                    className={
                      `${td} whitespace-nowrap text-xs ` +
                      "text-[var(--text-faint)]"
                    }
                  >
                    {new Date(r.created_at).toLocaleString()}
                  </td>
                  <td className={`${td} whitespace-nowrap`}>
                    {r.status === "open" ? (
                      <button
                        type="button"
                        className="text-xs font-bold text-sky"
                        onClick={() => {
                          setReviewId(reviewId === r.id ? null : r.id);
                          setMsg(null);
                          setErr(null);
                        }}
                      >
                        {t.appealReview}
                      </button>
                    ) : (
                      <span className="text-xs text-sub">—</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {reviewId !== null && (
        <div
          className={
            "mt-3 flex flex-col gap-2 rounded-[var(--r-md)] " +
            "border border-line bg-[var(--bg-soft)] p-3"
          }
        >
          <p className="text-sm font-bold">
            {t.appealReview} #{reviewId}
          </p>
          <div className="flex flex-wrap gap-3 text-sm">
            {OUTCOMES.map((o) => (
              <label key={o} className="flex items-center gap-1">
                <input
                  type="radio"
                  name={`outcome-${reviewId}`}
                  checked={outcome === o}
                  onChange={() => setOutcome(o)}
                />
                {outcomeLabel(o)}
              </label>
            ))}
          </div>
          <textarea
            value={note}
            onChange={(e) => setNote(e.target.value)}
            rows={3}
            placeholder={t.appealNotePlaceholder}
            className={inputCls}
          />
          <div className="flex gap-2">
            <button
              type="button"
              disabled={busy || note.trim().length === 0}
              onClick={() => submit(reviewId)}
              className="rounded-[var(--r-sm)] bg-sky px-3 py-1.5 text-sm
                font-bold text-white disabled:opacity-50"
            >
              {busy ? t.appealBusy : t.appealSubmit}
            </button>
            <button
              type="button"
              onClick={() => setReviewId(null)}
              className="rounded-[var(--r-sm)] border border-line px-3 py-1.5
                text-sm text-sub"
            >
              {t.collapse}
            </button>
          </div>
        </div>
      )}
    </section>
  );
}
