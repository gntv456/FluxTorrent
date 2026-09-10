"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
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

interface BookmarkRow {
  torrent_id: number;
  name: string;
  small_descr: string | null;
  size: number;
  seeders: number;
  leechers: number;
  created_at: string;
}

type Kind = "seeding" | "leeching" | "completed" | "uploads" | "bookmarks";

/** 我的种子列表（旧站 getusertorrentlist + usercp 收藏夹口径）：做种/完成/发布/收藏 */
export function MyTorrentList() {
  const { dict } = useI18n();
  const router = useRouter();
  const search = useSearchParams();
  const [kind, setKind] = useState<Kind>(
    search.get("tab") === "bookmarks" ? "bookmarks" : "seeding",
  );
  const [rows, setRows] = useState<SnatchRow[] | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    setRows(null);
    setErr(null);
    const url =
      kind === "bookmarks"
        ? "/api/v1/me/bookmarks?limit=50"
        : `/api/v1/me/torrentlist?kind=${kind}&limit=50`;
    api
      .get<SnatchRow[]>(url)
      .then(setRows)
      .catch((e) =>
        setErr(
          e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : dict.common.loadFailed,
        ),
      );
  }, [kind, dict]);

  async function removeBookmark(torrentId: number) {
    try {
      await api.put(`/api/v1/torrents/${torrentId}/bookmark`, { on: false });
      setRows((prev) => prev?.filter((r) => r.torrent_id !== torrentId) ?? null);
      router.refresh();
    } catch {
      // 取消失败静默保留原行
    }
  }

  function switchKind(k: Kind) {
    setKind(k);
    if (k === "bookmarks") router.replace("/my?tab=bookmarks", { scroll: false });
    else if (search.get("tab")) router.replace("/my", { scroll: false });
  }

  const tabs: [Kind, string][] = [
    ["seeding", dict.my.tlSeeding],
    ["completed", dict.my.tlCompleted],
    ["uploads", dict.my.tlUploads],
    ["bookmarks", dict.my.bookmarksCount],
  ];

  return (
    <section className="nexus-detail">
      <div className="mb-3 flex gap-2 overflow-x-auto">
        {tabs.map(([k, label]) => (
          <button
            key={k}
            type="button"
            onClick={() => switchKind(k)}
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
        <p className="py-4 text-center text-sm text-sub">
          {kind === "bookmarks" ? dict.my.bmEmpty : dict.my.tlEmpty}
        </p>
      )}
      {rows && rows.length > 0 && kind === "bookmarks" && (
        <table className="nexus-table">
          <thead>
            <tr>
              <td className="colhead">{dict.torrents.colTitle}</td>
              <td className="colhead w-24">{dict.torrents.colSize}</td>
              <td className="colhead w-20 text-center">{dict.torrents.colSeeders}</td>
              <td className="colhead w-20 text-center">{dict.torrents.colLeechers}</td>
              <td className="colhead w-24 text-right" />
            </tr>
          </thead>
          <tbody>
            {(rows as unknown as BookmarkRow[]).map((r) => (
              <tr key={r.torrent_id}>
                <td className="min-w-0 max-w-0">
                  <Link
                    href={`/torrent/${r.torrent_id}`}
                    className="block truncate font-bold text-ink hover:text-sky"
                  >
                    {r.name}
                  </Link>
                  {r.small_descr && (
                    <p className="truncate text-xs text-sub">{r.small_descr}</p>
                  )}
                </td>
                <td className="num w-24 text-sub">{formatBytes(r.size)}</td>
                <td className="num w-20 text-center text-mint">{r.seeders}</td>
                <td className="num w-20 text-center text-coral">{r.leechers}</td>
                <td className="w-24 text-right">
                  <button
                    type="button"
                    onClick={() => removeBookmark(r.torrent_id)}
                    aria-label={dict.my.bmRemove}
                    className="rounded-full border border-line px-2 py-0.5 text-xs text-sub hover:border-coral hover:text-coral"
                  >
                    {dict.my.bmRemove}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {rows && rows.length > 0 && kind !== "bookmarks" && (
        <table className="nexus-table">
          <thead>
            <tr>
              <td className="colhead">{dict.torrents.colTitle}</td>
              <td className="colhead w-24">{dict.torrents.colSize}</td>
              <td className="colhead w-20 text-center">{dict.torrents.colSeeders}</td>
              <td className="colhead w-20 text-center">{dict.torrents.colLeechers}</td>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.torrent_id}>
                <td className="min-w-0 max-w-0">
                  <Link
                    href={`/torrent/${r.torrent_id}`}
                    className="block truncate font-bold text-ink hover:text-sky"
                  >
                    {r.name}
                  </Link>
                </td>
                <td className="num w-24 text-sub">{formatBytes(r.size)}</td>
                <td
                  className={`num w-20 text-center ${r.seeding ? "text-mint" : "text-sub"}`}
                >
                  {r.seeders}
                </td>
                <td
                  className={`num w-20 text-center ${r.leeching ? "text-coral" : "text-sub"}`}
                >
                  {r.leechers}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
