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

/** 我的数据总览（旧站 my_data_stats 口径）：传输量/分享率/做种下载/发布数 */
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

  const cells: { label: string; value: string; cls?: string }[] = [
    { label: dict.my.uploaded, value: formatBytesLocal(me.uploaded), cls: "text-mint" },
    { label: dict.my.downloaded, value: formatBytesLocal(me.downloaded), cls: "text-coral" },
    { label: dict.my.ratio, value: ratio, cls: Number(ratio) < 1 ? "text-coral" : "text-mint" },
    { label: dict.my.classLabel, value: me.class_name ?? "—" },
    { label: dict.my.seedingLabel, value: String(me.seeding), cls: "text-mint" },
    { label: dict.my.leechingLabel, value: String(me.leeching), cls: "text-coral" },
    { label: dict.my.uploadsCount, value: String(me.uploads) },
    { label: dict.my.bookmarksCount, value: String(me.bookmarks) },
  ];

  return (
    <section className="rounded-[var(--r-lg)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
      <div className="mb-3 flex items-center gap-3">
        <span aria-hidden className="text-[40px] leading-none">
          🦉
        </span>
        <div>
          <h2 className="font-display text-lg">{me.username}</h2>
          <p className="text-xs text-sub">{me.class_name ?? ""}</p>
        </div>
      </div>
      <dl className="num grid grid-cols-2 gap-3 text-sm sm:grid-cols-4">
        {cells.map((c) => (
          <div
            key={c.label}
            className="rounded-[var(--r-md)] border border-line bg-cloud/60 p-3"
          >
            <dt className="text-xs text-sub">{c.label}</dt>
            <dd className={`mt-1 text-lg ${c.cls ?? ""}`}>{c.value}</dd>
          </div>
        ))}
      </dl>
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
