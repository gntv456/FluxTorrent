"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 下载/做种记录（NP viewsnatches.php 口径）：当前做种/下载中/已完成 + 上传下载量 */
interface SnatchRow {
  user_id: number;
  username: string;
  uploaded: number;
  downloaded: number;
  seeded_seconds: number;
  completed_at: string | null;
  seeding: boolean;
  leeching: boolean;
}

export function SnatchList({ torrentId }: { torrentId: number }) {
  const { dict } = useI18n();
  const t = dict.snatches2 ?? {
    title: "下载记录",
    load: "加载下载记录",
    loading: "加载中…",
    empty: "暂无下载记录",
    colUser: "用户",
    colUploaded: "上传",
    colDownloaded: "下载",
    colSeeded: "做种时长",
    colCompleted: "完成时间",
    stSeeding: "做种中",
    stLeeching: "下载中",
  };
  const [rows, setRows] = useState<SnatchRow[] | null>(null);
  const [busy, setBusy] = useState(false);

  if (rows === null) {
    return (
      <button
        type="button"
        disabled={busy}
        onClick={async () => {
          setBusy(true);
          try {
            setRows(await api.get<SnatchRow[]>(`/api/v1/torrents/${torrentId}/snatches`));
          } catch {
            setRows([]);
          } finally {
            setBusy(false);
          }
        }}
        className="min-h-[32px] rounded-full border border-line px-3 text-xs font-bold text-sub disabled:opacity-50"
      >
        {busy ? t.loading : `👁 ${t.load}`}
      </button>
    );
  }

  return (
    <table className="nexus-table">
      <thead>
        <tr>
          <td className="colhead">{t.colUser}</td>
          <td className="colhead">{t.colUploaded}</td>
          <td className="colhead">{t.colDownloaded}</td>
          <td className="colhead">{t.colSeeded}</td>
          <td className="colhead">{t.colCompleted}</td>
        </tr>
      </thead>
      <tbody>
        {rows.map((r) => (
          <tr key={r.user_id}>
            <td>
              <a href={`/users/${r.user_id}`} className="font-bold">
                {r.username}
              </a>
              {r.seeding && (
                <span className="ml-1 fun-status fun-status--normal">{t.stSeeding}</span>
              )}
              {r.leeching && (
                <span className="ml-1 fun-status fun-status--dull">{t.stLeeching}</span>
              )}
            </td>
            <td className="num">{(r.uploaded / 1024 ** 3).toFixed(2)} GB</td>
            <td className="num">{(r.downloaded / 1024 ** 3).toFixed(2)} GB</td>
            <td className="num">{(r.seeded_seconds / 3600).toFixed(1)} h</td>
            <td className="nowrap">
              {r.completed_at ? new Date(r.completed_at).toLocaleString() : "—"}
            </td>
          </tr>
        ))}
        {rows.length === 0 && (
          <tr>
            <td colSpan={5} className="py-6 text-center text-sub">
              {t.empty}
            </td>
          </tr>
        )}
      </tbody>
    </table>
  );
}
