"use client";

import {
  BTN_MD_SKY,
  BTN_SM_GHOST,
  INPUT_CARD,
  INPUT_MD,
  INPUT_W32,
} from "@/lib/ui-classes";

import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";
import { Pager } from "@/components/ui/pager";

/** 第八轮 P2-5：签到记录与手工补签（同类站 user/attendance-logs 口径） */

interface AttendanceRow {
  user_id: number;
  username: string;
  date: string;
  streak: number;
  reward: number;
  makeup: boolean;
}

export function AdminAttendance() {
  const { dict, currency } = useI18n();
  const at = dict.adminAttendance;
  const [rows, setRows] = useState<AttendanceRow[]>([]);
  const [total, setTotal] = useState(0);
  const [uid, setUid] = useState("");
  const [makeup, setMakeup] = useState("");
  const [page, setPage] = useState(1);
  const [mkUid, setMkUid] = useState("");
  const [mkDate, setMkDate] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // P2 清偿：撤销补签二次确认（danger），window.confirm 退役
  const ask = useConfirm();

  // 站点时区（UTC+8）的今天：与后端补签上界（P-96）同口径。直接切 UTC 日期
  // 会在北京 0-8 点窗口错一天（P-92 同族）。
  const siteToday = new Date(Date.now() + 8 * 3600e3)
    .toISOString()
    .slice(0, 10);

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  };

  const load = useCallback(async () => {
    const p = new URLSearchParams({ page: String(page), per_page: "20" });
    if (uid.trim()) p.set("uid", uid.trim());
    if (makeup !== "") p.set("makeup", makeup);
    try {
      const r = await api.get<{ rows: AttendanceRow[]; total: number }>(
        `/api/v1/admin/attendance?${p}`,
      );
      setRows(r.rows);
      setTotal(r.total);
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.adminAttendance.loadFail);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [uid, makeup, page]);

  useEffect(() => {
    load();
  }, [load]);

  async function makeupSubmit() {
    if (!mkUid.trim() || !mkDate) {
      flash(at.uidDateRequired);
      return;
    }
    setBusy(true);
    try {
      await api.post("/api/v1/admin/attendance/makeup", {
        user_id: Number(mkUid),
        date: mkDate,
      });
      flash(at.madeUp);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.opFail);
    } finally {
      setBusy(false);
    }
  }

  /** 撤销补签：该口是 DELETE + JSON body（与 torrents/previews 同形），
   *  api-client 的 del() 不带 body，沿用组件内 fetch 写法。 */
  async function revoke(r: AttendanceRow) {
    setBusy(true);
    try {
      const res = await fetch("/api/v1/admin/attendance/makeup", {
        method: "DELETE",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ user_id: r.user_id, date: r.date }),
      });
      if (!res.ok) {
        const j = (await res.json().catch(() => ({}))) as {
          message?: string;
        };
        throw new Error(j.message ?? at.opFail);
      }
      flash(at.deleted);
      await load();
    } catch (e) {
      flash(e instanceof Error ? e.message : at.opFail);
    } finally {
      setBusy(false);
    }
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
          {at.fUid}
          <input
            value={uid}
            onChange={(e) => {
              setUid(e.target.value);
              setPage(1);
            }}
            placeholder={at.qAll}
            className={INPUT_W32}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          {at.fType}
          <select
            value={makeup}
            onChange={(e) => {
              setMakeup(e.target.value);
              setPage(1);
            }}
            className={INPUT_CARD}
          >
            <option value="">{at.optAll}</option>
            <option value="false">{at.optNormal}</option>
            <option value="true">{at.optMakeup}</option>
          </select>
        </label>
      </section>
      <section className="flex flex-wrap items-end gap-2 rounded-[var(--r-md)] border border-line p-3">
        <h3 className="w-full text-sm font-bold">{at.mkTitle}</h3>
        <label className="flex flex-col gap-1 text-xs">
          {at.fUid}
          <input
            value={mkUid}
            onChange={(e) => setMkUid(e.target.value)}
            placeholder="4"
            className="min-h-[40px] w-24 rounded-[var(--r-sm)] border border-line px-2"
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          {at.fDate}
          <input
            type="date"
            value={mkDate}
            max={siteToday}
            onChange={(e) => setMkDate(e.target.value)}
            className={INPUT_MD}
          />
        </label>
        <button disabled={busy} onClick={makeupSubmit} className={BTN_MD_SKY}>
          {at.mkBtn}
        </button>
        <p className="text-xs text-sub">{at.mkHint}</p>
      </section>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">{at.thUser}</td>
            <td className="colhead">{at.thDate}</td>
            <td className="colhead">{at.thStreak}</td>
            <td className="colhead">{fmt(at.thReward, { magic: currency })}</td>
            <td className="colhead">{at.thType}</td>
            <td className="colhead">{at.thOp}</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={`${r.user_id}-${r.date}`}>
              <td>
                <a
                  href={`/admin/users/${r.user_id}`}
                  className="font-bold text-link"
                >
                  {r.username}
                </a>
              </td>
              <td className="num">{r.date}</td>
              <td className="num">{r.streak}</td>
              <td className="num">{r.reward}</td>
              <td>
                {r.makeup ? (
                  <span className="rounded-full bg-sun/30 px-2 py-0.5">
                    {at.makeup}
                  </span>
                ) : (
                  at.normal
                )}
              </td>
              <td>
                {r.makeup ? (
                  <button
                    disabled={busy}
                    onClick={() =>
                      ask.open({
                        title: at.delBtn,
                        message: fmt(at.delConfirm, {
                          user: r.username,
                          date: r.date,
                        }),
                        tone: "danger",
                        busy,
                        run: () => revoke(r),
                      })
                    }
                    className={BTN_SM_GHOST}
                  >
                    {at.delBtn}
                  </button>
                ) : (
                  <span className="text-sub">—</span>
                )}
              </td>
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td colSpan={6} className="py-6 text-center text-sub">
                {at.empty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
      <Pager
        page={page}
        pages={Math.max(1, Math.ceil(total / 20))}
        total={total}
        onStep={setPage}
      />
      <AskDialog {...ask} />
    </div>
  );
}
