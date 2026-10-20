"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";
import { Badge } from "@/components/ui/badge";

/** 定向众筹后台管理（闭环审查 C11）：列表 / 状态筛选 / 关闭（可退款）。
 *  此前站长无任何出口，网关掉单或需要干预时只能改库。 */

interface FundingRow {
  id: number;
  torrent_id: number;
  torrent_name: string | null;
  creator_id: number;
  creator_name: string | null;
  goal: number;
  raised: number;
  backers: number;
  hours: number;
  status: number;
  ends_at: string;
  created_at: string;
}

/** 众筹状态徽章（2026-10-16 S2）：原为色类串表，现映射 Badge 的
 *  tone/variant——warn 浅底=进行中、success 浅底=已达标、
 *  neutral 浅底=已结束/流拍（中性灰）。 */
const ST_TONE: Record<
  number,
  { tone: "warn" | "success" | "neutral"; variant: "soft" }
> = {
  0: { tone: "warn", variant: "soft" },
  1: { tone: "success", variant: "soft" },
  2: { tone: "neutral", variant: "soft" },
  3: { tone: "neutral", variant: "soft" },
};

const SEL_CLS =
  "min-h-[36px] rounded-md border border-line bg-cloud px-2 text-sm";
const ACT_REFUND = "text-sky-deep hover:underline disabled:opacity-40";
const ACT_CLOSE = "text-tomato hover:underline disabled:opacity-40";

export function AdminFundings() {
  const { dict, locale } = useI18n();
  const t = dict.adminFundings;
  const c = dict.common;
  const [rows, setRows] = useState<FundingRow[]>([]);
  const [status, setStatus] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // P2 清偿：关闭众筹二次确认状态机（danger）
  const ask = useConfirm();

  const load = useCallback(async () => {
    const p = new URLSearchParams();
    if (status) p.set("status", status);
    try {
      setRows(await api.get<FundingRow[]>(`/api/v1/admin/fundings?${p}`));
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : c.networkError);
      setRows([]);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status]);

  useEffect(() => {
    void load();
  }, [load]);

  const stLabel = (s: number) =>
    ({
      0: t.stOpen,
      1: t.stReached,
      2: t.stClosed,
      3: t.stClosing,
    })[s] ?? String(s);

  async function cancel(row: FundingRow, refund: boolean) {
    setBusy(true);
    try {
      const r = await api.post<{ refunded: number }>(
        "/api/v1/admin/fundings/cancel",
        { id: row.id, refund },
      );
      setMsg(
        refund
          ? fmt(t.canceledRefunded, { n: String(r.refunded) })
          : t.canceled,
      );
      await load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : c.networkError);
    } finally {
      setBusy(false);
    }
  }

  /** P2 清偿：关闭众筹二次确认（danger），window.confirm 退役 */
  function askCancel(row: FundingRow, refund: boolean) {
    const key = refund ? t.confirmCancelRefund : t.confirmCancelNoRefund;
    ask.open({
      title: refund ? t.cancelRefund : t.cancelNoRefund,
      message: fmt(key, { id: row.id }),
      tone: "danger",
      busy,
      run: () => cancel(row, refund),
    });
  }

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      <section className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-xs">
          {t.fStatus}
          <select
            value={status}
            onChange={(e) => setStatus(e.target.value)}
            className={SEL_CLS}
          >
            <option value="">{t.optAll}</option>
            <option value="0">{t.stOpen}</option>
            <option value="1">{t.stReached}</option>
            <option value="2">{t.stClosed}</option>
          </select>
        </label>
        <span className="ml-auto text-xs text-sub">{t.hint}</span>
      </section>
      <div className="brand-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead">ID</td>
              <td className="colhead">{t.thTorrent}</td>
              <td className="colhead">{t.thCreator}</td>
              <td className="colhead">{t.thProgress}</td>
              <td className="colhead">{t.thBackers}</td>
              <td className="colhead">{t.thStatus}</td>
              <td className="colhead">{t.thEnds}</td>
              <td className="colhead">{t.thAction}</td>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.id}>
                <td className="num">{r.id}</td>
                <td className="max-w-[16rem] truncate">
                  <a href={`/torrent/${r.torrent_id}`} className="text-link">
                    {r.torrent_name ?? `#${r.torrent_id}`}
                  </a>
                </td>
                <td>
                  <a
                    href={`/admin/users/${r.creator_id}`}
                    className="text-link"
                  >
                    {r.creator_name ?? `#${r.creator_id}`}
                  </a>
                </td>
                <td className="num">
                  {r.raised} / {r.goal}
                </td>
                <td className="num">{r.backers}</td>
                <td>
                  <Badge
                    tone={ST_TONE[r.status]?.tone ?? "neutral"}
                    variant={ST_TONE[r.status]?.variant ?? "soft"}
                  >
                    {stLabel(r.status)}
                  </Badge>
                </td>
                <td className="text-sub">
                  {new Date(r.ends_at).toLocaleDateString(dateLocale(locale))}
                </td>
                <td>
                  {r.status === 0 && (
                    <div className="flex gap-2">
                      <button
                        disabled={busy}
                        onClick={() => askCancel(r, true)}
                        className={ACT_REFUND}
                      >
                        {t.cancelRefund}
                      </button>
                      <button
                        disabled={busy}
                        onClick={() => askCancel(r, false)}
                        className={ACT_CLOSE}
                      >
                        {t.cancelNoRefund}
                      </button>
                    </div>
                  )}
                </td>
              </tr>
            ))}
            {rows.length === 0 && (
              <tr>
                <td colSpan={8} className="py-6 text-center text-sub">
                  {t.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      <AskDialog {...ask} />
    </div>
  );
}
