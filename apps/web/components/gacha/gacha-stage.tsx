"use client";

/**
 * /gacha 星轨卡册（设计稿 抽卡页-高保真样图.html 的生产实现）。
 *
 * 结构照搬设计稿：HUD + 五个视图（卡池 / 演出 / 卡册 / 概率 / 记录）+ 七类弹窗，
 * 整页一屏不滚动，视图内局部滚动。数据一次取齐（RSC 传入 banners / rates /
 * rarities / cards / me），只有换池时补拉一次 rates。
 *
 * 与后端契约：
 *  - POST /gacha/draw {banner_id,count,idempotency_key}
 *    → rows[] 里 type ∈ card|shard|miss
 *  - POST /gacha/{dismantle|exchange|levelup} {card_id,idempotency_key}
 *  - GET  /gacha/me → 双余额 + 图鉴 + 近 30 抽（recent 行键是 type/r/wasPity）
 * 幂等键在响应丢失时必须复用（重复扣券 / 重复扣碎片），这里按动作路径缓存。
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, ApiError, isRetryableNetError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { Badge } from "@/components/ui/badge";
import { toast } from "@/components/ui/toast";
import { CardFace, familyOf, SIGS } from "@/components/gacha/gacha-card";
import { stageCopy, fill } from "@/i18n/gacha-stage/stage-copy";
import { Modal } from "@/components/gacha/gacha-modal";
import {
  Astro,
  AstroDot,
  DotMatrix,
  RuleStep,
  SKY,
} from "@/components/gacha/gacha-astro";
import { Icon, Sprite } from "@/components/gacha/gacha-icons";
import {
  ago,
  daysLeft,
  fmtDay,
  fmtTime,
  pct,
  rarVar,
} from "@/components/gacha/gacha-astro";
import type {
  DrawRow,
  ModalKind,
  StageBanner,
  StageCard,
  StageMe,
  StageRarity,
  StageRates,
  StageRecent,
  StageView,
} from "@/components/gacha/stage-types";


const HARD_FALLBACK = 90;

/** 长 SVG path 折行拼接，避免单行超宽 */
const j = (...p: string[]) => p.join("");


/* ---------- 主组件 ---------- */
export function GachaStage({
  banners: bannersProp,
  rarities,
  cards,
  me: meProp,
  rates: ratesProp,
}: {
  banners: StageBanner[];
  rarities: StageRarity[];
  cards: StageCard[];
  me: StageMe | null;
  rates: StageRates | null;
}) {
  const { locale } = useI18n();
  const t = stageCopy(locale);
  const banners = bannersProp;

  const [bannerId, setBannerId] = useState(banners[0]?.id ?? 0);
  const [rates, setRates] = useState<StageRates | null>(ratesProp);
  const [me, setMe] = useState<StageMe | null>(meProp);
  const [view, setView] = useState<StageView>("pool");
  const [rows, setRows] = useState<DrawRow[] | null>(null);
  const [drawing, setDrawing] = useState(false);
  const [skip, setSkip] = useState(false);
  const [flipped, setFlipped] = useState(false);
  const [modal, setModal] = useState<ModalKind | null>(null);
  const [modalCard, setModalCard] = useState<StageCard | null>(null);
  const [confirm, setConfirm] = useState<{
    card: StageCard;
    cost: number;
  } | null>(null);
  const [filter, setFilter] = useState<string>("all");
  const [query, setQuery] = useState("");
  const [note, setNote] = useState<string | null>(null);
  const [busyCard, setBusyCard] = useState<number | null>(null);
  const [unknownKey, setUnknownKey] = useState<string | null>(null);
  const [ratesLoading, setRatesLoading] = useState(false);

  const idem = useRef<Record<string, string>>({});
  const drawKey = useRef("");

  const banner = banners.find((b) => b.id === bannerId) ?? banners[0] ?? null;
  const hard = banner?.pityHard || rates?.banner.pityHard || HARD_FALLBACK;
  const soft = banner?.pitySoft || rates?.banner.pitySoft || 60;
  const starsOf = useCallback(
    (r?: string | null) => rarities.find((x) => x.key === r)?.stars ?? 1,
    [rarities],
  );

  const rarityFam = (r?: string | null) => {
    const m = rarities.find((x) => x.key === r);
    return familyOf(r ?? "N", m?.stars ?? 1);
  };

  /* 最近 30 抽（按当前池过滤；recent 行键 type/r/wasPity） */
  const recentAll = me?.recent ?? [];
  const recent = useMemo(
    () =>
      recentAll
        .filter((r) =>
          banner ? r.bannerId == null || r.bannerId === banner.id : true,
        )
        .slice(0, 30),
    [recentAll, banner],
  );
  const pity = recent.find((r) => r.pityAt != null)?.pityAt ?? 0;
  const hardLeft = Math.max(0, hard - pity);
  const litCount = me?.litCount ?? 0;
  const litTotal = me?.litTotal ?? cards.length;
  const litPct = litTotal ? Math.min(100, (litCount / litTotal) * 100) : 0;

  /* 卡名表 */
  const cardById = useMemo(() => new Map(cards.map((c) => [c.id, c])), [cards]);
  const ownedById = useMemo(
    () => new Map((me?.cards ?? []).map((c) => [c.id, c])),
    [me],
  );
  const nameOfRow = (r: DrawRow) =>
    (r.cardId != null && cardById.get(r.cardId)?.name) ||
    (r.r ? (rarities.find((x) => x.key === r.r)?.label ?? r.r) : "");

  /* 合成目标：星数最高的档里的第一张卡（优先 SSR），成本来自卡面 synth_shards */
  const topRarity = useMemo(() => {
    const cardTiers = rates?.rows.filter((r) => r.type === "card") ?? [];
    const sorted = [...cardTiers].sort((a, b) => starsOf(b.r) - starsOf(a.r));
    return sorted[0]?.r ?? null;
  }, [rates, starsOf]);
  const targetCard = useMemo(() => {
    const pool = cards.filter((c) => c.rarity === (topRarity ?? c.rarity));
    const t2 = pool.filter((c) => !ownedById.get(c.id)?.exchanged);
    return t2[0] ?? pool[0] ?? cards[0] ?? null;
  }, [cards, topRarity, ownedById]);

  const reloadMe = useCallback(async () => {
    try {
      const next = await api.get<StageMe>("/api/v1/gacha/me");
      setMe(next);
    } catch {
      /* 匿名或网络故障：保持当前画面，错误在动作路径提示 */
    }
  }, []);

  useEffect(() => {
    if (!meProp) void reloadMe();
  }, [meProp, reloadMe]);

  /* 换池：补拉该池 rates */
  useEffect(() => {
    if (!banner) return;
    if (ratesProp && ratesProp.banner.id === banner.id) {
      setRates(ratesProp);
      return;
    }
    let cancelled = false;
    setRatesLoading(true);
    api
      .get<StageRates>(`/api/v1/gacha/banner/${banner.id}/rates`)
      .then((r) => {
        if (!cancelled) setRates(r);
      })
      .catch(() => {
        if (!cancelled) setRates(null);
      })
      .finally(() => {
        if (!cancelled) setRatesLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [banner, ratesProp]);

  /* 抽卡 */
  const draw = useCallback(
    async (count: 1 | 10, reuse = false) => {
      if (!banner || drawing) return;
      if (!me) {
        setNote(t.loginFirst);
        return;
      }
      const cost =
        count === 10
          ? (banner.tenCost ?? banner.ticketCost * 10)
          : banner.ticketCost;
      if (me.ticketBalance < cost) {
        setNote(fill(t.noTickets, { n: cost }));
        return;
      }
      setDrawing(true);
      setNote(null);
      setUnknownKey(null);
      if (!reuse || !drawKey.current) {
        const rand = Math.floor(Math.random() * 1e6);
        drawKey.current = `ui-gacha-${Date.now()}-${rand}`;
      }
      try {
        const resp = await api.post<{
          rows: DrawRow[];
          ticketBalance: number;
          shardBalance: number;
        }>("/api/v1/gacha/draw", {
          banner_id: banner.id,
          count,
          idempotency_key: drawKey.current,
        });
        setRows(resp.rows);
        setMe((prev) =>
          prev
            ? {
                ...prev,
                ticketBalance: resp.ticketBalance,
                shardBalance: resp.shardBalance,
              }
            : prev,
        );
        setView("reveal");
        setFlipped(false);
        window.dispatchEvent(new CustomEvent("flux:gacha-drawn"));
        void reloadMe();
        drawKey.current = "";
        const gold = resp.rows.some((r) => starsOf(r.r) >= 5);
        toast.success(gold ? "SSR" : t.gained, {
          desc: gold ? t.gained : undefined,
        });
      } catch (e) {
        setNote(e instanceof ApiError ? e.message : String(e));
        if (isRetryableNetError(e)) setUnknownKey(drawKey.current);
      } finally {
        setDrawing(false);
      }
    },
    [banner, drawing, me, reloadMe, starsOf, t],
  );

  /* 翻牌：跳过直接全翻，否则逐张错峰 */
  useEffect(() => {
    if (!rows || view !== "reveal") return;
    if (skip) {
      setFlipped(true);
      return;
    }
    const id = window.setTimeout(() => setFlipped(true), 260);
    return () => window.clearTimeout(id);
  }, [rows, view, skip]);

  /* 卡册动作（幂等同键重试） */
  const act = useCallback(
    async (
      path: "dismantle" | "exchange" | "levelup",
      card: StageCard,
      reuse = false,
    ) => {
      const key = `${path}:${card.id}`;
      if (busyCard) return;
      setBusyCard(card.id);
      setNote(null);
      if (!reuse || !idem.current[key]) {
        idem.current[key] = `ui-gacha-${path}-${card.id}-${Date.now()}`;
      }
      try {
        await api.post(`/api/v1/gacha/${path}`, {
          card_id: card.id,
          idempotency_key: idem.current[key],
        });
        delete idem.current[key];
        setUnknownKey(null);
        await reloadMe();
        const owned = ownedById.get(card.id);
        if (path === "dismantle") {
          setNote(fill(t.doneDismantle, { n: card.dupeShards }));
        } else if (path === "levelup") {
          const lv = owned?.lv ?? 1;
          setNote(fill(t.doneLevelup, { a: lv, b: lv + 1 }));
        } else {
          setNote(fill(t.doneExchange, { name: card.name }));
        }
      } catch (e) {
        setNote(e instanceof ApiError ? e.message : String(e));
        if (isRetryableNetError(e)) setUnknownKey(key);
      } finally {
        setBusyCard(null);
      }
    },
    [busyCard, ownedById, reloadMe, t],
  );

  const retryUnknown = () => {
    if (!unknownKey) return;
    if (unknownKey.startsWith("draw")) {
      void draw(rows?.length === 10 ? 10 : 1, true);
      return;
    }
    const [path, id] = unknownKey.split(":");
    const card = cardById.get(Number(id));
    if (
      card &&
      (path === "dismantle" || path === "exchange" || path === "levelup")
    ) {
      void act(path, card, true);
    }
  };

  const openCard = (c: StageCard) => {
    setModalCard(c);
    setModal("card");
  };

  const copyCsv = async () => {
    const head = "seq,type,rarity,shards,wasPity,pityAt,drawnAt";
    const body = recent
      .slice()
      .reverse()
      .map((r) =>
        [
          r.seq,
          r.type,
          r.r ?? "",
          r.shards,
          r.wasPity ? 1 : 0,
          r.pityAt ?? "",
          r.drawnAt ?? "",
        ].join(","),
      )
      .join("\n");
    try {
      await navigator.clipboard.writeText(`${head}\n${body}`);
      setNote(t.copied);
    } catch {
      setNote(t.loadFailed);
    }
  };

  /* 视图内统计 */
  const held = useMemo(() => {
    const acc: Record<string, number> = {};
    for (const c of me?.cards ?? []) {
      const fam = rarityFam(c.rarity);
      acc[fam] = (acc[fam] ?? 0) + Math.max(1, c.held);
    }
    return acc;
  }, [me]); // eslint-disable-line react-hooks/exhaustive-deps

  const albumList = useMemo(() => {
    const owned = (me?.cards ?? []).map((c) => ({
      card: c as StageCard,
      held: c.held,
      lv: c.lv,
      lvMax: c.lvMax,
      locked: false,
    }));
    const extraLocked = cards
      .filter((c) => !ownedById.has(c.id))
      .map((c) => ({
        card: c,
        held: 0,
        lv: 0,
        lvMax: c.lvMax,
        locked: true,
      }));
    return [...owned, ...extraLocked];
  }, [me, cards, ownedById]);

  const filteredAlbum = albumList.filter((x) => {
    const fam = rarityFam(x.card.rarity);
    if (filter === "locked" && !x.locked) return false;
    if (filter !== "all" && filter !== "locked" && fam !== filter) return false;
    if (query && !x.card.name.toLowerCase().includes(query.toLowerCase()))
      return false;
    return true;
  });

  const chips: Array<{ key: string; label: string; n: number }> = [
    { key: "all", label: t.all, n: albumList.length },
    {
      key: "SSR",
      label: "SSR",
      n: albumList.filter(
        (x) => rarityFam(x.card.rarity) === "SSR" && !x.locked,
      ).length,
    },
    {
      key: "SR",
      label: "SR",
      n: albumList.filter((x) => rarityFam(x.card.rarity) === "SR" && !x.locked)
        .length,
    },
    {
      key: "R",
      label: "R",
      n: albumList.filter((x) => rarityFam(x.card.rarity) === "R" && !x.locked)
        .length,
    },
    {
      key: "locked",
      label: t.locked,
      n: albumList.filter((x) => x.locked).length,
    },
  ];

  const targetSynth = targetCard?.synthShards ?? 0;
  const shards = me?.shardBalance ?? 0;
  const synthPct = targetSynth
    ? Math.min(100, (shards / targetSynth) * 100)
    : 0;

  const ssrRows = recent.filter((r) => r.type === "card" && starsOf(r.r) >= 5);
  const cardRows = recent.filter((r) => r.type === "card" && r.cardId != null);
  const avg = ssrRows.length
    ? (cardRows.length / ssrRows.length).toFixed(1)
    : "—";

  /* 当前弹窗标题 */
  const modalTitle: Record<ModalKind, string> = {
    rules: t.modalRules,
    wallet: t.modalWallet,
    progress: t.modalProgress,
    rateSpec: t.modalSpec,
    export: t.modalExport,
    confirm: t.modalConfirm,
    card: t.modalCard,
  };

  const gainRows = recent
    .filter((r) => r.type === "card" && r.cardId != null)
    .slice(0, 3);

  /** 重复卡分解碎片：档位在 cards 元数据上（rates 行的 shards 允许为 0） */
  const dupeOf = (r: string) =>
    cards.find((c) => c.rarity === r)?.dupeShards ?? 0;

  const ramp = rates ? (rates.banner.pityRamp ?? 5).toFixed(1) : "5.0";
  const softSpan =
    `SSR 概率自 ${soft + 1} 抽起按 ${ramp}% / 抽 ` + "阶梯递增，直至命中。";

  const ruleSteps = (
    <>
      <RuleStep no="01" b={fill(t.softRule, { n: soft })} span={softSpan} />
      <RuleStep
        no="02"
        b={fill(t.hardRule, { n: hard })}
        span={`累计 ${hard} 抽未出 SSR，第 ${hard} 抽必得，计数归零。`}
      />
      <RuleStep
        no="03"
        b={t.tenRule}
        span="每次十连至少 1 张 SR 以上，不与硬保底叠加。"
      />
      <RuleStep
        no="04"
        b={t.inheritRule}
        span="保底计数按卡池独立，卡池结束后清零。"
      />
    </>
  );

  const exchangeSteps = (
    <>
      <RuleStep
        no="01"
        b="重复卡自动分解"
        span={`按档位结算碎片：${
          (rates?.rows ?? [])
            .filter((r) => r.type === "card")
            .map((r) => `${r.r} ${dupeOf(r.r) || "—"}`)
            .join(" / ") || "以卡面定义为准"
        }。`}
      />
      <RuleStep
        no="02"
        b={`${targetSynth || 520} 碎片兑换当期 ${targetCard?.rarity ?? ""}`.trimEnd()}
        span="每季每张限兑 1 次，兑换后卡面标记「本季已兑换」。"
      />
    </>
  );

  return (
    <section className="gx" data-locale={locale}>
      <Sprite />
      <div className="app" data-v={view}>
        {/* HUD */}
        <header className="hud">
          <div className="hud-l">
            <a className="hud-back" href="/games" aria-label={t.backArcade}>
              <Icon name="back" size={15} />
            </a>
            <div className="brand">
              <svg
                className="bmark"
                viewBox="0 0 32 32"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.3"
                aria-hidden="true"
              >
                <circle cx="16" cy="16" r="11.4" />
                <circle cx="16" cy="16" r="5.4" opacity=".55" />
                <path
                  d="M16 4.6v3.4M16 24v3.4M4.6 16H8M24 16h3.4"
                  strokeLinecap="round"
                />
                <circle
                  cx="16"
                  cy="16"
                  r="1.5"
                  fill="currentColor"
                  stroke="none"
                />
              </svg>
              <span className="btxt">
                <b>{t.brand}</b>
                <em>{t.brandSub}</em>
              </span>
            </div>
          </div>
          <nav className="tabs" aria-label={t.brand}>
            {(["pool", "album", "rates", "records"] as const).map((v) => (
              <button
                key={v}
                type="button"
                className={[
                  "tab",
                  view === v || (view === "reveal" && v === "pool") ? "on" : "",
                ]
                  .filter(Boolean)
                  .join(" ")}
                onClick={() => setView(v)}
              >
                {t[v]}
              </button>
            ))}
          </nav>
          <div className="hud-r">
            <a
              className="res"
              href="#"
              onClick={(e) => (e.preventDefault(), setModal("wallet"))}
            >
              <span className="res-ic">
                <Icon name="ticket" size={12} />
              </span>
              <b>{me ? me.ticketBalance : "—"}</b>
              <s>{t.tickets}</s>
            </a>
            <a
              className="res"
              href="#"
              onClick={(e) => (e.preventDefault(), setModal("wallet"))}
            >
              <span className="res-ic sky">
                <Icon name="shard" size={12} />
              </span>
              <b>{me ? me.shardBalance : "—"}</b>
              <s>{t.shards}</s>
            </a>
            <button
              className="icon-btn only-narrow"
              onClick={() => setModal("progress")}
              aria-label={t.myProgress}
            >
              <Icon name="gauge" size={14} />
            </button>
            <button
              className="icon-btn"
              onClick={() => setModal("wallet")}
              aria-label={t.ticketSource}
            >
              <Icon name="plus" size={14} />
            </button>
            <button
              className="icon-btn opt"
              onClick={() => setModal("rules")}
              aria-label={t.poolRules}
            >
              <Icon name="info" size={14} />
            </button>
          </div>
        </header>

        {/* ===== 卡池 ===== */}
        <div className="view v-pool">
          <aside className="rail rail-l">
            <div className="rail-h">
              <b>{t.pool}</b>
              <i>
                {banners.length} {t.periods}
              </i>
            </div>
            <div className="pool-list">
              {banners.map((b) => {
                const left = daysLeft(b.endsAt);
                const live = left === null || left > 0;
                return (
                  <button
                    key={b.id}
                    type="button"
                    className={`pool-item${b.id === bannerId ? " on" : ""}`}
                    onClick={() => setBannerId(b.id)}
                  >
                    <div className={`pi-art${live ? "" : " off"}`}>
                      <svg
                        viewBox="0 0 60 76"
                        fill="none"
                        stroke={live ? "var(--ssr)" : "var(--t2)"}
                        strokeWidth="1.3"
                        strokeLinecap="round"
                        aria-hidden="true"
                      >
                        <circle cx="30" cy="22" r="7.6" />
                        <circle
                          cx="30"
                          cy="22"
                          r="2.2"
                          fill={live ? "var(--ssr)" : "var(--t2)"}
                          stroke="none"
                        />
                        <path
                          d={j(
                            "M30 30.4V56M25 34h10",
                            "M26 40h8M27 46h6M28 52h4",
                          )}
                        />
                        <path
                          d="M30 8v3M30 33.5v2M14 22h4M42 22h4"
                          opacity=".55"
                        />
                      </svg>
                    </div>
                    <div className="pi-body">
                      <div className="pi-name">{b.name}</div>
                      <div className="pi-meta">
                        <span className={`pi-tag ${live ? "live" : "past"}`}>
                          {live ? t.live : t.past}
                        </span>
                        {left !== null
                          ? fill(t.daysLeft, { n: left })
                          : t.permanent}
                      </div>
                    </div>
                  </button>
                );
              })}
            </div>
            <div className="rail-foot">
              <a
                className="rail-link"
                href="#"
                onClick={(e) => (e.preventDefault(), setModal("rules"))}
              >
                {t.poolRules}
                <Icon name="arrow" size={12} />
              </a>
              <a
                className="rail-link"
                href="#"
                onClick={(e) => (e.preventDefault(), setModal("wallet"))}
              >
                {t.ticketSource}
                <Icon name="arrow" size={12} />
              </a>
            </div>
          </aside>

          <main className="arena">
            <svg
              className="starfield"
              viewBox="0 0 1200 760"
              preserveAspectRatio="xMidYMid slice"
              aria-hidden="true"
            >
              {SKY.stars.map((s) => (
                <circle
                  key={s.key}
                  cx={s.cx}
                  cy={s.cy}
                  r={s.r}
                  fill="var(--t1)"
                  opacity={s.o}
                />
              ))}
              {SKY.groups.map((pts, gi) => (
                <g key={gi}>
                  <path
                    d={pts
                      .map((p, i) => `${i ? "L" : "M"}${p[0]} ${p[1]}`)
                      .join("")}
                    fill="none"
                    stroke="var(--star)"
                    strokeWidth="0.9"
                    opacity="0.16"
                  />
                  {pts.map((p, i) => (
                    <circle
                      key={i}
                      cx={p[0]}
                      cy={p[1]}
                      r="1.8"
                      fill="var(--t1)"
                      opacity="0.45"
                    />
                  ))}
                </g>
              ))}
            </svg>
            <div className="beam" aria-hidden="true" />
            <div className="grain" aria-hidden="true" />
            <span className="arena-mark" aria-hidden="true">
              {String(
                Math.max(1, banners.findIndex((b) => b.id === bannerId) + 1),
              ).padStart(2, "0")}
            </span>

            <div className="hero">
              <div className="hero-l">
                <div className="stage-badges">
                  <span className="badge">
                    <Icon name="info" size={10} />
                    {banner?.kind === "limited" ? t.limited : t.standard}
                  </span>
                </div>

                <div className="arena-copy">
                  <span className="eyebrow">
                    ASTRAL ARCHIVE ·{" "}
                    {banner?.kind === "limited" ? "LIMITED" : "STANDARD"}
                  </span>
                  <h1 className="arena-title">{banner?.name ?? t.brand}</h1>
                  <p className="arena-sub">
                    {fill(t.albumSub, { n: litTotal })}
                  </p>
                  <div className="arena-meta">
                    <span className="meta-cell">
                      <Icon name="clock" size={12} />
                      {daysLeft(banner?.endsAt) !== null ? (
                        <b>
                          {fill(t.daysLeft, {
                            n: daysLeft(banner?.endsAt) ?? 0,
                          })}
                        </b>
                      ) : (
                        <b>{t.live}</b>
                      )}
                    </span>
                    {banner?.endsAt && (
                      <>
                        <span className="meta-dot" aria-hidden="true" />
                        <span className="meta-cell">
                          {fmtDay(banner.endsAt)}
                        </span>
                      </>
                    )}
                    <span className="meta-dot" aria-hidden="true" />
                    <a
                      className="rate-link"
                      href="#"
                      onClick={(e) => (e.preventDefault(), setView("rates"))}
                    >
                      概率详情
                      <Icon name="arrow" size={11} />
                    </a>
                  </div>
                </div>

                <div className="feat-cap">
                  <b>{t.featured}</b>
                  <span>{fill(t.litCount, { a: litCount, b: litTotal })}</span>
                </div>
                <div className="featured">
                  {(rates?.rows ?? [])
                    .filter((r) => r.type === "card")
                    .sort((a, b) => starsOf(b.r) - starsOf(a.r))
                    .filter(
                      (r, i, arr) =>
                        arr.findIndex((x) => starsOf(x.r) === starsOf(r.r)) ===
                        i,
                    )
                    .slice(0, 3)
                    .map((r) => {
                      const card = cards.find((c) => c.rarity === r.r);
                      return (
                        <div className="feat-card" key={r.r}>
                          <CardFace
                            name={card?.name ?? r.r}
                            rarity={r.r}
                            stars={starsOf(r.r)}
                            tag={undefined}
                          />
                          <span className="feat-lb">
                            {r.r} · {pct(r.comp)}
                          </span>
                        </div>
                      );
                    })}
                </div>
              </div>

              <div className="astro-wrap">
                <Astro pity={pity} soft={soft} hard={hard} />
                <div className="astro-core">
                  {targetCard ? (
                    <button
                      type="button"
                      className="astro-card-btn"
                      onClick={() => openCard(targetCard)}
                      aria-label={targetCard.name}
                    >
                      <CardFace
                        name={targetCard.name}
                        rarity={targetCard.rarity}
                        stars={starsOf(targetCard.rarity)}
                      />
                    </button>
                  ) : null}
                </div>
                <div className="astro-cap">
                  <span>
                    {t.hardProgress
                      .replace("{a}", String(pity))
                      .replace("{b}", String(hard))}
                  </span>
                  <span>{fill(t.toHard, { n: hardLeft })}</span>
                </div>
              </div>
            </div>

            <div className="arena-extra">
              <div className="extra-card">
                <div className="extra-h">
                  <b>{t.myProgress}</b>
                  <a
                    href="#"
                    onClick={(e) => (e.preventDefault(), setModal("progress"))}
                  >
                    {t.detail}
                    <Icon name="arrow" size={10} />
                  </a>
                </div>
                <div className="extra-prog">
                  <b>{hardLeft}</b>
                  <span>{t.pityUnit}</span>
                </div>
                <div className="bar" aria-hidden="true">
                  <i
                    style={{ width: `${Math.min(100, (pity / hard) * 100)}%` }}
                  />
                </div>
                <div className="bar-lb">
                  <span>
                    {t.hardProgress
                      .replace("{a}", String(pity))
                      .replace("{b}", String(hard))}
                  </span>
                  <span>{fill(t.toHard, { n: hardLeft })}</span>
                </div>
                <div className="extra-row">
                  <span>{t.litLabel}</span>
                  <b>
                    {litCount} / {litTotal}
                  </b>
                </div>
                <div className="bar sky" aria-hidden="true">
                  <i style={{ width: `${litPct}%` }} />
                </div>
              </div>
              <div className="extra-card extra-gain">
                <div className="extra-h">
                  <b>{t.seasonGain}</b>
                  <a
                    href="#"
                    onClick={(e) => (e.preventDefault(), setView("album"))}
                  >
                    {t.gainToAlbum}
                    <Icon name="arrow" size={10} />
                  </a>
                </div>
                {gainRows.length ? (
                  gainRows.map((r) => (
                    <div className="extra-row" key={r.seq}>
                      <span>
                        <i className={`dot ${rarityFam(r.r).toLowerCase()}`} />
                        {nameOfRow(r as DrawRow)}
                      </span>
                      <em>
                        {r.isNew ? "NEW · " : ""}
                        {ago(r.drawnAt)}
                      </em>
                    </div>
                  ))
                ) : (
                  <div className="extra-row">
                    <span>—</span>
                    <em>{t.albumEmpty}</em>
                  </div>
                )}
              </div>
            </div>
            <div className="arena-foot">
              <span>ASTRAL ARCHIVE</span>
              <span className="af-dot" />
              <span>{banner?.key ?? "SEASON"}</span>
            </div>
          </main>

          <aside className="rail rail-r">
            <div className="panel">
              <div className="panel-h">
                <b>{t.myProgress}</b>
                <a
                  href="#"
                  onClick={(e) => (e.preventDefault(), setView("records"))}
                >
                  {t.records}
                </a>
              </div>
              <div className="pity-big">
                <b>{hardLeft}</b>
                <span>{t.pityUnit}</span>
              </div>
              <p className="pity-hint">
                {t.hardProgress
                  .replace("{a}", String(pity))
                  .replace("{b}", String(hard))}
              </p>
              <div className="bar" aria-hidden="true">
                <i
                  style={{ width: `${Math.min(100, (pity / hard) * 100)}%` }}
                />
              </div>
              <div className="bar-lb">
                <span>{fill(t.toHard, { n: hardLeft })}</span>
                <span>{pct(rates?.hardProb ?? 0)}</span>
              </div>
              <div className="mine-rows">
                <div className="mine-row">
                  {t.litLabel}
                  <b>
                    {litCount} / {litTotal}
                  </b>
                </div>
                <DotMatrix lit={litCount} total={litTotal} />
              </div>
            </div>

            <div className="promo">
              <span className="promo-ic">
                <Icon name="gift" size={14} />
              </span>
              <div className="promo-tx">
                <b>{t.promoTitle}</b>
                <span>
                  {(() => {
                    const ten =
                      banner?.tenCost ?? (banner?.ticketCost ?? 25) * 10;
                    const save = (banner?.ticketCost ?? 25) * 10 - ten;
                    return save > 0
                      ? fill(t.promoSub, { n: ten, m: save })
                      : fill(t.promoFlat, { n: ten });
                  })()}
                </span>
              </div>
            </div>

            <div className="panel">
              <div className="panel-h">
                <b>{t.seasonGain}</b>
                <a
                  href="#"
                  onClick={(e) => (e.preventDefault(), setView("album"))}
                >
                  {t.gainToAlbum}
                </a>
              </div>
              {gainRows.length ? (
                gainRows.map((r) => (
                  <div className="drop-row" key={r.seq}>
                    <i
                      className={`drop-dot dot-${rarityFam(r.r).toLowerCase()}`}
                    />
                    <span className="drop-nm">{nameOfRow(r as DrawRow)}</span>
                    <span className="drop-tm">
                      {r.isNew ? "NEW · " : ""}
                      {ago(r.drawnAt)}
                    </span>
                  </div>
                ))
              ) : (
                <div className="drop-row">
                  <span className="drop-nm">{t.albumEmpty}</span>
                </div>
              )}
            </div>

            <div className="panel src-panel">
              <div className="panel-h src-h">
                <b>{t.ticketSource}</b>
              </div>
              <div className="src-body">
                <a
                  className="gain-row"
                  href="#"
                  onClick={(e) => (e.preventDefault(), setModal("wallet"))}
                >
                  <Icon name="task" size={13} />
                  {t.daily}
                  <span className="arr">+1</span>
                </a>
                <a
                  className="gain-row"
                  href="#"
                  onClick={(e) => (e.preventDefault(), setModal("wallet"))}
                >
                  <Icon name="bag" size={13} />
                  {t.shop}
                  <span className="arr">+1</span>
                </a>
                <a
                  className="gain-row"
                  href="#"
                  onClick={(e) => (e.preventDefault(), setModal("wallet"))}
                >
                  <Icon name="bank" size={13} />
                  {t.bank}
                  <span className="arr">+1</span>
                </a>
              </div>
            </div>
          </aside>

          <div className="dock">
            <div className="dock-l">
              <div className="dock-note">
                <Icon name="info" size={12} />
                {fill(t.dockNote, { n: hard })}
              </div>
              <button
                type="button"
                className={`switch${skip ? " on" : ""}`}
                onClick={() => setSkip((s) => !s)}
                aria-pressed={skip}
              >
                <span className="tg" aria-hidden="true" />
                <span>{t.skipOn}</span>
              </button>
            </div>
            <div className="dock-r">
              <button
                type="button"
                className="btn btn-ghost"
                onClick={() => void draw(1)}
                disabled={drawing}
              >
                <span>{t.single}</span>
                <span className="cost">
                  <Icon name="ticket" size={11} />
                  {banner?.ticketCost ?? 25}
                </span>
              </button>
              <button
                type="button"
                className="btn btn-gold"
                onClick={() => void draw(10)}
                disabled={drawing}
              >
                <span className="cost" style={{ marginRight: 6 }}>
                  <Icon name="cards" size={17} />
                </span>
                <span>{drawing ? t.drawing : t.ten}</span>
                <span className="cost">
                  <Icon name="ticket" size={11} />
                  {banner?.tenCost ?? (banner?.ticketCost ?? 25) * 10}
                </span>
              </button>
            </div>
          </div>
        </div>

        {/* ===== 演出 ===== */}
        <div className="view v-reveal">
          <div className="rev-top">
            <div className="rev-cap">
              <b>
                {rows?.length === 10 ? t.revealTen : t.revealOne} ·{" "}
                {banner?.name ?? ""}
              </b>
              <span>
                {fill(t.costText, {
                  n:
                    rows?.length === 10
                      ? (banner?.tenCost ?? (banner?.ticketCost ?? 25) * 10)
                      : (banner?.ticketCost ?? 25),
                })}
              </span>
            </div>
            <div className="rev-bal">
              <span className="hide-sm">
                {t.restTickets} <b>{me?.ticketBalance ?? "—"}</b>
              </span>
              <span className="hide-sm">
                {t.shards} <b>{me?.shardBalance ?? "—"}</b>
              </span>
              <button
                type="button"
                className="skip-btn"
                onClick={() => setFlipped(true)}
              >
                {t.skip}
              </button>
            </div>
          </div>
          <div className="pull-wrap">
            <div
              className={`pull-grid${rows?.length === 1 ? " one" : ""}`}
              aria-live="polite"
            >
              {(rows ?? []).map((r, i) => {
                const fam = rarityFam(r.r);
                const name = nameOfRow(r);
                const isCard = r.type === "card";
                const rare = fam === "SSR";
                const rare2 = fam === "SR";
                return (
                  <div
                    key={r.seq ?? i}
                    className={[
                      "pull-cell",
                      rare ? "rare" : rare2 ? "rare-sr" : "",
                      flipped ? "flipped" : "",
                    ]
                      .filter(Boolean)
                      .join(" ")}
                    data-r={fam}
                    style={{
                      transitionDelay:
                        flipped && !skip ? `${i * 62}ms` : undefined,
                    }}
                  >
                    {r.wasPity && (
                      <Badge tone="warn" className="pity-flag">
                        {t.hardPity}
                      </Badge>
                    )}
                    <div className="rays" aria-hidden="true" />
                    <div className="pull-inner">
                      <div className="back">
                        <div className="card card-back" data-r="SSR">
                          <div className="card-art">
                            <svg
                              viewBox="0 0 100 100"
                              fill="none"
                              stroke="currentColor"
                              strokeWidth="1.4"
                              strokeLinecap="round"
                              aria-hidden="true"
                              dangerouslySetInnerHTML={{
                                __html: SIGS.astrolabe,
                              }}
                            />
                          </div>
                        </div>
                      </div>
                      <div className="face">
                        {isCard ? (
                          <CardFace
                            name={name}
                            rarity={r.r ?? "N"}
                            stars={starsOf(r.r)}
                            tag={r.isNew ? "NEW" : undefined}
                          />
                        ) : (
                          <div className="card pull-miss" data-r={fam}>
                            <span className="card-gem" aria-hidden="true" />
                            <div className="card-art">
                              <svg
                                viewBox="0 0 100 100"
                                fill="none"
                                stroke="currentColor"
                                strokeWidth="1.5"
                                strokeLinecap="round"
                                aria-hidden="true"
                                dangerouslySetInnerHTML={{ __html: SIGS.crest }}
                              />
                            </div>
                            <div className="card-plate">
                              <span className="card-name">
                                {r.type === "shard"
                                  ? `+${r.shards} ${t.shards}`
                                  : "—"}
                              </span>
                              <div className="card-sub">
                                <span
                                  className="card-stars"
                                  aria-hidden="true"
                                />
                                <span className="card-lv">{t.shards}</span>
                              </div>
                            </div>
                          </div>
                        )}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
            <div className="rev-sum">
              <div className="sum-left">
                <div className="sum-line">
                  <b>{t.gained}</b>
                  {(() => {
                    const pc: Record<string, number> = {};
                    let shardGain = 0;
                    for (const r of rows ?? []) {
                      if (r.type === "card" && r.r)
                        pc[r.r] = (pc[r.r] ?? 0) + 1;
                      if (r.type === "shard") shardGain += r.shards ?? 0;
                    }
                    return (
                      <>
                        {(["SSR", "SR", "R", "N"] as const).map((k) =>
                          pc[k] ? (
                            <span className="rarity-pill" key={k}>
                              <i
                                style={{
                                  background: rarVar(k),
                                }}
                              />
                              {k} ×{pc[k]}
                            </span>
                          ) : null,
                        )}
                        {shardGain > 0 && (
                          <span className="rarity-pill">
                            <i style={{ background: "var(--star)" }} />
                            {t.shards} +{shardGain}
                          </span>
                        )}
                        {!Object.keys(pc).length && shardGain === 0 && (
                          <span className="muted">—</span>
                        )}
                      </>
                    );
                  })()}
                </div>
                <div
                  className="sum-line"
                  style={{ color: "var(--t3)", fontSize: 11 }}
                >
                  {(rows ?? []).filter(
                    (r) => r.type === "card" && r.r === "SSR",
                  ).length
                    ? fill(t.resetPity, {
                        n: (rows ?? []).filter(
                          (r) => r.type === "card" && r.r === "SSR",
                        ).length,
                      })
                    : fill(t.pityPlus, { n: rows?.length ?? 0, m: hardLeft })}
                </div>
              </div>
              <div className="sum-right">
                <button
                  type="button"
                  className="btn btn-ghost"
                  onClick={() => setView("album")}
                >
                  {t.viewAlbum}
                </button>
                <button
                  type="button"
                  className="btn btn-gold"
                  onClick={() => void draw(10)}
                >
                  <span>{t.again}</span>
                  <span className="cost">
                    <Icon name="ticket" size={11} />
                    {banner?.tenCost ?? (banner?.ticketCost ?? 25) * 10}
                  </span>
                </button>
              </div>
            </div>
          </div>
        </div>

        {/* ===== 卡册 ===== */}
        <div className="view v-album">
          <div className="alb-head">
            <div className="alb-t">
              <em>
                Collection ·{" "}
                {banner?.kind === "limited" ? "LIMITED" : "STANDARD"}
              </em>
              <h2>{t.album}</h2>
              <p>{fill(t.albumSub, { n: litTotal })}</p>
            </div>
            <div className="alb-stats">
              <div className="alb-stat">
                <b>{litCount}</b>
                <span>{t.litLabel}</span>
              </div>
              <div className="alb-stat">
                <b>{litTotal}</b>
                <span>{t.slots}</span>
              </div>
              <div className="alb-stat">
                <b>{shards}</b>
                <span>{t.shards}</span>
              </div>
            </div>
          </div>

          {targetCard ? (
            <div className="alb-synth">
              <div className="synth-face">
                <Icon name="cards" size={16} />
              </div>
              <div className="synth-tx">
                <b>
                  {t.synthTarget} · {targetCard.rarity} {targetCard.name}
                </b>
                <div className="bar gold" aria-hidden="true">
                  <i style={{ width: `${synthPct}%` }} />
                </div>
                <div className="bar-lb">
                  <span>
                    {shards} / {targetSynth} {t.shards}
                  </span>
                  <span>
                    {targetSynth > shards
                      ? `+${targetSynth - shards}`
                      : t.synth}
                  </span>
                </div>
              </div>
              <button
                type="button"
                className="mini-btn gold"
                disabled={
                  !!ownedById.get(targetCard.id)?.exchanged ||
                  shards < targetSynth
                }
                onClick={() => {
                  setConfirm({ card: targetCard, cost: targetSynth });
                  setModal("confirm");
                }}
              >
                {t.synth}
              </button>
            </div>
          ) : null}

          <div className="alb-bar">
            {chips.map((c) => (
              <button
                key={c.key}
                type="button"
                className={`chip${filter === c.key ? " on" : ""}`}
                onClick={() => setFilter(c.key)}
              >
                {c.label} <i>{c.n}</i>
              </button>
            ))}
            <label className="search">
              <Icon name="search" size={12} />
              <input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder={t.search}
                aria-label={t.search}
              />
            </label>
          </div>

          <div className="alb-grid">
            {filteredAlbum.map(({ card, held: h, lv, lvMax, locked }) => (
              <div className={`acard${locked ? " locked" : ""}`} key={card.id}>
                <button
                  type="button"
                  className="acard-hit"
                  onClick={() => openCard(card)}
                  aria-label={locked ? t.locked : card.name}
                >
                  <CardFace
                    name={card.name}
                    rarity={card.rarity}
                    stars={starsOf(card.rarity)}
                    locked={locked}
                    dim={!!ownedById.get(card.id)?.exchanged}
                  />
                </button>
                <div className="acard-meta">
                  <b>{locked ? t.locked : card.name}</b>
                  <span>
                    {locked
                      ? card.rarity
                      : fill(t.held, { n: h, lv, max: lvMax })}
                  </span>
                </div>
                {locked ? (
                  <div className="acard-acts">
                    <span className="acard-hint">{t.unlockHint}</span>
                  </div>
                ) : (
                  <div className="acard-acts">
                    <button
                      type="button"
                      className="mini-btn"
                      disabled={busyCard === card.id}
                      onClick={() => void act("dismantle", card)}
                    >
                      {t.dismantle}
                    </button>
                    <button
                      type="button"
                      className="mini-btn gold"
                      disabled={
                        busyCard === card.id ||
                        !!ownedById.get(card.id)?.exchanged ||
                        shards < card.synthShards
                      }
                      onClick={() => {
                        setConfirm({ card, cost: card.synthShards });
                        setModal("confirm");
                      }}
                    >
                      {ownedById.get(card.id)?.exchanged
                        ? t.exchangedTag
                        : t.exchange}
                    </button>
                    <button
                      type="button"
                      className="mini-btn"
                      disabled={busyCard === card.id || lv >= lvMax}
                      onClick={() => void act("levelup", card)}
                    >
                      {lv >= lvMax ? t.maxed : t.levelup}
                    </button>
                  </div>
                )}
              </div>
            ))}
          </div>
          <div className="alb-more">
            <Icon name="chev" size={11} />
            {filteredAlbum.length} / {albumList.length}
          </div>
        </div>

        {/* ===== 概率 ===== */}
        <div className="view v-rates">
          <div className="alb-head" style={{ marginBottom: 18 }}>
            <div className="alb-t">
              <em>Disclosure</em>
              <h2>{t.rates}</h2>
              <p>{t.ratesSub.replace("{id}", String(banner?.id ?? 0))}</p>
            </div>
            <div className="alb-stats">
              <div className="alb-stat">
                <b>{hard}</b>
                <span>{t.hardCap}</span>
              </div>
              <div className="alb-stat">
                <b>{soft}</b>
                <span>{t.softStart}</span>
              </div>
            </div>
          </div>

          <div className="rate-ladder">
            <div className="rl-head">
              <b>{t.ladder}</b>
              <span>{fill(t.ladderNow, { n: pity, m: hardLeft })}</span>
            </div>
            <div className="rl-track" aria-hidden="true">
              <span
                className="rl-fill"
                style={{ width: `${Math.min(100, (pity / hard) * 100)}%` }}
              />
              <span
                className="rl-now"
                style={{ left: `${Math.min(98, (pity / hard) * 100)}%` }}
              >
                {pity}
              </span>
            </div>
            <div className="rl-axis">
              <span>0</span>
              <span>
                {soft} · {t.softStart}
              </span>
              <span>
                {hard} · {t.hardCap}
              </span>
            </div>
          </div>
          <div className="rate-wrap">
            <div className="rate-main">
              <div className="rate-head">
                <h3>
                  {banner?.name ?? ""} · {t.dropTitle}
                </h3>
                <span>
                  {ratesLoading
                    ? t.loadFailed
                    : `${litTotal} 张 · 单抽 ${
                        banner?.ticketCost ?? 25
                      } ${t.tickets}`}
                </span>
              </div>
              <table className="rate-table">
                <colgroup>
                  <col style={{ width: "16%" }} />
                  <col style={{ width: "14%" }} />
                  <col style={{ width: "34%" }} />
                  <col style={{ width: "18%" }} />
                  <col style={{ width: "18%" }} />
                </colgroup>
                <thead>
                  <tr>
                    <th>{t.thRarity}</th>
                    <th>{t.thStars}</th>
                    <th>{t.thComp}</th>
                    <th>{t.thBase}</th>
                    <th>{t.thShards}</th>
                  </tr>
                </thead>
                <tbody>
                  {(rates?.rows ?? [])
                    .filter((r) => r.type === "card")
                    .map((r) => (
                      <tr key={r.r}>
                        <td>
                          <b
                            style={{
                              color: rarVar(r.r),
                            }}
                          >
                            {r.r}
                          </b>
                        </td>
                        <td className="num">{starsOf(r.r)} ★</td>
                        <td>
                          <div className="num">{pct(r.comp)}</div>
                          <div className="rate-bar">
                            <i
                              style={{
                                width: `${Math.min(100, r.comp * 100)}%`,
                                background: rarVar(r.r),
                              }}
                            />
                          </div>
                        </td>
                        <td className="num">{pct(r.base)}</td>
                        <td className="num">{dupeOf(r.r) || "—"}</td>
                      </tr>
                    ))}
                  {(rates?.rows ?? [])
                    .filter((r) => r.type !== "card")
                    .map((r) => (
                      <tr key={`x-${r.r}`}>
                        <td>
                          <b style={{ color: "var(--t3)" }}>{r.r}</b>
                        </td>
                        <td className="num">—</td>
                        <td>
                          <div className="num">{pct(r.comp)}</div>
                        </td>
                        <td className="num">{pct(r.base)}</td>
                        <td className="num">{r.shards || "—"}</td>
                      </tr>
                    ))}
                </tbody>
              </table>
              <div className="rate-note">
                <p className="note-line">{t.rateNote}</p>
                <button
                  type="button"
                  className="mini-btn"
                  onClick={() => setModal("rateSpec")}
                >
                  {t.specBtn}
                  <Icon name="arrow" size={11} />
                </button>
              </div>
            </div>
            <div className="rate-side">
              <div className="rule-card">
                <h4>{t.poolRules}</h4>
                {ruleSteps}
              </div>
              <div className="rule-card">
                <h4>{t.exchangeRule}</h4>
                {exchangeSteps}
              </div>
            </div>
          </div>

          <div className="rate-foot">
            <span>GET /api/v1/gacha/banner/{banner?.id ?? 0}/rates</span>
            <span>{rates?.banner.key ?? ""}</span>
          </div>
        </div>

        {/* ===== 记录 ===== */}
        <div className="view v-records">
          <div className="alb-head" style={{ marginBottom: 16 }}>
            <div className="alb-t">
              <em>Ledger</em>
              <h2>{t.records}</h2>
              <p>{t.recordsSub}</p>
            </div>
            <button
              type="button"
              className="mini-btn"
              onClick={() => setModal("export")}
            >
              {t.exportBtn}
            </button>
          </div>
          <div className="rec-tiles">
            <div className="tile">
              <span>{t.totalDraws}</span>
              <b>{recent.length}</b>
              <i>{recent.length} / 30</i>
            </div>
            <div className="tile">
              <span>{t.ssrCount}</span>
              <b style={{ color: "var(--ssr)" }}>{ssrRows.length}</b>
              <i>{recent.length ? pct(ssrRows.length / recent.length) : "—"}</i>
            </div>
            <div className="tile">
              <span>{t.avgDraws}</span>
              <b>{avg}</b>
              <i>{t.thisPage}</i>
            </div>
            <div className="tile">
              <span>{t.shardTotal}</span>
              <b>{recent.reduce((a, r) => a + (r.shards || 0), 0)}</b>
              <i>
                {shards} {t.shards}
              </i>
            </div>
          </div>
          <div className="strip-cap">
            <b>{t.stripCap}</b>
            <span>{t.recordsFoot}</span>
          </div>
          <div className="strip" aria-hidden="true">
            {recent
              .slice()
              .reverse()
              .map((r) => (
                <span
                  key={r.seq}
                  className={`strip-cell${r.wasPity ? " pity" : ""}`}
                  data-r={rarityFam(r.r)}
                />
              ))}
          </div>
          <div className="rec-table">
            <table>
              <colgroup>
                <col style={{ width: "24%" }} />
                <col style={{ width: "40%" }} />
                <col style={{ width: "18%" }} />
                <col style={{ width: "18%" }} />
              </colgroup>
              <thead>
                <tr>
                  <th>{t.thTime}</th>
                  <th>{t.thResult}</th>
                  <th>{t.thPity}</th>
                  <th>{t.thBalance}</th>
                </tr>
              </thead>
              <tbody>
                {recent.map((r) => (
                  <tr key={r.seq}>
                    <td className="num">{fmtTime(r.drawnAt)}</td>
                    <td>
                      {r.type === "card" ? (
                        <b
                          style={{
                            color: `var(--${rarityFam(r.r).toLowerCase()})`,
                          }}
                        >
                          {nameOfRow(r as DrawRow)}
                        </b>
                      ) : r.type === "shard" ? (
                        `${t.shards} +${r.shards}`
                      ) : (
                        "—"
                      )}
                    </td>
                    <td>
                      {r.wasPity ? (
                        <b style={{ color: "var(--gold-l)" }}>{t.hardPity}</b>
                      ) : r.r && starsOf(r.r) >= 4 && (r.pityAt ?? 0) > soft ? (
                        <b style={{ color: "var(--ur)" }}>{t.softPity}</b>
                      ) : (
                        <span className="muted">{t.natural}</span>
                      )}
                    </td>
                    <td className="num">{r.pityAt ?? "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <div className="rec-foot">
            <span className="muted">{t.recordsFoot}</span>
          </div>
        </div>

        {/* 提示条（区内 toast，不遮挡按钮） */}
        {(note || unknownKey) && (
          <div className="gx-note" role="status">
            {unknownKey ? t.loadFailed : note}
            {unknownKey && (
              <button
                type="button"
                className="gx-note-retry"
                onClick={retryUnknown}
              >
                {t.retry}
              </button>
            )}
            <button
              type="button"
              className="gx-note-x"
              onClick={() => {
                setNote(null);
                setUnknownKey(null);
              }}
              aria-label={t.closeAria}
            >
              ×
            </button>
          </div>
        )}

        {/* ===== 弹窗内容 ===== */}
        <Modal
          open={modal === "rules"}
          kind="rules"
          title={modalTitle.rules}
          onClose={() => setModal(null)}
          closeLabel={t.closeAria}
          closeText={t.close}
        >
          <div className="rule-card">
            <h4>{t.poolRules}</h4>
            {ruleSteps}
          </div>
          <div className="rule-card">
            <h4>{t.exchangeRule}</h4>
            {exchangeSteps}
          </div>
          <p className="modal-note">
            规则随卡池切换生效；已抽结果与保底计数不回改。
          </p>
        </Modal>

        <Modal
          open={modal === "wallet"}
          kind="wallet"
          title={modalTitle.wallet}
          onClose={() => setModal(null)}
          closeLabel={t.closeAria}
          closeText={t.close}
        >
          <div className="rule-card">
            <h4>{t.tickets}</h4>
            <RuleStep
              no="01"
              b={t.daily}
              span="完成 3 项当日任务 +1 券，每日 04:00 刷新。"
            />
            <RuleStep
              no="02"
              b={t.shop}
              span="1200 魔力兑换 1 券，每周限 5 次。"
            />
            <RuleStep
              no="03"
              b={t.bank}
              span="每 5000 魔力利息结算 1 券，上限 10 券 / 月。"
            />
          </div>
          <div className="rule-card">
            <h4>{t.shards}</h4>
            <RuleStep
              no="01"
              b="重复卡分解"
              span="抽出已持有卡自动折算碎片，无需手动操作。"
            />
            <RuleStep
              no="02"
              b={`${targetSynth || 520} 碎片兑换当期 ${
                targetCard?.rarity ?? ""
              }`.trimEnd()}
              span="每季每张限兑 1 次。"
            />
          </div>
        </Modal>

        <Modal
          open={modal === "progress"}
          kind="progress"
          title={modalTitle.progress}
          onClose={() => setModal(null)}
          closeLabel={t.closeAria}
          closeText={t.close}
        >
          <div className="m-prog">
            <b>{hardLeft}</b>
            <span>{t.pityUnit}</span>
            <i>
              {t.hardProgress
                .replace("{a}", String(pity))
                .replace("{b}", String(hard))}
            </i>
          </div>
          <div className="bar" aria-hidden="true">
            <i style={{ width: `${Math.min(100, (pity / hard) * 100)}%` }} />
          </div>
          <div className="bar-lb">
            <span>
              {t.hardProgress
                .replace("{a}", String(pity))
                .replace("{b}", String(hard))}
            </span>
            <span>{fill(t.toHard, { n: hardLeft })}</span>
          </div>
          <div className="m-split">
            <b>
              图鉴点亮{" "}
              <b>
                {litCount} / {litTotal}
              </b>
            </b>
            <DotMatrix lit={litCount} total={litTotal} />
          </div>
          <div className="rule-card">
            <h4>{t.seasonGain}</h4>
            {gainRows.length ? (
              gainRows.map((r, i) => (
                <RuleStep
                  key={r.seq}
                  no={String(i + 1).padStart(2, "0")}
                  b={`${nameOfRow(r as DrawRow)}${r.isNew ? " · NEW" : ""}`}
                  span={
                    `${r.r ?? ""} · ${ago(r.drawnAt)}` +
                    (r.wasPity ? " · 硬保底触发" : "")
                  }
                />
              ))
            ) : (
              <p className="modal-note">{t.albumEmpty}</p>
            )}
          </div>
        </Modal>

        <Modal
          open={modal === "rateSpec"}
          kind="rateSpec"
          title={modalTitle.rateSpec}
          onClose={() => setModal(null)}
          closeLabel={t.closeAria}
          closeText={t.close}
        >
          <div className="spec-card">
            <div className="spec-hd">
              <h4>{t.modalSpec}</h4>
              <span>
                <i />
                {rates?.banner.key ?? ""}
              </span>
            </div>
            <div className="spec-row">
              <b>{t.thBase}</b>
              <span>单抽独立概率，未含任何保底补正</span>
            </div>
            <div className="spec-row">
              <b>{t.thComp}</b>
              <span>含软保底爬升与硬保底的长期期望值</span>
            </div>
            <div className="spec-row">
              <b>周期</b>
              <span>
                {rates ? rates.cycle : "—"} · 硬保底单抽命中{" "}
                {pct(rates?.hardProb ?? 0)}
              </span>
            </div>
            <div className="spec-row">
              <b>复核接口</b>
              <span>
                <code>GET /api/v1/gacha/banner/{banner?.id ?? 0}/rates</code> ·
                只读公开
              </span>
            </div>
          </div>
          <div className="spec-card">
            <div className="spec-hd">
              <h4>实测对照</h4>
              <span>{recent.length} 抽样本</span>
            </div>
            <div className="spec-row">
              <b>SSR 出货</b>
              <span>
                {ssrRows.length} 张 · 实测{" "}
                {recent.length ? pct(ssrRows.length / recent.length) : "—"}
                （综合 {pct(rates?.rows.find((r) => r.r === "SSR")?.comp ?? 0)}
                ）
              </span>
            </div>
            <div className="spec-row">
              <b>保底触发</b>
              <span>
                {recent.filter((r) => r.wasPity).length} 次 · 平均 {avg} 抽
              </span>
            </div>
          </div>
          <div className="spec-card">
            <div className="spec-hd">
              <h4>公示承诺</h4>
              <span>只读留痕</span>
            </div>
            <div className="spec-row">
              <b>同源</b>
              <span>前端口径与 GET /rates 逐项一致，不额外推算</span>
            </div>
            <div className="spec-row">
              <b>不改写</b>
              <span>概率调整仅在卡池切换时生效，记录不可回溯</span>
            </div>
          </div>
        </Modal>

        <Modal
          open={modal === "export"}
          kind="export"
          title={modalTitle.export}
          onClose={() => setModal(null)}
          closeLabel={t.closeAria}
          closeText={t.close}
          footer={
            <>
              <button
                type="button"
                className="btn btn-ghost ft-know"
                onClick={() => setModal(null)}
              >
                {t.close}
              </button>
              <button
                type="button"
                className="btn btn-gold ft-go"
                onClick={() => void copyCsv()}
              >
                {t.copyCsv}
              </button>
            </>
          }
        >
          <p className="modal-note">
            导出近 30 抽原始行，字段与 <code>POST /gacha/draw</code> 的{" "}
            <code>rows[]</code> 一致，不做二次加工。
          </p>
          <div className="spec-card">
            <div className="spec-row">
              <b>seq</b>
              <span>本池内抽序号，自 1 递增</span>
            </div>
            <div className="spec-row">
              <b>type / r</b>
              <span>档位与稀有度，取自 rarities 表</span>
            </div>
            <div className="spec-row">
              <b>wasPity / isNew</b>
              <span>是否保底触发 / 是否首次点亮</span>
            </div>
            <div className="spec-row">
              <b>shards</b>
              <span>本次结算碎片，重复卡才有值</span>
            </div>
          </div>
        </Modal>

        <Modal
          open={modal === "confirm"}
          kind="confirm"
          title={modalTitle.confirm}
          onClose={() => setModal(null)}
          closeLabel={t.closeAria}
          closeText={t.close}
          footer={
            <>
              <button
                type="button"
                className="btn btn-ghost ft-know"
                onClick={() => setModal(null)}
              >
                {t.cancel}
              </button>
              <button
                type="button"
                className="btn btn-gold ft-go"
                disabled={!!busyCard}
                onClick={() => {
                  if (!confirm) return;
                  void act("exchange", confirm.card);
                  setModal(null);
                }}
              >
                {t.confirmExchange}
              </button>
            </>
          }
        >
          <p className="modal-note">
            兑换后本季该卡不可再兑，碎片立即扣除且不可退回。
          </p>
          <div className="spec-card">
            <div className="spec-row">
              <b>目标</b>
              <span>
                {confirm
                  ? `${confirm.card.rarity} · ${confirm.card.name}`
                  : "—"}
              </span>
            </div>
            <div className="spec-row">
              <b>消耗</b>
              <span>
                {confirm?.cost ?? 0} {t.shards}（当前 {shards}）
              </span>
            </div>
            <div className="spec-row">
              <b>结果</b>
              <span>图鉴点亮 +1 · 卡面标记「本季已兑换」</span>
            </div>
          </div>
        </Modal>

        <Modal
          open={modal === "card"}
          kind="card"
          title={modalTitle.card}
          onClose={() => setModal(null)}
          closeLabel={t.closeAria}
          closeText={t.close}
          footer={
            modalCard ? (
              <>
                <button
                  type="button"
                  className="btn btn-ghost ft-know"
                  disabled={
                    !ownedById.get(modalCard.id) || busyCard === modalCard.id
                  }
                  onClick={() => void act("dismantle", modalCard)}
                >
                  {t.dismantle}
                </button>
                <button
                  type="button"
                  className="btn btn-ghost ft-know"
                  disabled={
                    !ownedById.get(modalCard.id) ||
                    busyCard === modalCard.id ||
                    (ownedById.get(modalCard.id)?.lv ?? 0) >=
                      (ownedById.get(modalCard.id)?.lvMax ?? 1)
                  }
                  onClick={() => void act("levelup", modalCard)}
                >
                  {t.levelup}
                </button>
                <button
                  type="button"
                  className="btn btn-gold ft-go"
                  disabled={
                    !ownedById.get(modalCard.id) ||
                    !!ownedById.get(modalCard.id)?.exchanged ||
                    shards < modalCard.synthShards
                  }
                  onClick={() => {
                    setConfirm({
                      card: modalCard,
                      cost: modalCard.synthShards,
                    });
                    setModal("confirm");
                  }}
                >
                  {t.exchange}
                </button>
              </>
            ) : null
          }
        >
          {modalCard && (
            <>
              <div className="m-card">
                <CardFace
                  name={modalCard.name}
                  rarity={modalCard.rarity}
                  stars={starsOf(modalCard.rarity)}
                  tag={
                    ownedById.get(modalCard.id)?.exchanged
                      ? t.exchangedTag
                      : undefined
                  }
                />
              </div>
              <div className="spec-card">
                <div className="spec-row">
                  <b>档位</b>
                  <span>
                    {modalCard.rarity} · {starsOf(modalCard.rarity)} 星
                  </span>
                </div>
                <div className="spec-row">
                  <b>持有</b>
                  <span>
                    ×{ownedById.get(modalCard.id)?.held ?? 0} · Lv.
                    {ownedById.get(modalCard.id)?.lv ?? 0}/
                    {ownedById.get(modalCard.id)?.lvMax ?? modalCard.lvMax}
                  </span>
                </div>
                <div className="spec-row">
                  <b>分解</b>
                  <span>重复卡可拆 +{modalCard.dupeShards} 碎片</span>
                </div>
                <div className="spec-row">
                  <b>兑换</b>
                  <span>
                    {modalCard.synthShards} 碎片 ·{" "}
                    {ownedById.get(modalCard.id)?.exchanged
                      ? "本季已兑换"
                      : "本季可兑 1 次"}
                  </span>
                </div>
                <div className="spec-row">
                  <b>来源</b>
                  <span>{banner?.name ?? ""}</span>
                </div>
              </div>
            </>
          )}
        </Modal>
      </div>
    </section>
  );
}
