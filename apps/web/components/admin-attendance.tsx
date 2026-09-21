"use client";

import { BTN_MD_SKY, BTN_SM_GHOST, INPUT_CARD, INPUT_MD, INPUT_W32 } from "@/lib/ui-classes";

import { useI18n } from "@/i18n/client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 第八轮 P2-5：签到记录与手工补签（好学站 user/attendance-logs 口径） */

interface AttendanceRow {
  user_id: number;
  username: string;
  date: string;
  streak: number;
  reward: number;
  makeup: boolean;
}

export function AdminAttendance() {
  const { currency } = useI18n();
  const [rows, setRows] = useState<AttendanceRow[]>([]);
  const [total, setTotal] = useState(0);
  const [uid, setUid] = useState("");
  const [makeup, setMakeup] = useState("");
  const [page, setPage] = useState(1);
  const [mkUid, setMkUid] = useState("");
  const [mkDate, setMkDate] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

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
      flash(e instanceof ApiError ? e.message : "加载失败");
    }
  }, [uid, makeup, page]);

  useEffect(() => {
    load();
  }, [load]);

  async function makeupSubmit() {
    if (!mkUid.trim() || !mkDate) {
      flash("UID 与日期必填");
      return;
    }
    setBusy(true);
    try {
      await api.post("/api/v1/admin/attendance/makeup", {
        user_id: Number(mkUid),
        date: mkDate,
      });
      flash("已补签");
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
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
          用户 UID
          <input
            value={uid}
            onChange={(e) => {
              setUid(e.target.value);
              setPage(1);
            }}
            placeholder="留空看全部"
            className={INPUT_W32}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          类型
          <select
            value={makeup}
            onChange={(e) => {
              setMakeup(e.target.value);
              setPage(1);
            }}
            className={INPUT_CARD}
          >
            <option value="">全部</option>
            <option value="false">正常签到</option>
            <option value="true">补签</option>
          </select>
        </label>
      </section>
      <section className="flex flex-wrap items-end gap-2 rounded-[var(--r-md)] border border-line p-3">
        <h3 className="w-full text-sm font-bold">手工补签</h3>
        <label className="flex flex-col gap-1 text-xs">
          用户 UID
          <input
            value={mkUid}
            onChange={(e) => setMkUid(e.target.value)}
            placeholder="4"
            className="min-h-[40px] w-24 rounded-[var(--r-sm)] border border-line px-2"
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          日期
          <input
            type="date"
            value={mkDate}
            onChange={(e) => setMkDate(e.target.value)}
            className={INPUT_MD}
          />
        </label>
        <button
          disabled={busy}
          onClick={makeupSubmit}
          className={BTN_MD_SKY}
        >
          补签
        </button>
        <p className="text-xs text-sub">当日已有签到记录的用户不可重复补签。</p>
      </section>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">用户</td>
            <td className="colhead">日期</td>
            <td className="colhead">连续天数</td>
            <td className="colhead">获得{currency}</td>
            <td className="colhead">类型</td>
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
                    补签
                  </span>
                ) : (
                  "正常"
                )}
              </td>
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td colSpan={5} className="py-6 text-center text-sub">
                暂无签到记录
              </td>
            </tr>
          )}
        </tbody>
      </table>
      <div className="flex items-center justify-between text-sm text-sub">
        <span>共 {total} 条</span>
        <div className="flex gap-2">
          <button
            disabled={page <= 1}
            onClick={() => setPage(page - 1)}
            className={BTN_SM_GHOST}
          >
            上一页
          </button>
          <span>第 {page} 页</span>
          <button
            disabled={rows.length < 20}
            onClick={() => setPage(page + 1)}
            className={BTN_SM_GHOST}
          >
            下一页
          </button>
        </div>
      </div>
    </div>
  );
}
