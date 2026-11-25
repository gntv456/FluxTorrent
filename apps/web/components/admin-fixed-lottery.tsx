"use client";

/** 固定抽签运营面板（0520）：活动列表/创建/发布/取消/立即开奖 +
 *  中奖与履约状态 + 领取审核队列。数据全部来自 /admin/lottery/* 口；
 *  文案在 dict.adminLottery（三语）。 */

import { useCallback, useEffect, useState } from "react";
import { useI18n } from "@/i18n/client";
import { api, ApiError } from "@/lib/api-client";
import { AdminLotteryCreate } from "./admin-lottery-create";
import { AdminLotteryClaims } from "./admin-lottery-claims";
import { apiErrorMessage } from "@/i18n/api-error-message";
import {
  BTN_SM_GHOST,
  BTN_SM_SKY,
  PANEL_MD,
} from "@/lib/ui-classes";
import { SkeletonBlock } from "@/components/ui/skeleton";
import {
  FULFILL_KEYS,
  STATUS_KEYS,
  TRIGGER_KEYS,
  WINNER_KEYS,
  dictLabel,
} from "./admin-lottery-shared";

type Row = {
  id: number;
  code: string;
  title: string;
  status: string;
  trigger: string;
  ticketPrice: number;
};
type WinnerRow = {
  winnerId: number;
  username: string;
  prizeName: string;
  slotNo: number;
  status: string;
  fulfillStatus: string | null;
  attempts: number | null;
  lastError: string | null;
};
type ClaimRow = {
  winnerId: number;
  username: string;
  prizeName: string;
  submission: Record<string, unknown>;
};

export function AdminFixedLottery() {
  const { dict } = useI18n();
  const t = dict.adminLottery;
  const [rows, setRows] = useState<Row[] | null>(null);
  const [sel, setSel] = useState<number | null>(null);
  const [winners, setWinners] = useState<WinnerRow[] | null>(null);
  const [claims, setClaims] = useState<ClaimRow[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const r = await api.get<{ rows: Row[] }>(
        "/api/v1/admin/lottery/campaigns"
      );
      setRows(r.rows);
    } catch {
      setRows(null);
    }
    try {
      const q = await api.get<{ rows: ClaimRow[] }>(
        "/api/v1/admin/lottery/claims"
      );
      setClaims(q.rows);
    } catch {
      setClaims(null);
    }
  }, []);
  useEffect(() => {
    void load();
  }, [load]);

  const loadWinners = useCallback(async (id: number) => {
    setWinners(null);
    try {
      const r = await api.get<{ rows: WinnerRow[] }>(
        `/api/v1/admin/lottery/campaigns/${id}/winners`
      );
      setWinners(r.rows);
    } catch {
      setWinners(null);
    }
  }, []);

  async function act(fn: () => Promise<unknown>, ok: string) {
    setErr(null);
    setMsg(null);
    try {
      await fn();
      setMsg(ok);
      await load();
    } catch (e) {
      setErr(
        e instanceof ApiError ? apiErrorMessage(dict, e) : String(e)
      );
    }
  }

  return (
    <div className="flex flex-col gap-4">
      {err && (
        <div className="rounded-lg bg-rose-500/10 px-3 py-2
          text-sm text-rose-500">
          {err}
        </div>
      )}
      {msg && (
        <div className="rounded-lg bg-emerald-500/10 px-3 py-2
          text-sm text-emerald-600">
          {msg}
        </div>
      )}

      <AdminLotteryCreate
        t={t}
        act={act}
      />

      <section className={PANEL_MD}>
        <h2 className="mb-2 font-bold">{t.list}</h2>
        {rows === null ? (
          <SkeletonBlock height={120} />
        ) : rows.length === 0 ? (
          <p className="text-sm opacity-60">{t.empty}</p>
        ) : (
          <table className="w-full text-sm">
            <tbody>
              {rows.map((r) => (
                <tr key={r.id} className="border-t border-line/40">
                  <td className="py-1.5 pr-2 font-bold">{r.title}</td>
                  <td className="py-1.5 pr-2 opacity-60">{r.code}</td>
                  <td className="py-1.5 pr-2">
                    {dictLabel(t, STATUS_KEYS, r.status)}
                  </td>
                  <td className="py-1.5 pr-2">
                    {dictLabel(t, TRIGGER_KEYS, r.trigger)}
                  </td>
                  <td className="py-1.5">
                    <div className="flex gap-1">
                      {r.status === "draft" && (
                        <button className={BTN_SM_GHOST}
                          onClick={() => void act(
                            () => api.post(
                              `/api/v1/admin/lottery/campaigns/${r.id}/publish`,
                              {}
                            ),
                            "ok"
                          )}>
                          {t.publish}
                        </button>
                      )}
                      {r.status === "open" && (
                        <>
                          <button className={BTN_SM_SKY}
                            onClick={() => void act(
                              () => api.post(
                                `/api/v1/admin/lottery/campaigns/${r.id}/draw`,
                                {}
                              ),
                              "ok"
                            )}>
                            {t.draw}
                          </button>
                          <button className={BTN_SM_GHOST}
                            onClick={() => {
                              const reason = window.prompt(t.cancelReason);
                              if (reason?.trim()) {
                                void act(
                                  () => api.del(
                                    `/api/v1/admin/lottery/campaigns/${r.id}`,
                                    { reason }
                                  ),
                                  "ok"
                                );
                              }
                            }}>
                            {t.cancel}
                          </button>
                        </>
                      )}
                      <button className={BTN_SM_GHOST}
                        onClick={() => {
                          setSel(r.id);
                          void loadWinners(r.id);
                        }}>
                        {t.winners}
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {sel !== null && winners !== null && (
          <table className="mt-2 w-full text-xs">
            <tbody>
              {winners.map((w) => (
                <tr key={w.winnerId} className="border-t border-line/40">
                  <td className="py-1 pr-2">{w.username}</td>
                  <td className="py-1 pr-2">{w.prizeName} #{w.slotNo}</td>
                  <td className="py-1 pr-2">
                    {dictLabel(t, WINNER_KEYS, w.status)}
                  </td>
                  <td className="py-1 pr-2 opacity-60">
                    {w.fulfillStatus
                      ? dictLabel(t, FULFILL_KEYS, w.fulfillStatus)
                      : ""}
                    {w.lastError ? `: ${w.lastError}` : ""}
                  </td>
                  <td className="py-1">
                    {(w.fulfillStatus === "failed" ||
                      w.fulfillStatus === "exhausted") && (
                      <button className={BTN_SM_GHOST}
                        onClick={() => void act(
                          () => api.post(
                            `/api/v1/admin/lottery/fulfillments/` +
                              `${w.winnerId}/retry`,
                            {}
                          ),
                          "ok"
                        )}>
                        {t.retry}
                      </button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>

    </div>
  );
}
