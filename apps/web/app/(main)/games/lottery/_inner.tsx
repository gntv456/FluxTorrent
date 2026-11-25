"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { fmt, fmtCur } from "@/i18n/config";
import {
  BTN_MD_SKY,
  BTN_SM_GHOST,
  PANEL_MD,
} from "@/lib/ui-classes";
import {
  ClaimForm,
  claimFormVisible,
  prizeStatusText,
  statusKey,
} from "./detail";
import type { Detail, ListRow } from "./detail";
import { SkeletonBlock } from "@/components/ui/skeleton";

export default function LotteryPage() {
  const { dict, currency } = useI18n();
  const t = dict.lottery;
  const [rows, setRows] = useState<ListRow[] | null>(null);
  const [tab, setTab] = useState<"open" | "drawn">("open");
  const [code, setCode] = useState<string | null>(null);
  const [detail, setDetail] = useState<Detail | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [claimFields, setClaimFields] = useState("");

  const load = useCallback(async () => {
    try {
      const r = await api.get<{ rows: ListRow[] }>("/api/v1/games/lottery");
      setRows(r.rows);
    } catch (e) {
      setErr(apiErrorMessage(dict, e));
    }
  }, [dict]);

  useEffect(() => {
    load();
  }, [load]);

  const loadDetail = useCallback(async (c: string) => {
    setDetail(null);
    setClaimFields("");
    try {
      const r = await api.get<Detail>(`/api/v1/games/lottery/${c}`);
      setDetail(r);
    } catch (e) {
      setErr(apiErrorMessage(dict, e));
    }
  }, [dict]);

  useEffect(() => {
    if (code) loadDetail(code);
  }, [code, loadDetail]);

  async function join(c: string) {
    setBusy(true);
    setErr(null);
    try {
      await api.post(`/api/v1/games/lottery/${c}/join`, {
        clientUuid: crypto.randomUUID(),
      });
      await load();
      if (code) await loadDetail(code);
    } catch (e) {
      setErr(
        e instanceof ApiError ? apiErrorMessage(dict, e) : t.errorJoin
      );
    } finally {
      setBusy(false);
    }
  }

  async function submitClaim(winnerId: number) {
    setBusy(true);
    setErr(null);
    try {
      let fields: Record<string, unknown> = {};
      try {
        fields = claimFields.trim() ? JSON.parse(claimFields) : {};
      } catch {
        setErr(t.claimInvalidJson);
        return;
      }
      await api.post(`/api/v1/games/lottery/claim/${winnerId}`, { fields });
      if (code) await loadDetail(code);
    } catch (e) {
      setErr(apiErrorMessage(dict, e));
    } finally {
      setBusy(false);
    }
  }

  const shown = (rows ?? []).filter((r) =>
    tab === "open" ? r.status === "open" : r.status !== "open"
  );

  return (
    <div className="mx-auto w-full max-w-3xl p-4">
      <h1 className="mb-1 text-xl font-bold">{t.title}</h1>
      <p className="mb-4 text-sm opacity-60">{t.sub}</p>
      {err && (
        <div
          className={
            "rounded-lg bg-rose-500/10 px-3 py-2 text-sm text-rose-500 mb-3"
          }
        >
          {err}
        </div>
      )}
      <div className="mb-3 flex gap-2">
        {(["open", "drawn"] as const).map((k) => (
          <button
            key={k}
            className={BTN_SM_GHOST}
            style={
              tab === k
                ? { fontWeight: 700, textDecoration: "underline" }
                : undefined
            }
            onClick={() => setTab(k)}
          >
            {k === "open" ? t.tabOpen : t.tabEnded}
          </button>
        ))}
      </div>
      {rows === null ? (
        <SkeletonBlock height={200} />
      ) : shown.length === 0 ? (
        <div className={PANEL_MD + " text-center text-sm opacity-60"}>
          {t.empty}
        </div>
      ) : (
        <div className="grid gap-3">
          {shown.map((r) => (
            <button
              key={r.code}
              className={PANEL_MD + " text-left"}
              onClick={() => setCode(r.code)}
            >
              <div className="flex items-center justify-between">
                <span className="font-bold">{r.title}</span>
                <span className="text-xs opacity-60">
                  {r.status === "open"
                    ? t.tabOpen
                    : r.voided
                      ? t.voided
                      : t.tabEnded}
                </span>
              </div>
              <div className="mt-1 text-xs opacity-70">
                {r.trigger === "scheduled" && r.drawAt
                  ? fmt(t.drawAt, {
                      t: r.drawAt.slice(0, 16).replace("T", " "),
                    })
                  : t.tabOpen}
                {r.ticketPrice > 0 &&
                  ` · ${fmtCur(t.ticketPrice, { n: r.ticketPrice }, currency)}`}
              </div>
              {r.joined && (
                <div className="mt-1 text-xs text-[var(--sky-text)]">
                  {t.joined}
                </div>
              )}
            </button>
          ))}
        </div>
      )}

      {detail && (
        <div className="fixed inset-0 z-50 flex items-center justify-center
          bg-black/40 p-4">
          <div
            className={PANEL_MD +
              " max-h-[80vh] w-full max-w-lg overflow-auto"}
          >
            <div className="flex items-center justify-between">
              <h2 className="text-lg font-bold">{detail.title}</h2>
              <button className={BTN_SM_GHOST} onClick={() => setCode(null)}>
                ×
              </button>
            </div>
            {detail.intro && (
              <p className="mt-1 text-sm opacity-70">{detail.intro}</p>
            )}
            <div className="mt-2 text-sm">
              {fmt(t.participants, { n: detail.participants })}
              {detail.ticketPrice > 0 &&
                ` · ${
                  fmtCur(t.ticketPrice, { n: detail.ticketPrice }, currency)
                }`}
            </div>
            <div className="mt-1 text-xs opacity-70">
              {detail.trigger === "scheduled" && detail.drawAt
                ? fmt(t.drawAt, {
                    t: detail.drawAt.slice(0, 16).replace("T", " "),
                  })
                : detail.threshold
                  ? fmt(t.threshold, { n: detail.threshold })
                  : ""}
              {detail.fallbackAt &&
                ` · ${fmt(t.fallback, {
                  t: detail.fallbackAt.slice(0, 16).replace("T", " "),
                })}`}
            </div>

            <h3 className="mt-3 text-sm font-bold">{t.prizes}</h3>
            <ul className="mt-1 text-sm">
              {detail.prizes.map((p) => (
                <li key={p.id} className="flex justify-between py-0.5">
                  <span>{p.name}</span>
                  <span className="opacity-60">
                    {fmt(t.prizeQty, { n: p.quantity })}
                  </span>
                </li>
              ))}
            </ul>

            {detail.status === "open" && (
              <button
                className={BTN_MD_SKY + " mt-3 w-full"}
                disabled={busy || detail.joined}
                onClick={() => join(detail.code)}
              >
                {detail.joined
                  ? t.joined
                  : detail.ticketPrice > 0
                    ? fmtCur(t.ticketPrice, { n: detail.ticketPrice }, currency)
                    : t.joinFree}
              </button>
            )}

            {detail.myPrize && (
              <div className="mt-3 rounded-lg bg-[var(--sky-soft)] p-3 text-sm">
                <div className="font-bold">
                  {fmt(t.won, { prize: detail.myPrize.prizeName })}
                </div>
                <div className="mt-1 opacity-70">
                  {prizeStatusText(
                    detail.myPrize,
                    t as Record<string, string>
                  )}
                </div>
              </div>
            )}
            {detail.status === "drawn" &&
              !detail.myPrize &&
              detail.winners.length > 0 && (
                <div className="mt-2 text-sm opacity-60">{t.notWon}</div>
              )}

            {detail.winners.length > 0 && (
              <>
                <h3 className="mt-3 text-sm font-bold">{t.winners}</h3>
                <ul className="mt-1 text-sm">
                  {detail.winners.map((w, i) => (
                    <li key={i} className="flex justify-between py-0.5">
                      <span>
                        {w.username} · {w.prizeName}
                      </span>
                      <span className="opacity-60">
                        {(t as Record<string, string>)[
                          statusKey(w.status)
                        ]}
                      </span>
                    </li>
                  ))}
                </ul>
              </>
            )}

            {claimFormVisible(detail.myPrize) && (
              <ClaimForm
                t={t}
                busy={busy}
                value={claimFields}
                onChange={setClaimFields}
                onSubmit={() => submitClaim(detail.myPrize!.winnerId)}
                submission={detail.myPrize.submission}
              />
            )}
          </div>
        </div>
      )}
    </div>
  );
}
