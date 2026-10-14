"use client";

/**
 * 促销 intent 审批队列（0356）：发布者提交的促销申请 pending →
 * 管理组通过（写 promotions + sha3-256 快照留痕）/ 拒绝（带理由）。
 * 与 FreeleechPanel（直挂现状）同域不同面板，独立文件承载 300 行门禁。
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import type { PromoIntentRow } from "./admin-shared";

const STATUS_KEYS = [
  "pending",
  "approved",
  "rejected",
  "cancelled",
  "expired",
] as const;

/** 促销申请队列：GET /staff/promo-intents + approve / reject */
export function PromoIntentsPanel() {
  const { dict, locale } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  const t = dict.adminPromoIntents;
  const [rows, setRows] = useState<PromoIntentRow[]>([]);
  const [status, setStatus] = useState<string>("pending");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [notes, setNotes] = useState<Record<number, string>>({});

  const load = useCallback(async () => {
    try {
      setRows(
        await api.get<PromoIntentRow[]>(
          `/api/v1/staff/promo-intents?status=${status}`,
        ),
      );
    } catch {
      setRows([]); setMsg(dict.common.loadFailed);
    }
  }, [status, dict]);

  useEffect(() => {
    load();
  }, [load]);

  async function approve(id: number) {
    setBusy(true);
    try {
      await api.post(`/api/v1/staff/promo-intents/${id}/approve`, {});
      setMsg(t.approved);
      load();
    } catch (e) {
      setMsg(
        e instanceof ApiError
          ? (apiErrorMessage(dict, e))
          : a.actionFailed,
      );
    } finally {
      setBusy(false);
    }
  }

  async function reject(id: number) {
    const note = notes[id]?.trim();
    if (!note) {
      setMsg(t.needNote);
      return;
    }
    setBusy(true);
    try {
      await api.post(`/api/v1/staff/promo-intents/${id}/reject`, { note });
      setMsg(t.rejected);
      load();
    } catch (e) {
      setMsg(
        e instanceof ApiError
          ? (apiErrorMessage(dict, e))
          : a.actionFailed,
      );
    } finally {
      setBusy(false);
    }
  }

  const scopeText = (r: PromoIntentRow) => {
    const s = r.scope === "torrent" ? t.scopesTorrent : t.scopes[r.scope];
    if (r.scope === "torrent" && r.torrent_name) {
      return `${s} #${r.torrent_id} · ${r.torrent_name}`;
    }
    if (r.scope === "category" && r.category_name) {
      return `${s} · ${r.category_name}`;
    }
    return s ?? r.scope;
  };

  return (
    <section className="brand-panel p-4">
      <h2 className="mb-2 text-base font-bold">{t.title}</h2>
      <p className="mb-3 text-xs text-sub">{t.intro}</p>
      {msg && (
        <p className="mb-3 rounded-[var(--r-md)] bg-sky-soft p-2 text-xs">
          {msg}
        </p>
      )}
      <div className="mb-3 flex flex-wrap gap-2">
        {STATUS_KEYS.map((s) => (
          <button
            key={s}
            onClick={() => setStatus(s)}
            className={`min-h-[28px] rounded-full border px-3 text-xs
              font-bold ${
                status === s
                  ? "border-sky bg-sky-soft text-link"
                  : "border-line text-sub"
              }`}
          >
            {t.statuses[s]}
          </button>
        ))}
      </div>
      <div className="brand-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead w-24">{t.colKind}</td>
              <td className="colhead">{t.colScope}</td>
              <td className="colhead">{t.colRequester}</td>
              <td className="colhead">{t.colStart}</td>
              <td className="colhead">{t.colEnd}</td>
              <td className="colhead w-20">{t.colStatus}</td>
              <td className="colhead">{t.colActions}</td>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.id}>
                <td className="rowfollow">
                  {dict.adminPromo.kinds[r.kind] ?? r.kind}
                </td>
                <td className="rowfollow">{scopeText(r)}</td>
                <td className="rowfollow">
                  {r.requested_by_name ?? r.requested_by}
                </td>
                <td className="rowfollow text-xs text-sub">
                  {new Date(r.starts_at).toLocaleString(dateLocale(locale))}
                </td>
                <td className="rowfollow text-xs text-sub">
                  {new Date(r.ends_at).toLocaleString(dateLocale(locale))}
                </td>
                <td className="rowfollow">
                  {t.statuses[r.status as keyof typeof t.statuses] ?? r.status}
                  {r.status === "rejected" && r.decide_note
                    ? ` · ${r.decide_note}`
                    : ""}
                </td>
                <td className="rowfollow">
                  {r.status === "pending" ? (
                    <div className="flex flex-col gap-1">
                      <div className="flex gap-1">
                        <button
                          disabled={busy}
                          onClick={() => approve(r.id)}
                          className="min-h-[28px] rounded-full border
                            border-line px-3 font-bold text-link
                            disabled:opacity-50"
                        >
                          {t.approveBtn}
                        </button>
                        <button
                          disabled={busy}
                          onClick={() => reject(r.id)}
                          className="min-h-[28px] rounded-full border
                            border-line px-3 font-bold text-danger
                            disabled:opacity-50"
                        >
                          {t.rejectBtn}
                        </button>
                      </div>
                      <input
                        value={notes[r.id] ?? ""}
                        onChange={(e) =>
                          setNotes((m) => ({ ...m, [r.id]: e.target.value }))
                        }
                        placeholder={t.notePh}
                        className="min-h-[28px] rounded-[var(--r-md)] border
                          border-line bg-cloud px-2 text-xs"
                      />
                    </div>
                  ) : (
                    fmt(t.decidedAt, {
                      t: r.decided_at
                        ? new Date(r.decided_at).toLocaleString(
                            dateLocale(locale),
                          )
                        : "",
                    })
                  )}
                </td>
              </tr>
            ))}
            {rows.length === 0 && (
              <tr>
                <td colSpan={7} className="py-4 text-center text-sub">
                  {t.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}
