"use client";

import { BTN_XS_GHOST } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt, fmtCur } from "@/i18n/config";
import { formatBytes, promotionBadge } from "@/lib/format";
import { promoCls, tagStyle } from "@/components/torrent-table-badges";
import { Badge } from "@/components/ui/badge";
import type {
  TagBadge,
  TorrentListItem,
} from "@fluxtorrent/domain-types";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";
import { ResurrectionMine, type MineRow } from "@/components/resurrection-mine";

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

      <div className="brand-wide-table-scroll">
        <table className="nexus-table torrents-table">
          <thead>
            <tr>
              <th className="w-12">{dict.torrents.colType}</th>
              <th>{dict.torrents.colTitle}</th>
              <th className="w-16" title={dict.torrents.colComments}>
                {dict.torrents.colComments}
              </th>
              <th className="w-20" title={dict.torrents.alive}>
                {dict.torrents.alive}
              </th>
              <th className="w-20">{dict.torrents.colSize}</th>
              <th className="w-16">{dict.torrents.colSeeders}</th>
              <th className="w-16">{dict.torrents.colLeechers}</th>
              <th className="w-16">{dict.torrents.colCompleted}</th>
              <th className="w-24">{dict.torrents.colActions}</th>
            </tr>
          </thead>
          <tbody>
            {(dead ?? []).map((r) => {
              const claimedText = claimed[r.torrent_id];
              const ageDays = Math.floor(
                (Date.now() - new Date(r.created_at).getTime()) / 86400000,
              );
              return (
                <tr key={r.torrent_id}>
                  <td className="w-12 text-sub">#{r.category_id}</td>
                  <td>
                    <Link
                      href={`/torrent/${r.torrent_id}`}
                      className="text-link"
                    >
                      {r.name}
                    </Link>
                    {(() => {
                      const promo = promotionBadge(
                        r.promotion as TorrentListItem["promotion"] | null,
                      );
                      if (!promo) return null;
                      const hLeft = r.promotion_ends_at
                        ? Math.ceil(
                            (new Date(r.promotion_ends_at).getTime() -
                              Date.now()) /
                              3600000,
                          )
                        : null;
                      const leftCompact =
                        hLeft && hLeft > 0
                          ? hLeft >= 48
                            ? Math.floor(hLeft / 24) + "D"
                            : hLeft + "H"
                          : null;
                      return (
                        <span
                          className={`${promoCls(promo.key)} torrents-promo--text`}
                        >
                          {promo.key.toUpperCase()}
                          {leftCompact && (
                            <b className="torrents-promo-left">
                              {leftCompact}
                            </b>
                          )}
                        </span>
                      );
                    })()}
                    <div className="text-xs text-sub">
                      {fmtCur(
                        t.contract,
                        {
                          h: String(r.required_hours),
                          sparks: String(r.reward_sparks),
                        },
                        currency,
                      )}
                    </div>
                    {r.small_descr && (
                      <div className="text-xs text-sub">{r.small_descr}</div>
                    )}
                    {(r.tags?.length ?? 0) > 0 && (
                      <div className="flex flex-wrap gap-1 pt-0.5">
                        {r.tags!.map((tg) => (
                          <span
                            key={tg.id}
                            className="rounded px-1.5 text-xs"
                            style={tagStyle(tg)}
                          >
                            {tg.name}
                          </span>
                        ))}
                      </div>
                    )}
                  </td>
                  <td className="num text-sub">{r.comments || ""}</td>
                  <td className="num text-sub">
                    {ageDays > 0 ? `${ageDays}d` : ""}
                  </td>
                  <td className="num text-sub">{formatBytes(r.size)}</td>
                  <td className="num">{r.seeders}</td>
                  <td className="num">{r.leechers}</td>
                  <td className="num">{r.times_completed}</td>
                  <td className="text-right">
                    {claimedText ? (
                      <span className="text-xs font-bold text-mint">
                        {claimedText}
                      </span>
                    ) : (
                      <button
                        type="button"
                        disabled={busy !== null}
                        onClick={() =>
                          ask.open({
                            title: t.claimBtn,
                            message: t.note.replaceAll("{magic}", currency),
                            tone: "danger",
                            run: () => claim(r.torrent_id),
                          })
                        }
                        className={"min-h-11 rounded-full border border-[var(--compat-orange-dark)] "
    + "px-3 text-xs font-bold text-[var(--compat-orange-dark)] "
    + "disabled:opacity-50"}
                      >
                        {t.claimBtn}
                      </button>
                    )}
                  </td>
                </tr>
              );
            })}
            {dead !== null && dead.length === 0 && (
              <tr>
                <td colSpan={9} className="py-6 text-center text-sub">
                  {t.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

{mineOpen && (<ResurrectionMine mine={mine} />)}
      <AskDialog {...ask} />
    </section>
  );
}
