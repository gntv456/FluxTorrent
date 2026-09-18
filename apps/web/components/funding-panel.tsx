"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";

/** 定向众筹免费（0078，HDBits Featured 口径）
 *  GET /fundings?status= 列表（0 进行中 / 1 已达成 / 2 已退款 / 3 已了结）
 *  POST /fundings {torrent_id, goal, hours, days} 发起（发布者或 staff）
 *  POST /fundings/contribute {funding_id, amount, idempotency_key} 参与
 *  GET /fundings/mine 我的参与（元组：[funding_id, amount, tax, status, funding_id2]） */

const ALL_LABEL: Record<string, string> = {
  "zh-CN": "全部",
  "zh-TW": "全部",
  en: "All",
};

interface FundingRow {
  id: number;
  torrent_id: number;
  torrent_name: string | null;
  goal: number;
  raised: number;
  backers: number;
  hours: number;
  status: number;
  ends_at: string;
}

type MineRow = [number, number, number, number, number]; // (f.id, c.amount, c.tax, f.status, c.funding_id)

export function FundingPanel() {
  const { dict, locale, currency } = useI18n();
  const t = dict.funding;
  const [statusFilter, setStatusFilter] = useState<string>(""); // "" = 全部
  const [rows, setRows] = useState<FundingRow[] | null>(null);
  const [mine, setMine] = useState<MineRow[] | null>(null);
  const [mineOpen, setMineOpen] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [form, setForm] = useState({ torrent_id: "", goal: "1000", hours: "168", days: "14" });
  const [amounts, setAmounts] = useState<Record<number, string>>({});
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const qs = statusFilter ? `?status=${statusFilter}` : "";
      setRows(await api.get<FundingRow[]>(`/api/v1/fundings${qs}`));
    } catch {
      setRows([]);
    }
  }, [statusFilter]);
  useEffect(() => {
    load();
  }, [load]);

  const loadMine = useCallback(async () => {
    try {
      setMine(await api.get<MineRow[]>("/api/v1/fundings/mine"));
    } catch {
      setMine([]);
    }
  }, []);

  function flash(m: string) {
    setMsg(m);
    setTimeout(() => setMsg(null), 4000);
  }

  async function create() {
    setBusy(true);
    try {
      const r = await api.post<{ id: number }>("/api/v1/fundings", {
        torrent_id: Number(form.torrent_id),
        goal: Number(form.goal),
        hours: Number(form.hours),
        days: Number(form.days),
      });
      flash(t.created.replace("{id}", String(r.id)));
      setCreateOpen(false);
      load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function contribute(f: FundingRow) {
    const amount = Number(amounts[f.id]);
    if (!amount || amount <= 0) return;
    if (!window.confirm(`${t.contributeBtn} #${f.id} · ${amount} ${currency}？`)) return;
    setBusy(true);
    try {
      const r = await api.post<{ paid: number; raised: number; goal: number }>(
        "/api/v1/fundings/contribute",
        {
          funding_id: f.id,
          amount,
          idempotency_key: `web-${f.id}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`,
        },
      );
      flash(
        t.contributed
          .replace("{paid}", String(r.paid))
          .replace("{raised}", String(r.raised))
          .replace("{goal}", String(r.goal)),
      );
      setAmounts((a) => ({ ...a, [f.id]: "" }));
      load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  const statusLabel = (s: number) =>
    s === 0 ? t.stActive : s === 1 ? t.stReached : s === 2 ? t.stRefunded : s === 3 ? t.stSettled : t.stOther;

  const inputCls =
    "min-h-[38px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky";

  return (
    <section className="baozi-panel flex flex-col gap-3 p-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="text-base font-bold text-ink">🎁 {t.title}</h2>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
            onClick={() => {
              setMineOpen((v) => !v);
              if (!mineOpen) void loadMine();
            }}
          >
            {t.mine}
          </button>
          <button
            type="button"
            className="baozi-button min-h-[36px] text-xs"
            onClick={() => setCreateOpen((v) => !v)}
          >
            + {t.createBtn}
          </button>
        </div>
      </div>
      <p className="text-xs text-sub">{t.note.replaceAll("{magic}", currency)}</p>

      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-2 text-xs text-ink" role="status">
          {msg}
        </p>
      )}

      {/* 状态过滤 */}
      <div className="flex flex-wrap gap-1">
        {(["", "0", "1", "2", "3"] as const).map((s) => (
          <button
            key={s}
            type="button"
            onClick={() => setStatusFilter(s)}
            className={`min-h-[32px] rounded-full px-3 text-xs font-bold ${
              statusFilter === s
                ? "bg-sky text-white"
                : "border border-line text-sub"
            }`}
          >
            {s === "" ? (ALL_LABEL[locale] ?? "全部") : statusLabel(Number(s))}
          </button>
        ))}
      </div>

      {/* 发起表单 */}
      {createOpen && (
        <form
          className="flex flex-col gap-2 rounded-[var(--r-md)] border border-dashed border-line p-3"
          onSubmit={(e) => {
            e.preventDefault();
            void create();
          }}
        >
          <p className="text-xs font-bold text-sub">{t.createTitle}</p>
          <div className="flex flex-wrap items-end gap-2">
            <label className="flex flex-col gap-1 text-xs">
              {t.fldTorrent}
              <input
                type="number"
                min={1}
                required
                value={form.torrent_id}
                onChange={(e) => setForm({ ...form, torrent_id: e.target.value })}
                className={`${inputCls} w-28`}
              />
            </label>
            <label className="flex flex-col gap-1 text-xs">
              {t.fldGoal.replaceAll("{magic}", currency)}
              <input
                type="number"
                min={1000}
                required
                value={form.goal}
                onChange={(e) => setForm({ ...form, goal: e.target.value })}
                className={`${inputCls} w-36`}
              />
            </label>
            <label className="flex flex-col gap-1 text-xs">
              {t.fldHours}
              <input
                type="number"
                min={1}
                max={720}
                value={form.hours}
                onChange={(e) => setForm({ ...form, hours: e.target.value })}
                className={`${inputCls} w-40`}
              />
            </label>
            <label className="flex flex-col gap-1 text-xs">
              {t.fldDays}
              <input
                type="number"
                min={1}
                max={60}
                value={form.days}
                onChange={(e) => setForm({ ...form, days: e.target.value })}
                className={`${inputCls} w-32`}
              />
            </label>
            <button type="submit" disabled={busy || !form.torrent_id} className="baozi-button min-h-[38px] text-xs disabled:opacity-50">
              {t.createBtn}
            </button>
          </div>
        </form>
      )}

      {/* 众筹列表 */}
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead">#</td>
              <td className="colhead">{t.torrentCol}</td>
              <td className="colhead w-28">{t.goalCol}</td>
              <td className="colhead w-28">{t.raisedCol}</td>
              <td className="colhead w-20">{t.backersCol}</td>
              <td className="colhead w-40">{t.endsCol}</td>
              <td className="colhead w-24">{t.statusCol}</td>
              <td className="colhead w-56">{t.actionCol}</td>
            </tr>
          </thead>
          <tbody>
            {(rows ?? []).map((f) => {
              const pct = Math.min(100, Math.round((f.raised / Math.max(1, f.goal)) * 100));
              return (
                <tr key={f.id}>
                  <td className="rowfollow num">{f.id}</td>
                  <td className="rowfollow">
                    <a className="text-link" href={`/torrent/${f.torrent_id}`}>
                      {f.torrent_name ?? `#${f.torrent_id}`}
                    </a>
                  </td>
                  <td className="rowfollow num">{f.goal.toLocaleString()}</td>
                  <td className="rowfollow num">
                    <b className={pct >= 100 ? "text-mint" : undefined}>{f.raised.toLocaleString()}</b>
                    <span className="text-sub"> ({pct}%)</span>
                  </td>
                  <td className="rowfollow num">{f.backers}</td>
                  <td className="rowfollow text-sub">
                    {new Date(f.ends_at).toLocaleString(dateLocale(locale))}
                  </td>
                  <td className="rowfollow">
                    <span
                      className={`rounded-full px-2 py-0.5 text-[10px] font-bold ${
                        f.status === 0 ? "bg-sun/30" : f.status === 1 ? "bg-mint/30" : "bg-sky-soft"
                      }`}
                    >
                      {statusLabel(f.status)}
                    </span>
                  </td>
                  <td className="rowfollow">
                    {f.status === 0 && new Date(f.ends_at) > new Date() ? (
                      <span className="flex items-center gap-1">
                        <input
                          type="number"
                          min={1}
                          placeholder={t.amountPh.replaceAll("{magic}", currency)}
                          value={amounts[f.id] ?? ""}
                          onChange={(e) => setAmounts((a) => ({ ...a, [f.id]: e.target.value }))}
                          className="min-h-[30px] w-24 rounded-[var(--r-sm)] border border-line bg-cloud px-2 text-xs"
                          aria-label={t.contributeBtn}
                        />
                        <button
                          type="button"
                          disabled={busy || !Number(amounts[f.id])}
                          onClick={() => contribute(f)}
                          className="min-h-[30px] rounded-full bg-sky px-3 text-[11px] font-bold text-white disabled:opacity-50"
                        >
                          {t.contributeBtn}
                        </button>
                      </span>
                    ) : (
                      "—"
                    )}
                  </td>
                </tr>
              );
            })}
            {rows !== null && rows.length === 0 && (
              <tr>
                <td colSpan={8} className="py-6 text-center text-sub">
                  {t.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {/* 我的参与 */}
      {mineOpen && (
        <div>
          <h3 className="mb-2 text-sm font-bold">{t.mine}</h3>
          <table className="nexus-table text-xs">
            <tbody>
              <tr>
                <td className="colhead">#</td>
                <td className="colhead">{t.raisedCol}</td>
                <td className="colhead w-24">{t.statusCol}</td>
              </tr>
              {(mine ?? []).map((m, i) => (
                <tr key={`${m[0]}-${i}`}>
                  <td className="rowfollow num">
                    <a className="text-link" href={`/torrents`}>
                      #{m[0]}
                    </a>
                  </td>
                  <td className="rowfollow num">
                    {fmt(t.minePaid, { paid: m[1].toLocaleString(), tax: m[2].toLocaleString() })}
                  </td>
                  <td className="rowfollow">{statusLabel(m[3])}</td>
                </tr>
              ))}
              {mine !== null && mine.length === 0 && (
                <tr>
                  <td colSpan={3} className="py-4 text-center text-sub">
                    {t.mineEmpty}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
