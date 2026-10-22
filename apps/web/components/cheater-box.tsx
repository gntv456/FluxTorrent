"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { CheatCasePanel } from "./cheat-case-panel";
import { CheatEventsTable } from "./cheat-events-table";
import { CheatAppealQueue } from "./cheat-appeal-queue";
import { RiskWatchlist } from "./risk-watchlist";
import { CheatAccuracyPanel } from "./cheat-accuracy-panel";

interface CheaterRow {
  user_id: number;
  username: string;
  torrent_id: number | null;
  name: string | null;
  upspeed: number;
  uploaded_delta: number;
  announced_at: string;
}

function fmtSpeed(n: number): string {
  const mb = n / (1024 * 1024);
  if (mb >= 1) return `${mb.toFixed(1)} MB/s`;
  return `${(n / 1024).toFixed(0)} KB/s`;
}

function fmtBytes(n: number): string {
  const gb = n / (1024 * 1024 * 1024);
  if (Math.abs(gb) >= 1) return `${gb.toFixed(2)} GB`;
  return `${(n / (1024 * 1024)).toFixed(0)} MB`;
}

/** 作弊者信箱（同类站 cheaterbox.php 复刻）：可疑会话 + 作弊事件处置待办 */
export function CheaterBox() {
  const { dict, locale } = useI18n();
  const t = dict.cheaterbox;
  const [rows, setRows] = useState<CheaterRow[] | null>(null);
  // P-615：取数失败曾被演成「0 条 + 本页为空」——复核人员会以为没有可疑会话
  const [failed, setFailed] = useState(false);
  const [noPerm, setNoPerm] = useState(false);

  useEffect(() => {
    api
      .get<CheaterRow[]>("/api/v1/admin/cheaters")
      .then(setRows)
      .catch((e) => {
        // 信封码 2003 = 无权限（403 是 HTTP 状态，P-501）。
        if (e instanceof ApiError && e.code === 2003) setNoPerm(true);
        else {
          setRows([]);
          setFailed(true);
        }
      });
  }, []);

  const th = "px-3 py-2 text-left text-xs font-bold whitespace-nowrap";
  const td = "px-3 py-2 text-sm align-middle";

  if (noPerm) {
    return (
      <section className="brand-panel">
        <header className="brand-panel__head">
          <h2>🚫 {t.title}</h2>
        </header>
        <p className="funbox__empty">{t.noPerm}</p>
      </section>
    );
  }

  return (
    <section className="brand-panel">
      <header className="brand-panel__head">
        <h2>
          <span aria-hidden="true">🚫</span> {t.title}
        </h2>
        {rows !== null && !failed && <small>{rows.length}</small>}
      </header>

      {/* 作弊事件待办 + 反作弊案件队列（0367 M1）+ 风险观察名单（0368 M2）
          + 反作弊申诉复核队列（0369 M3）+ 规则准确率（0372 M5） */}
      <CheatEventsTable />
      <CheatCasePanel />
      <RiskWatchlist />
      <CheatAppealQueue />
      <CheatAccuracyPanel />

      {rows === null ? (
        <p className="funbox__empty">…</p>
      ) : failed ? (
        <p role="alert" className="funbox__empty">
          {dict.common.loadFailed}
        </p>
      ) : rows.length === 0 ? (
        <p className="funbox__empty">{t.empty}</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full border-collapse">
            <thead>
              <tr className="border-b border-[var(--border-soft)]">
                <th className={th}>{t.colUser}</th>
                <th className={th}>{t.colTorrent}</th>
                <th className={th}>{t.colSpeed}</th>
                <th className={th}>{t.colDelta}</th>
                <th className={th}>{t.colTime}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr
                  key={`${r.user_id}-${r.torrent_id ?? "x"}`}
                  className={
                    "border-b border-dashed " +
                    "border-[var(--border-soft)]"
                  }
                >
                  <td className={td}>
                    <a
                      className={
                        "font-bold text-[var(--brand-orange-dark)] " +
                        "hover:underline"
                      }
                      href={`/users/${r.user_id}`}
                    >
                      {r.username}
                    </a>
                  </td>
                  <td className={`${td} max-w-[320px] truncate`}>
                    {r.torrent_id ? (
                      <a
                        className="hover:underline"
                        href={`/torrent/${r.torrent_id}`}
                      >
                        {r.name ?? `#${r.torrent_id}`}
                      </a>
                    ) : (
                      "—"
                    )}
                  </td>
                  <td className={td}>{fmtSpeed(r.upspeed)}</td>
                  <td className={td}>{fmtBytes(r.uploaded_delta)}</td>
                  <td
                    className={
                      `${td} whitespace-nowrap text-xs ` +
                      "text-[var(--text-faint)]"
                    }
                  >
                    {new Date(r.announced_at).toLocaleString(locale)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
