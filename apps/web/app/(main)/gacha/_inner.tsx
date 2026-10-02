import { getDict } from "@/i18n/server";
import { api } from "@/lib/api-client";
import { GachaRates } from "@/components/gacha-rates";
import { GachaAlbum } from "@/components/gacha-album";
import { GachaTabbed } from "@/components/gacha-tabbed";
import { ArcadeGlyph } from "@/components/arcade/arcade-glyph";

export const dynamic = "force-dynamic";

/** 后端公示形状（gacha_http::rates 三端点的信封 data）。 */
export interface GachaRarityRow {
  key: string;
  label: string;
  sort: number;
  stars: number;
  goldRank: number;
  frame: string | null;
  frameHi: string | null;
  glow: string | null;
  bg1: string | null;
  bg2: string | null;
  ink: string | null;
  foil: number;
  foilMask: string;
}

export interface GachaBannerRates {
  banner: {
    id: number;
    key: string;
    name: string;
    kind: string;
    ticketCost: number;
    pitySoft: number;
    pityHard: number;
    pityRamp: number;
  };
  rows: Array<{
    r: string;
    type: string;
    base: number;
    comp: number;
    value: number;
    shards: number;
    /** 卡档的合成需求（gacha_cards.synth_shards，非卡档为 0） */
    synth: number;
  }>;
  goldBase: number;
  goldComp: number;
  cycle: number;
  hardProb: number;
  sv: number;
  ratioNew: number;
  ratioEnd: number;
  ratioWorst: number;
}

/**
 * G31-A 公示页（方案 §5）：概率**双列**（表定 base / 含保底综合 comp）——
 * 方案红线「只公示表定会放行实际倒灌经济的池子」。匿名可读；数据一次取齐。
 */
export default async function GachaDisclosurePage() {
  const { dict } = await getDict();
  const [rarities, banners] = await Promise.all([
    api
      .get<GachaRarityRow[]>("/api/v1/gacha/rarities")
      .catch(() => [] as GachaRarityRow[]),
    api.get<unknown[]>("/api/v1/gacha/banners").catch(() => [] as unknown[]),
  ]);
  const ids = Array.isArray(banners)
    ? banners
        .map((b) =>
          typeof b === "object" && b !== null && "id" in b
            ? Number((b as { id: unknown }).id)
            : NaN,
        )
        .filter((n) => Number.isInteger(n))
    : [];
  const rates = await Promise.all(
    ids.map((id) =>
      api
        .get<GachaBannerRates>(`/api/v1/gacha/banner/${id}/rates`)
        .catch(() => null),
    ),
  );
  // 收藏面载荷提前取：卡册合成进度条与 GachaAlbum 共用这份数据
  const me = await api
    .get<import("@/components/gacha-album").MePayload>("/api/v1/gacha/me")
    .catch(() => null);
  // 星轨卡册卡位（样图⑧）：本期卡池前 3 档卡面（按稀有度星数降序）
  const first = rates.find((r): r is GachaBannerRates => r !== null);
  // 卡名公示：rates 行不带卡名，用 /gacha/cards 按「档位 → 池内该档卡」
  // 对上真名（每档取 sort 最前一张； rates 暴露 cardId 后可直接精确到卡）
  const cardNames = await api
    .get<{ id: number; name: string; rarity: string }[]>("/api/v1/gacha/cards")
    .catch(() => [] as { id: number; name: string; rarity: string }[]);
  const bannerName =
    banners.find(
      (b): b is { id: number; name: string } =>
        typeof b === "object" &&
        b !== null &&
        "id" in b &&
        Number((b as { id: unknown }).id) === first?.banner.id,
    )?.name ?? first?.banner.key;
  // 合成目标（样图⑧「当期 SSR」）：优先 SSR 档的 synth_shards；池里没有
  // SSR 时退星数最高卡档。不是写死 20 —— 需求由卡定义下发（样例池 = 520）
  const cardRows = (first?.rows ?? []).filter((row) => row.type === "card");
  const starsOf = (r: string) => rarities.find((x) => x.key === r)?.stars ?? 0;
  const ssrSynth =
    (
      cardRows.find((row) => row.r === "SSR") ??
      [...cardRows].sort((a, b) => starsOf(b.r) - starsOf(a.r))[0]
    )?.synth ?? 0;
  const topCards = (() => {
    if (!first) return [];
    const byR = new Map(rarities.map((r) => [r.key, r]));
    // 每档代表卡：按星数降序**去重档位**取前 3（高低档并列呈现，
    // 蓝卡位不会被前 3 全挤成金 —— 样图⑧ 的三卡即三个不同档）
    const rows = cardRows
      .sort((a, b) => (byR.get(b.r)?.stars ?? 0) - (byR.get(a.r)?.stars ?? 0))
      .filter(
        (row, i, arr) =>
          arr.findIndex(
            (x) => byR.get(x.r)?.stars === byR.get(row.r)?.stars,
          ) === i,
      )
      .slice(0, 3);
    return rows.map((row) => {
      const meta = byR.get(row.r);
      const card = cardNames.find((c) => c.rarity === row.r);
      return {
        name: card?.name ?? meta?.label ?? row.r,
        sub: row.r.toUpperCase(),
        stars: meta?.stars ?? 1,
        tone: (meta?.stars ?? 1) >= 5 ? "gold" : "sky",
      };
    });
  })();

  return (
    <div className="flex flex-col gap-4">
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">Gacha</div>
          <h1 className="font-display text-2xl">{dict.nav.gachaDisclosure}</h1>
        </div>
      </div>
      {/* 样图⑧三 Tab：卡池 / 概率 / 我的卡册（RSC 预渲染，客户端只做显隐） */}
      <GachaTabbed
        labels={{
          pool: dict.games.gachaTabPool,
          rates: dict.games.gachaTabRates,
          album: dict.games.gachaTabAlbum,
        }}
        rates={
          <GachaRates
            rarities={rarities}
            banners={rates.filter((r): r is GachaBannerRates => r !== null)}
          />
        }
        album={<GachaAlbum rarities={rarities} initial={me} />}
        pool={
          <>
            {/* 新手首抽体验价：未抽过卡的用户单抽 1 券（后端放行） */}
            {me && (me.cards?.length ?? 0) === 0 && (
              <p className="rounded-[var(--r-md)] bg-mint-soft px-3 py-2 text-xs font-bold text-ink">
                {dict.games.gachaFirstDraw}
              </p>
            )}
            {first && (
              <section className="sw-cardbook">
                <div className="sw-cardbook-cap">
                  {dict.games.gachaPool} · {bannerName}
                </div>
                <div className="sw-cardbook-grid">
                  {topCards.map((c) => (
                    <div key={c.name} className={`sw-card sw-tone-${c.tone}`}>
                      <span className="sw-card-stars num" aria-hidden>
                        {"★".repeat(c.stars)}
                      </span>
                      <span className="sw-card-face" aria-hidden>
                        <ArcadeGlyph k="gacha" />
                      </span>
                      <span className="sw-card-name">{c.name}</span>
                      <span className="sw-card-rank num">{c.sub}</span>
                    </div>
                  ))}
                </div>
                {/* 合成进度（样图⑧）：真实碎片数据驱动（登录态）；未登录整块隐藏 */}
                {me && ssrSynth > 0 && (
                  <div className="sw-synth">
                    <div className="sw-synth-cap">
                      {dict.games.gachaSynthCap}
                    </div>
                    <div className="sw-synth-row">
                      <span className="sw-synth-track" aria-hidden>
                        <i
                          style={{
                            width: `${Math.min(
                              100,
                              (me.shardBalance / ssrSynth) * 100,
                            )}%`,
                          }}
                        />
                      </span>
                      <b className="num">
                        {dict.games.gachaSynthFrag
                          .replace("{a}", String(me.shardBalance))
                          .replace("{b}", String(ssrSynth))}
                      </b>
                    </div>
                    <p className="sw-synth-note">
                      {dict.games.gachaSynthNote.replace(
                        "{n}",
                        String(ssrSynth),
                      )}
                    </p>
                  </div>
                )}
              </section>
            )}
          </>
        }
      />
    </div>
  );
}
