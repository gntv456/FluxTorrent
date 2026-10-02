"use client";

import { useCallback, useEffect, useState } from "react";
import { PANEL_LG } from "@/lib/ui-classes";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { HistoryStrip } from "@/components/game/game-kit-feedback";

export interface PrizeChip {
  label: string;
  weight_permille: number;
  payout: number;
  value?: number;
}

interface Linkage {
  fishing_event_active: boolean;
  fishing_event_unlocked: boolean;
  weekly_seed_hours: number;
  fishing_event_threshold: number;
}
interface Rod {
  level: number;
  max: number;
  next_cost: number;
  window_bonus_ms: number;
}
interface Album {
  got: number;
  total: number;
  fishes: {
    label: string;
    rarity: number;
    event: boolean;
    count: number;
    mult: number;
  }[];
}

/** 稀有度 → 鱼形（纯展示，r1..r5） */
const FISH_FACE = ["🐟", "🐟", "🐠", "🐠", "🐡", "🦈"];

/** 钓鱼侧栏：奖池档位 + 渔汛横幅 + 鱼竿升级 + 渔获图鉴 + 战绩。
 *  生态数据自取（linkage / rod / collection），主流程只管抛竿起竿；
 *  抽成组件是为了让专注页守住 300 行。 */
export function FishingSide({
  prizes,
  ticket,
  hist,
}: {
  prizes: PrizeChip[];
  ticket: number;
  hist: { net: number; game?: string }[];
}) {
  const { dict, currency } = useI18n();
  const t = dict.games;
  const tf = t.fishing;
  const [link, setLink] = useState<Linkage | null>(null);
  const [rod, setRod] = useState<Rod | null>(null);
  const [album, setAlbum] = useState<Album | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const load = useCallback(async () => {
    // 侧栏生态件：缺一个不拦主玩法，静默降级
    try {
      setLink(await api.get<Linkage>("/api/v1/games/linkage/status"));
    } catch {
      /* ignore */
    }
    try {
      setRod(await api.get<Rod>("/api/v1/games/fishing/rod"));
    } catch {
      /* ignore */
    }
    try {
      setAlbum(await api.get<Album>("/api/v1/games/fishing/collection"));
    } catch {
      /* ignore */
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function upgrade() {
    if (busy) return;
    setBusy(true);
    setErr(null);
    try {
      setRod(await api.post<Rod>("/api/v1/games/fishing/rod/upgrade", {}));
    } catch (e) {
      setErr(
        e instanceof ApiError
          ? e.code === 1002
            ? e.message
            : (dict.errors[e.code] ?? e.message)
          : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  const ev = link
    ? link.fishing_event_active
      ? link.fishing_event_unlocked
        ? tf.eventOn
        : tf.eventLocked
            .replace("{need}", String(link.fishing_event_threshold))
            .replace("{n}", String(link.weekly_seed_hours))
      : tf.eventOff.replace("{need}", String(link.fishing_event_threshold))
    : null;

  return (
    <div className={PANEL_LG}>
      {ev && (
        <p className={`fs-event${link?.fishing_event_active ? " on" : ""}`}>
          {ev}
        </p>
      )}

      <h2 className="mb-2 font-display text-base">{tf.prizePool}</h2>
      <div className="flex flex-wrap gap-1.5">
        {prizes.map((p, i) => (
          <span
            key={i}
            className={`rounded-full px-2 py-0.5 text-[11px] font-bold ${
              (p.value ?? p.payout * ticket) >= ticket * 5
                ? "bg-sun-soft text-[var(--warning)]"
                : (p.value ?? p.payout * ticket) > ticket
                  ? "bg-mint-soft text-[var(--mint)]"
                  : "bg-[var(--surface-sunken)] text-sub"
            }`}
          >
            {p.label} {(p.weight_permille / 10).toFixed(1)}%
          </span>
        ))}
      </div>
      <p className="mt-2 text-[11px] text-sub">{tf.poolNote}</p>

      <h2 className="mb-2 mt-3 font-display text-base">{tf.rodTitle}</h2>
      <div className="fs-rod">
        <span className="fs-rod-ico" aria-hidden>
          🎣
        </span>
        <span className="num fs-rod-lv">
          {tf.rodLv.replace("{n}", String(rod?.level ?? 1))}
          {rod && rod.level >= rod.max ? ` · ${tf.rodMax}` : ""}
        </span>
        <span className="num fs-rod-bonus">
          {tf.rodBonus.replace("{n}", String(rod?.window_bonus_ms ?? 0))}
        </span>
        {rod && rod.level < rod.max && (
          <button
            type="button"
            className="arc-call sun fs-rod-up"
            onClick={upgrade}
            disabled={busy}
          >
            {busy
              ? tf.rodUpgrading
              : tf.rodUpgrade
                  .replace("{n}", String(rod.next_cost))
                  .replace("{magic}", currency)}
          </button>
        )}
      </div>
      {err && <p className="text-xs text-danger">{err}</p>}

      <h2 className="mb-2 mt-3 font-display text-base">
        <span id="album">{tf.albumTitle}</span>
        {album && (
          <span className="num fs-album-n">
            {tf.albumCount
              .replace("{got}", String(album.got))
              .replace("{total}", String(album.total))}
          </span>
        )}
      </h2>
      <div className="fs-album">
        {(album?.fishes ?? []).map((f) => (
          <div
            key={`${f.event ? "e" : "s"}-${f.label}`}
            className={`fs-fish${f.count > 0 ? " got" : ""} r${f.rarity}`}
            title={`${f.label} ×${f.mult}`}
          >
            <span className="fs-fish-face" aria-hidden>
              {FISH_FACE[f.rarity] ?? "🐟"}
            </span>
            <span className="fs-fish-name">{f.label}</span>
            {f.event && <span className="fs-fish-ev">{tf.albumEvent}</span>}
            <span className="num fs-fish-n">
              {f.count > 0 ? `×${f.count}` : "—"}
            </span>
          </div>
        ))}
        {album && album.got === 0 && (
          <p className="text-[11px] text-sub">{tf.albumEmpty}</p>
        )}
      </div>

      <h2 className="mb-2 mt-3 font-display text-base">{t.history}</h2>
      <HistoryStrip rounds={hist} />
    </div>
  );
}
