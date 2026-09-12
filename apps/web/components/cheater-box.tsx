"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

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

/** 作弊者信箱（好学站 cheaterbox.php 复刻）：管理组查看可疑会话检测记录 */
export function CheaterBox() {
  const { dict, locale } = useI18n();
  const t = dict.cheaterbox;
  const [rows, setRows] = useState<CheaterRow[] | null>(null);
  const [noPerm, setNoPerm] = useState(false);

  useEffect(() => {
    api
      .get<CheaterRow[]>("/api/v1/admin/cheaters")
      .then(setRows)
      .catch((e) => {
        if (e instanceof ApiError && e.code === 403) setNoPerm(true);
        else setRows([]);
      });
  }, []);

  const th = "px-3 py-2 text-left text-xs font-bold whitespace-nowrap";
  const td = "px-3 py-2 text-sm align-middle";

  if (noPerm) {
    return (
      <section className="baozi-panel">
        <header className="baozi-panel__head">
          <h2>🚫 {t.title}</h2>
        </header>
        <p className="funbox__empty">{t.noPerm}</p>
      </section>
    );
  }

  return (
    <section className="baozi-panel">
      <header className="baozi-panel__head">
        <h2>
          <span aria-hidden="true">🚫</span> {t.title}
        </h2>
        {rows !== null && <small>{rows.length}</small>}
      </header>
      {rows === null ? (
        <p className="funbox__empty">…</p>
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
                <tr key={`${r.user_id}-${r.torrent_id ?? "x"}`} className="border-b border-dashed border-[var(--border-soft)]">
                  <td className={td}>
                    <a className="font-bold text-[var(--baozi-orange-dark)] hover:underline" href={`/users/${r.user_id}`}>
                      {r.username}
                    </a>
                  </td>
                  <td className={`${td} max-w-[320px] truncate`}>
                    {r.torrent_id ? (
                      <a className="hover:underline" href={`/torrent/${r.torrent_id}`}>
                        {r.name ?? `#${r.torrent_id}`}
                      </a>
                    ) : (
                      "—"
                    )}
                  </td>
                  <td className={td}>{fmtSpeed(r.upspeed)}</td>
                  <td className={td}>{fmtBytes(r.uploaded_delta)}</td>
                  <td className={`${td} whitespace-nowrap text-xs text-[var(--text-faint)]`}>
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
