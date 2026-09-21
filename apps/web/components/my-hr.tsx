"use client";

import { useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";

interface HrRow {
  torrent_id: number;
  torrent_name: string;
  required_seconds: number;
  seeded_seconds: number;
  deadline: string;
  status: string;
}

/** 我的 H&R 表（/me/hr 口径：种子/要求/已做种/截止/状态 + 自助免罪） */
export function MyHrTable({ rows, locale }: { rows: HrRow[]; locale: string }) {
  const { dict, currency } = useI18n();
  const router = useRouter();
  const t = dict.myhr;
  const dl = locale === "zh-TW" ? "zh-TW" : locale === "en" ? "en-US" : "zh-CN";
  const [busyId, setBusyId] = useState<number | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

  async function selfPardon(tid: number) {
    if (!window.confirm(dict.myhr2.pardonConfirm.replace("{magic}", currency)))
      return;
    setBusyId(tid);
    setMsg(null);
    try {
      await api.post("/api/v1/me/hr/pardon", { torrent_id: tid });
      setMsg(dict.myhr2.pardonOk);
      router.refresh();
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
    } finally {
      setBusyId(null);
    }
  }

  return (
    <div className="flex flex-col gap-2">
      <table className="nexus-table">
        <tbody>
          <tr>
            <td className="colhead">{t.colTorrent}</td>
            <td className="colhead">{t.colRequired}</td>
            <td className="colhead">{t.colSeeded}</td>
            <td className="colhead">{t.colDeadline}</td>
            <td className="colhead">{t.colStatus}</td>
            <td className="colhead" />
          </tr>
          {rows.map((r) => {
            const hours = (secs: number) => `${(secs / 3600).toFixed(1)} h`;
            return (
              <tr key={r.torrent_id}>
                <td>
                  <Link href={`/torrent/${r.torrent_id}`} className="font-bold">
                    {r.torrent_name}
                  </Link>
                </td>
                <td className="num">{hours(r.required_seconds)}</td>
                <td className="num">{hours(r.seeded_seconds)}</td>
                <td className="nowrap">
                  {new Date(r.deadline).toLocaleString(dl)}
                </td>
                <td>
                  {r.status === "violated" ? (
                    <span className="fun-status fun-status--banned">
                      {t.stFlagged}
                    </span>
                  ) : r.status === "satisfied" ? (
                    <span className="fun-status fun-status--normal">
                      {t.stOk}
                    </span>
                  ) : r.status === "pardoned" ? (
                    <span className="fun-status fun-status--normal">
                      {dict.myhr2.stPardoned}
                    </span>
                  ) : (
                    <span className="fun-status fun-status--dull">
                      {t.stPending}
                    </span>
                  )}
                </td>
                <td>
                  {r.status === "violated" && (
                    <button
                      type="button"
                      disabled={busyId === r.torrent_id}
                      onClick={() => selfPardon(r.torrent_id)}
                      className="min-h-[28px] rounded-full border border-line px-3 text-[11px] font-bold text-sky-deep disabled:opacity-50"
                      title={dict.myhr2.pardonNote.replace("{magic}", currency)}
                    >
                      {dict.myhr2.pardonBtn.replace("{magic}", currency)}
                    </button>
                  )}
                </td>
              </tr>
            );
          })}
          {rows.length === 0 && (
            <tr>
              <td colSpan={6} className="py-8 text-center text-sub">
                {t.empty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
      {msg && (
        <p className="text-xs text-sub" role="status">
          {msg}
        </p>
      )}
    </div>
  );
}
