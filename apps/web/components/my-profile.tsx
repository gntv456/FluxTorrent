"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

interface MeProfile {
  id: number;
  username: string;
  class_name: string | null;
  uploaded: number;
  downloaded: number;
  seeding: number;
  leeching: number;
  uploads: number;
  bookmarks: number;
}

/** 我的数据总览（旧站 usercp/my_data_stats 口径）：rowhead/rowfollow 经典表格 */
export function MyProfileCard() {
  const { dict } = useI18n();
  const [me, setMe] = useState<MeProfile | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    api
      .get<MeProfile>("/api/v1/me")
      .then(setMe)
      .catch((e) =>
        setErr(
          e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : dict.common.loadFailed,
        ),
      );
  }, [dict]);

  if (err) return <p className="text-sm text-sub">{err}</p>;
  if (!me) return <p className="text-sm text-sub">{dict.my.loading}</p>;

  const ratio =
    me.downloaded === 0
      ? "∞"
      : (me.uploaded / me.downloaded).toFixed(2);

  const rows: { label: string; value: React.ReactNode }[] = [
    { label: dict.my.classLabel, value: me.class_name ?? "—" },
    {
      label: dict.my.uploaded,
      value: <span className="num text-mint">{formatBytesLocal(me.uploaded)}</span>,
    },
    {
      label: dict.my.downloaded,
      value: <span className="num text-coral">{formatBytesLocal(me.downloaded)}</span>,
    },
    {
      label: dict.my.ratio,
      value: (
        <span className={`num ${Number(ratio) < 1 ? "text-coral" : "text-mint"}`}>{ratio}</span>
      ),
    },
    { label: dict.my.seedingLabel, value: <span className="num text-mint">{me.seeding}</span> },
    { label: dict.my.leechingLabel, value: <span className="num text-coral">{me.leeching}</span> },
    { label: dict.my.uploadsCount, value: <span className="num">{me.uploads}</span> },
    { label: dict.my.bookmarksCount, value: <span className="num">{me.bookmarks}</span> },
  ];

  return (
    <section className="nexus-detail">
      <table className="nexus-table nexus-form">
        <thead>
          <tr>
            <td colSpan={2} className="colhead">
              {me.username}
              {me.class_name ? <span className="text-sub"> · {me.class_name}</span> : null}
            </td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.label}>
              <td className="rowhead">{r.label}</td>
              <td className="rowfollow">{r.value}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}

function formatBytesLocal(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v >= 100 ? v.toFixed(0) : v.toFixed(2)} ${units[i]}`;
}
