"use client";

/**
 * 复活任务·我的任务子表（2026-11-28 从 resurrection-panel.tsx 拆出守行数门）：
 * 点「我的复活任务」展开；数据由面板拉好传入（mine 行元组）。
 */
import Link from "next/link";

import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import { EmptyState } from "@/components/ui/empty-state";

export type MineRow = [number, number, number, string, string, number];

export function ResurrectionMine({ mine }: { mine: MineRow[] | null }) {
  const { dict, locale } = useI18n();
  const t = dict.resurrect;
  const statusLabel = (s: string) =>
    s === "open" ? t.stOpen : s === "done" ? t.stDone : t.stOther;
  return (
    <div className="mt-3 border-t border-dashed border-line px-4 pt-3">
      <h3 className="mb-2 text-sm font-bold">{t.mine}</h3>
      <table className="nexus-table text-xs">
        <tbody>
          <tr>
            <th scope="col" className="colhead">
              {t.colTorrent}
            </th>
            <th scope="col" className="colhead w-28">
              {t.colRequired}
            </th>
            <th scope="col" className="colhead w-28">
              {t.colSeeded}
            </th>
            <th scope="col" className="colhead w-24">
              {t.colStatus}
            </th>
          </tr>
          {(mine ?? []).map((m, i) => (
            <tr key={`${m[0]}-${i}`}>
              <td>
                <Link href={`/torrent/${m[0]}`} className="text-link">
                  #{m[0]}
                </Link>
                <span className="ml-2 text-sub">
                  {new Date(m[3]).toLocaleString(dateLocale(locale))}
                </span>
              </td>
              <td className="num">{fmt(t.required, { n: m[2] })}</td>
              <td className="num">
                {fmt(t.seeded, { n: Math.floor(m[5] / 3600) })}
              </td>
              <td>{statusLabel(m[4])}</td>
            </tr>
          ))}
          {mine !== null && mine.length === 0 && (
            <tr>
              <td colSpan={4} className="py-4">
                <EmptyState icon="info" title={t.mineEmpty} compact />
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
