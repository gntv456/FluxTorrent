"use client";

import Link from "next/link";
import { useI18n } from "@/i18n/client";

interface HrRow {
  torrent_id: number;
  name: string;
  size: number;
  completed_at: string | null;
  seeded_seconds: number;
  hr_flag: boolean;
  remaining_seconds: number | null;
}

/** H&R 表格（colhead 六列：种子/大小/完成时间/已做种/剩余/状态） */
export function MyHrTable({ rows, locale }: { rows: HrRow[]; locale: string }) {
  const { dict } = useI18n();
  const t = dict.myhr;
  const dl = locale === "zh-TW" ? "zh-TW" : locale === "en" ? "en-US" : "zh-CN";
  return (
    <table className="nexus-table">
      <tbody>
        <tr>
          <td className="colhead">{t.colTorrent}</td>
          <td className="colhead">{t.colSize}</td>
          <td className="colhead">{t.colCompleted}</td>
          <td className="colhead">{t.colSeeded}</td>
          <td className="colhead">{t.colRemaining}</td>
          <td className="colhead">{t.colStatus}</td>
        </tr>
        {rows.map((r) => {
          const ok = (r.seeded_seconds >= 432000);
          return (
            <tr key={r.torrent_id}>
              <td>
                <Link href={`/torrent/${r.torrent_id}`} className="font-bold">
                  {r.name}
                </Link>
              </td>
              <td className="num">{(r.size / 1024 ** 3).toFixed(2)} GB</td>
              <td>
                {r.completed_at
                  ? new Date(r.completed_at).toLocaleString(dl)
                  : "—"}
              </td>
              <td className="num">
                {(r.seeded_seconds / 3600).toFixed(1)} h
              </td>
              <td className="num">
                {ok ? "—" : `${((r.remaining_seconds ?? 0) / 3600).toFixed(1)} h`}
              </td>
              <td>
                {r.hr_flag ? (
                  <span className="fun-status fun-status--banned">{t.stFlagged}</span>
                ) : ok ? (
                  <span className="fun-status fun-status--normal">{t.stOk}</span>
                ) : (
                  <span className="fun-status fun-status--dull">{t.stPending}</span>
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
  );
}
