"use client";

/**
 * 复活任务·卡片视图（2026-11-28 从 resurrection-panel.tsx 拆出守行数门）：
 * 非表格视图（card/poster 共用此紧凑卡网格）；认领动作由面板传入。
 */
import Link from "next/link";

import { useI18n } from "@/i18n/client";
import { formatBytes } from "@/lib/format";

export function ResurrectionCardView({
  items,
  contractHead,
  claimed,
  busy,
  onClaim,
}: {
  items: {
    torrent_id: number;
    name: string;
    size: number;
  }[];
  /** 契约句首段（contract 冒号前） */
  contractHead: string;
  claimed: Record<number, string>;
  busy: number | null;
  onClaim: (torrentId: number) => void;
}) {
  const { dict } = useI18n();
  return (
    <div className="grid grid-cols-2 gap-3 p-4 xl:grid-cols-4">
      {items.map((r) => (
        <div key={r.torrent_id} className="brand-panel p-3">
          <Link href={`/torrent/${r.torrent_id}`} className="text-link">
            {r.name}
          </Link>
          <div className="mt-1 text-xs text-sub">
            {formatBytes(r.size)} · {contractHead}
          </div>
          <div className="mt-2 text-right">
            {claimed[r.torrent_id] ? (
              <span className="text-xs font-bold text-mint">
                {claimed[r.torrent_id]}
              </span>
            ) : (
              <button
                type="button"
                disabled={busy !== null}
                onClick={() => onClaim(r.torrent_id)}
                className="min-h-11 rounded-full border
                  border-[var(--compat-orange-dark)] px-3 text-xs font-bold
                  text-[var(--compat-orange-dark)] disabled:opacity-50"
              >
                认领复活
              </button>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}
