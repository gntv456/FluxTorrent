import { getDict } from "@/i18n/server";
import { api } from "@/lib/api-client";
import { GachaRates } from "@/components/gacha-rates";
import { GachaAlbum } from "@/components/gacha-album";

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
    api
      .get<unknown[]>("/api/v1/gacha/banners")
      .catch(() => [] as unknown[]),
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
  // 星轨卡册卡位（样图⑧）：本期卡池前 3 档卡面（按稀有度星数降序）
  const first = rates.find((r): r is GachaBannerRates => r !== null);
  const topCards = (() => {
    if (!first) return [];
    const byR = new Map(rarities.map((r) => [r.key, r]));
    // 每档代表卡：按星数降序**去重档位**取前 3（高低档并列呈现，
    // 蓝卡位不会被前 3 全挤成金 —— 样图⑧ 的三卡即三个不同档）
    const rows = first.rows
      .filter((row) => row.type === "card")
      .sort((a, b) => (byR.get(b.r)?.stars ?? 0) - (byR.get(a.r)?.stars ?? 0))
      .filter(
        (row, i, arr) =>
          arr.findIndex((x) => byR.get(x.r)?.stars === byR.get(row.r)?.stars)
          === i,
      )
      .slice(0, 3);
    return rows.map((row) => {
      const meta = byR.get(row.r);
      return {
        name: meta?.label ?? row.r,
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
      {first && (
        <section className="sw-cardbook">
          <div className="sw-cardbook-cap">
            {dict.games.gachaPool} · {first.banner.name}
          </div>
          <div className="sw-cardbook-grid">
            {topCards.map((c) => (
              <div key={c.name} className={`sw-card sw-tone-${c.tone}`}>
                <span className="sw-card-stars num" aria-hidden>
                  {"★".repeat(c.stars)}
                </span>
                <span className="sw-card-face" aria-hidden>
                  🃏
                </span>
                <span className="sw-card-name">{c.name}</span>
                <span className="sw-card-rank num">{c.sub}</span>
              </div>
            ))}
          </div>
          {/* 合成进度（样图⑧）：碎片进度条 + 当期 SSR 目标说明。
              碎片数据在收藏面（登录态），未登录按 0 展示轨道 */}
          <div className="sw-synth">
            <div className="sw-synth-cap">{dict.games.gachaSynthCap}</div>
            <div className="sw-synth-row">
              <span className="sw-synth-track" aria-hidden>
                <i />
              </span>
              <b className="num">{dict.games.gachaSynthFrag}</b>
            </div>
            <p className="sw-synth-note">{dict.games.gachaSynthNote}</p>
          </div>
        </section>
      )}
      <GachaRates
        rarities={rarities}
        banners={rates.filter((r): r is GachaBannerRates => r !== null)}
      />
      {/* 登录态附加收藏面（未登录仅公示——me 端点 401 时静默省略） */}
      <GachaAlbum
        rarities={rarities}
        initial={await api
          .get<import("@/components/gacha-album").MePayload>(
            "/api/v1/gacha/me",
          )
          .catch(() => null)}
      />
    </div>
  );
}
