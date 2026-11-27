import { api } from "@/lib/api-client";
import { GachaStage } from "@/components/gacha/gacha-stage";
import type {
  StageBanner,
  StageCard,
  StageMe,
  StageRarity,
  StageRates,
} from "@/components/gacha/stage-types";

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

/** /gacha 卡池列表行：2026-11 起 banners 端点补下起止/保底/十连价 */
interface BannerRow {
  id: number;
  key: string;
  name: string;
  kind: string;
  ticketCost: number;
  tenCost?: number | null;
  startsAt?: string | null;
  endsAt?: string | null;
  pitySoft?: number | null;
  pityHard?: number | null;
}

interface CardRow {
  id: number;
  key: string;
  name: string;
  rarity: string;
  synthShards: number;
  dupeShards: number;
  lvMax: number;
  lvCostEach?: number;
}

/**
 * 星轨卡册抽卡页（设计稿 抽卡页-高保真样图.html 的生产落地）。
 *
 * 五视图 × 单屏：卡池 / 演出 / 卡册 / 概率 / 记录，全部数据一次取齐后交给
 * <GachaStage/>，视图切换只换 data-v，不重新请求 RSC；只有换卡池时客户端
 * 再补拉该池 rates。匿名可读（me 取不到时 HUD 显示占位、抽卡提示登录）。
 */
export default async function GachaDisclosurePage() {
  const [rarities, banners, cards, me] = await Promise.all([
    api
      .get<StageRarity[]>("/api/v1/gacha/rarities")
      .catch(() => [] as StageRarity[]),
    api
      .get<BannerRow[]>("/api/v1/gacha/banners")
      .catch(() => [] as BannerRow[]),
    api.get<CardRow[]>("/api/v1/gacha/cards").catch(() => [] as CardRow[]),
    api.get<StageMe>("/api/v1/gacha/me").catch(() => null),
  ]);

  const firstId = banners[0]?.id;
  const rates =
    firstId != null
      ? await api
          .get<GachaBannerRates>(`/api/v1/gacha/banner/${firstId}/rates`)
          .catch(() => null)
      : null;

  const stageBanners: StageBanner[] = banners.map((b) => ({
    id: b.id,
    key: b.key,
    name: b.name,
    kind: b.kind,
    ticketCost: b.ticketCost,
    tenCost: b.tenCost ?? b.ticketCost * 10,
    startsAt: b.startsAt ?? null,
    endsAt: b.endsAt ?? null,
    pitySoft: b.pitySoft ?? rates?.banner.pitySoft ?? 50,
    pityHard: b.pityHard ?? rates?.banner.pityHard ?? 65,
  }));

  const stageCards: StageCard[] = cards.map((c) => ({
    id: c.id,
    key: c.key,
    name: c.name,
    rarity: c.rarity,
    synthShards: c.synthShards,
    dupeShards: c.dupeShards,
    lvMax: c.lvMax,
    lvCostEach: c.lvCostEach,
  }));

  const stageRates: StageRates | null = rates
    ? {
        banner: {
          id: rates.banner.id,
          key: rates.banner.key,
          kind: rates.banner.kind,
          ticketCost: rates.banner.ticketCost,
          pitySoft: rates.banner.pitySoft,
          pityHard: rates.banner.pityHard,
          pityRamp: rates.banner.pityRamp,
        },
        rows: rates.rows,
        goldBase: rates.goldBase,
        goldComp: rates.goldComp,
        cycle: rates.cycle,
        hardProb: rates.hardProb,
        ratioNew: rates.ratioNew,
        ratioEnd: rates.ratioEnd,
        ratioWorst: rates.ratioWorst,
      }
    : null;

  return (
    <div className="gacha-page" data-gacha-page="astral">
      {/* 抽卡页不再挂 PageHeader：HUD 自带标题与导航，一屏装下不滚动 */}
      <GachaStage
        banners={stageBanners}
        rarities={rarities}
        cards={stageCards}
        me={me}
        rates={stageRates}
      />
    </div>
  );
}
