"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { formatBytes } from "@/lib/format";

interface SnatchRow {
  torrent_id: number;
  name: string;
  size: number;
  seeders: number;
  leechers: number;
  seeding: boolean;
  leeching: boolean;
  completed_at: string | null;
  done: number;
}

type Kind = "seeding" | "leeching" | "completed" | "uploads";

/** 我的种子列表（旧站 getusertorrentlist 口径）：做种/完成/发布 */
export function MyTorrentList() {
  const { dict } = useI18n();
  const [kind, setKind] = useState<Kind>("seeding");
  const [rows, setRows] = useState<SnatchRow[] | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    setRows(null);
    setErr(null);
    api
      .get<SnatchRow[]>(`/api/v1/me/torrentlist?kind=${kind}&limit=50`)
      .then(setRows)
      .catch((e) =>
        setErr(
          e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : dict.common.loadFailed,
        ),
      );
  }, [kind, dict]);

  const tabs: [Kind, string][] = [
    ["seeding", dict.my.tlSeeding],
    ["completed", dict.my.tlCompleted],
    ["uploads", dict.my.tlUploads],
  ];

  return (
    <section className="rounded-[var(--r-lg)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
      <div className="mb-3 flex gap-2 overflow-x-auto">
        {tabs.map(([k, label]) => (
          <button
            key={k}
            type="button"
            onClick={() => setKind(k)}
            aria-current={kind === k ? "true" : undefined}
            className={`min-h-[44px] flex shrink-0 items-center rounded-full px-4 text-sm ${
              kind === k
                ? "bg-sky-deep text-white"
                : "border border-line bg-white text-ink"
            }`}
          >
            {label}
          </button>
        ))}
      </div>
      {err && <p className="py-4 text-center text-sm text-sub">{err}</p>}
      {!err && rows === null && (
        <p className="py-4 text-center text-sm text-sub">{dict.my.loading}</p>
      )}
      {rows && rows.length === 0 && (
        <p className="py-4 text-center text-sm text-sub">{dict.my.tlEmpty}</p>
      )}
      {rows && rows.length > 0 && (
        <ul className="flex flex-col divide-y divide-line">
          {rows.map((r) => (
            <li key={r.torrent_id} className="flex items-center gap-3 py-2.5">
              <Link
                href={`/torrent/${r.torrent_id}`}
                className="min-w-0 flex-1 truncate text-sm font-bold text-ink hover:text-sky"
              >
                {r.name}
              </Link>
              <span className="num shrink-0 text-xs text-sub">
                {formatBytes(r.size)}
              </span>
              <span
                className={`num shrink-0 text-xs ${
                  r.seeding ? "text-mint" : r.leeching ? "text-coral" : "text-sub"
                }`}
              >
                ↑{r.seeders} ↓{r.leechers}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
