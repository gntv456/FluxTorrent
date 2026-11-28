"use client";

import { BTN_XS_GHOST } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt, fmtCur } from "@/i18n/config";
import { formatBytes, promotionBadge } from "@/lib/format";
import { promoCls, tagStyle } from "@/components/torrent-table-badges";
import {
  TorrentCards,
  TorrentPosters,
} from "@/app/(main)/torrents/_parts/torrents-cards";
import { Badge } from "@/components/ui/badge";
import type {
  TagBadge,
  TorrentListItem,
} from "@fluxtorrent/domain-types";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";
import { ResurrectionMine, type MineRow } from "@/components/resurrection-mine";
import { ResurrectionAltViews } from "@/components/resurrection-alt-views";

/** 复活任务区（0073，U3D Graveyard 口径）：
 *  GET /resurrections 可领取死种（对象含资源库行同构字段）
 *  POST /resurrections/claim {torrent_id} 认领
 *  GET /resurrections/mine 我的任务（元组 [torrent_id, reward_sparks,
 *   required_hours, claimed_at, status, seeded_seconds]） */

/** 对象形态：PreserveItem 同构字段 + 复活契约两字段（required_hours/reward_sparks）。
 *  表头与资源库统一后行渲染复用 TorrentTr（t 传本类型，字段名全对齐）。 */
type DeadRow = {
  torrent_id: number;
  name: string;
  small_descr: string | null;
  size: number;
  seeders: number;
  leechers: number;
  times_completed: number;
  comments: number;
  category_id: number;
  medium_id: number | null;
  grade_id: number | null;
  edition_id: number | null;
  official: boolean;
  anonymous: boolean;
  sticky: boolean;
  promotion: string | null;
  promotion_ends_at: string | null;
  tags?: TagBadge[] | null;
  poster: string | null;
  owner_name: string | null;
  created_at: string;
  claimed_by: string | null;
  required_hours: number;
  reward_sparks: number;
};

export function ResurrectionPanel() {
  const { dict, locale, currency } = useI18n();
  const t = dict.resurrect;
  const [dead, setDead] = useState<DeadRow[] | null>(null);
  const [mine, setMine] = useState<MineRow[] | null>(null);
  const [mineOpen, setMineOpen] = useState(false);
  const [busy, setBusy] = useState<number | null>(null);
  // P2 清偿：认领前确认（danger：认领即担 H&R 义务），confirm 退役
  const ask = useConfirm();
  const [msg, setMsg] = useState<string | null>(null);
  const [claimed, setClaimed] = useState<Record<number, string>>({});
  // 种子粒度视图（2026-11-28 站长口径：与资源库完全同款三视图）
  const [view, setView] = useState<"table" | "card" | "poster">("table");
  const catColors: Record<number, string> = {};

  const load = useCallback(async () => {
    try {
      setDead(await api.get<DeadRow[]>("/api/v1/resurrections"));
    } catch {
      // P-314：读失败不能再演成「没有待救的种」，走本面板已有的 msg 通道
      setDead([]);
      setMsg(dict.common.loadFailed);
    }
  }, [dict]);
  useEffect(() => {
    load();
  }, [load]);

  const loadMine = useCallback(async () => {
    try {
      setMine(await api.get<MineRow[]>("/api/v1/resurrections/mine"));
    } catch {
      setMine([]);
      setMsg(dict.common.loadFailed);
    }
  }, [dict]);

  async function claim(torrentId: number) {
    if (busy !== null) return;
    setBusy(torrentId);
    setMsg(null);
    try {
      const r = await api.post<{ torrent_id: number; required_hours: number }>(
        "/api/v1/resurrections/claim",
        { torrent_id: torrentId },
      );
      const text = t.claimed
        .replace("{hours}", String(r.required_hours))
        .replace("{reward}", t.reward)
        .replaceAll("{magic}", currency);
      setClaimed((c) => ({ ...c, [torrentId]: text }));
      setMsg(text);
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.claimFailed);
    } finally {
      setBusy(null);
    }
  }

  const statusLabel = (s: string) =>
    s === "open" ? t.stOpen : s === "done" ? t.stDone : t.stOther;

  return (
    <section className="brand-panel preserve-list-card">
      <div className="preserve-list-card__heading">
        <h2>🌱 {t.title}</h2>
        <button
          type="button"
          className={BTN_XS_GHOST}
          onClick={() => {
            setMineOpen((v) => !v);
            if (!mineOpen) void loadMine();
          }}
        >
          {t.mine}
        </button>
      </div>
      <p className="px-4 pb-2 text-xs text-sub">
        {t.note.replaceAll("{magic}", currency)}
      </p>
      {msg && (
        <p
          className="mx-4 mb-2 rounded-[var(--r-md)] bg-sky-soft p-2 text-xs text-ink"
          role="status"
        >
          {msg}
        </p>
      )}

      {/* 视图切换（2026-11-28 站长口径：与资源库完全同款三视图） */}
      <div className="mb-2 flex gap-1">
        {(
          [
            ["table", dict.torrents.viewTable],
            ["card", dict.torrents.viewCard],
            ["poster", dict.torrents.viewPoster],
          ] as const
        ).map(([v, label]) => (
          <button
            key={v}
            type="button"
            onClick={() => setView(v)}
            className={`rounded px-2 py-1 text-xs ${
              view === v
                ? "bg-brand-soft font-bold text-brand"
                : "text-sub hover:text-brand"
            }`}
          >
            {label}
          </button>
        ))}
      </div>

      <ResurrectionAltViews
        view={view}
        items={dead}
        claimed={claimed}
        busy={busy}
        onClaim={claim}
        dict={dict}
        colors={catColors}
      />
      {mineOpen && <ResurrectionMine mine={mine} />}
      <AskDialog {...ask} />
    </section>
  );
}
