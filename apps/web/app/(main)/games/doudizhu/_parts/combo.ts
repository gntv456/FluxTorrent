/**
 * 前端牌型识别：只用于选牌阶段展示牌名与软提示。
 * 权威合法性 / 压牌大小仍由后端 action 校验（illegalMove 文案兜底）。
 * 癞子万能替代不在前端展开，按实牌判定。
 */

export type ComboKind =
  | "single"
  | "pair"
  | "triple"
  | "tripleOne"
  | "triplePair"
  | "straight"
  | "doublePair"
  | "plane"
  | "bomb"
  | "rocket"
  | "fourTwo"
  | "invalid";

export interface ComboResult {
  kind: ComboKind;
  valid: boolean;
  count: number;
}

/** 0..12 = 3..2；13 小王；14 大王。顺连只取 0..11（3..A），2 与王不入顺。 */
function rankOf(card: number): number {
  return card >= 52 ? 13 + (card - 52) : Math.floor(card / 4);
}

/** 排序去重后的点数是否连续（相邻差 1）。 */
function isConsecutive(ranks: number[]): boolean {
  for (let i = 1; i < ranks.length; i++) {
    if (ranks[i] - ranks[i - 1] !== 1) return false;
  }
  return true;
}

export function identifyCombo(cards: number[]): ComboResult {
  const count = cards.length;
  if (count === 0) return { kind: "invalid", valid: false, count };

  // 王炸：大小王各一张
  if (count === 2 && cards.includes(52) && cards.includes(53)) {
    return { kind: "rocket", valid: true, count };
  }

  const ranks = cards.map(rankOf);
  const byRank = new Map<number, number>();
  for (const r of ranks) byRank.set(r, (byRank.get(r) ?? 0) + 1);

  const four = [...byRank.entries()].filter(([, n]) => n === 4).map(([r]) => r);
  const triples = [...byRank.entries()]
    .filter(([, n]) => n === 3)
    .map(([r]) => r);
  const pairs = [...byRank.entries()]
    .filter(([, n]) => n === 2)
    .map(([r]) => r);
  const singles = [...byRank.entries()]
    .filter(([, n]) => n === 1)
    .map(([r]) => r);

  // 炸弹：四张同点（不含王炸）
  if (four.length === 1 && count === 4) {
    return { kind: "bomb", valid: true, count };
  }

  // 四带二：四张 + 两单（6 张）或 四张 + 两对（8 张）
  if (four.length === 1 && (count === 6 || count === 8)) {
    return { kind: "fourTwo", valid: true, count };
  }

  // 飞机：两组及以上连续三张（3..A 内），可带对应单牌或对子作翼
  if (triples.length >= 2) {
    const t = [...triples].sort((a, b) => a - b);
    const wingCount = count - t.length * 3;
    const wingOk =
      wingCount === 0 ||
      (singles.length === t.length && wingCount === t.length) ||
      (pairs.length === t.length && wingCount === t.length * 2);
    if (isConsecutive(t) && t.every((r) => r <= 11) && wingOk) {
      return { kind: "plane", valid: true, count };
    }
  }

  // 三张及其带牌
  if (triples.length === 1) {
    if (count === 3) return { kind: "triple", valid: true, count };
    if (count === 4 && singles.length === 1)
      return { kind: "tripleOne", valid: true, count };
    if (count === 5 && pairs.length === 1)
      return { kind: "triplePair", valid: true, count };
  }

  // 连对：三对及以上连续点数
  if (pairs.length >= 3 && pairs.length * 2 === count) {
    const p = [...pairs].sort((a, b) => a - b);
    if (isConsecutive(p) && p.every((r) => r <= 11)) {
      return { kind: "doublePair", valid: true, count };
    }
  }

  // 顺子：五张及以上全单张、点数连续（3..A，不含 2）
  if (singles.length === count && count >= 5) {
    const s = [...singles].sort((a, b) => a - b);
    if (isConsecutive(s) && s.every((r) => r <= 11)) {
      return { kind: "straight", valid: true, count };
    }
  }

  if (pairs.length === 1 && count === 2) {
    return { kind: "pair", valid: true, count };
  }
  if (singles.length === 1 && count === 1) {
    return { kind: "single", valid: true, count };
  }

  return { kind: "invalid", valid: false, count };
}
