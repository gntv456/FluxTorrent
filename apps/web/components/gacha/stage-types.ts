/** /gacha 抽卡页（星轨卡册）的数据形状。
 *
 * 与后端三个端点的信封字段一一对应：
 *  - GET /gacha/banners → StageBanner（含起止时间与保底参数）
 *  - GET /gacha/banner/{id}/rates → StageRates（表定 base / 综合 comp 双列）
 *  - GET /gacha/me → StageMe（双余额、图鉴、近 30 抽）
 * 页面把三份数据一次取齐交给 <GachaStage/>，视图切换只在客户端换 data-v，
 * 不再为每个 Tab 单独请求。
 */

export interface StageBanner {
  id: number;
  key: string;
  name: string;
  kind: string;
  ticketCost: number;
  tenCost?: number;
  startsAt?: string | null;
  endsAt?: string | null;
  pitySoft: number;
  pityHard: number;
}

export interface StageRarity {
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
}

export interface StageRateRow {
  r: string;
  type: string;
  base: number;
  comp: number;
  value: number;
  shards: number;
  synth: number;
}

export interface StageRates {
  banner: {
    id: number;
    key: string;
    kind: string;
    ticketCost: number;
    pitySoft: number;
    pityHard: number;
    pityRamp: number;
  };
  rows: StageRateRow[];
  goldBase: number;
  goldComp: number;
  cycle: number;
  hardProb: number;
  ratioNew?: number;
  ratioEnd?: number;
  ratioWorst: number;
}

export interface StageCard {
  id: number;
  key: string;
  name: string;
  rarity: string;
  synthShards: number;
  dupeShards: number;
  lvMax: number;
  lvCostEach?: number;
}

export interface StageMeCard extends StageCard {
  held: number;
  lv: number;
  litAt: string;
  exchanged: boolean;
}

export interface StageRecent {
  seq: number;
  type: string;
  r: string | null;
  shards: number;
  wasPity: boolean;
  drawnAt?: string | null;
  pityAt?: number | null;
  isNew?: boolean;
  bannerId?: number | null;
  /** 卡牌档对应的卡 id（碎片/空抽为 null） */
  cardId?: number | null;
}

export interface StageMe {
  ticketBalance: number;
  shardBalance: number;
  cards: StageMeCard[];
  litCount: number;
  litTotal: number;
  recent?: StageRecent[];
}

/** 抽卡响应行（POST /gacha/draw → rows[]） */
export interface DrawRow {
  seq: number;
  type: string;
  r: string | null;
  shards: number;
  pityAt: number;
  wasPity: boolean;
  isNew?: boolean;
  cardId?: number;
}

export type StageView = "pool" | "reveal" | "album" | "rates" | "records";

/** 七类弹窗种类（原 gacha-stage.tsx 内部类型，拆 Modal 时随迁） */
export type ModalKind =
  "rules" | "wallet" | "progress" | "rateSpec" | "export" | "confirm" | "card";
