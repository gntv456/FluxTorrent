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
  return (
    <div className="flex flex-col gap-4">
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">Gacha</div>
          <h1 className="font-display text-2xl">{dict.nav.gachaDisclosure}</h1>
        </div>
      </div>
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
