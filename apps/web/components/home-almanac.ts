"use client";

import { Lunar, Solar } from "lunar-javascript";

/** 万年历：公历日 → 农历 / 干支 / 纳音 / 冲煞 / 值神 / 宜忌 / 建除 / 彭祖百忌。
 *
 *  数据源 lunar-javascript（MIT、无三方依赖）：太阳历 → 农历必须走
 *  **Solar.fromYmd(y,m,d).getLunar()**；Lunar.fromYmd 收的是**农历**年月日，
 *  用错会差两个月（本项目已实测踩过）。
 *  库覆盖 1900–2100，越界/异常一律返回 null（不抛给上层）。
 *
 *  这是纯展示层数据：后端不存、不算，签到状态仍走 /api/v1/attendance。 */

export interface AlmanacMark {
  /** 农历日（廿四） */
  lunarDay: string;
  /** 日干支（庚辰） */
  ganzhi: string;
  /** 节气或节日名（无则空串） */
  mark: string;
  /** true=节气（红字），false=节日 */
  isTerm: boolean;
}

export interface AlmanacDetail {
  /** 公历年月日 */
  y: number;
  m: number;
  d: number;
  /** 星期序号（0=周日，与 Solar.getWeek 同口径） */
  weekday: number;
  /** 农历年干支（丙午）/ 生肖（马）/ 月（九）/ 日（廿四）——拼串交给 i18n */
  lunarYearGz: string;
  lunarAnimal: string;
  lunarMonth: string;
  lunarDay: string;
  /** 年/月/日干支 */
  ganzhi: string;
  nayin: string;
  /** 冲（狗） */
  chong: string;
  /** 煞（南） */
  sha: string;
  tianShen: string;
  tianShenType: string;
  luck: string;
  /** 建除十二神当日那一神（破） */
  zhiXing: string;
  yi: string[];
  ji: string[];
  pengZuGan: string;
  pengZuZhi: string;
  xun: string;
  kong: string;
  /** 星座（天蝎） */
  zodiac: string;
}

type LunarLike = InstanceType<typeof Lunar>;

/** 取农历对象；库只覆盖 1900–2100，越界与异常都吞成 null。 */
function safeLunar(y: number, m: number, d: number): LunarLike | null {
  try {
    return Solar.fromYmd(y, m, d).getLunar();
  } catch {
    return null;
  }
}

/** 日历格上的农历信息：农历日 + 日干支 + 节气/节日标记。 */
export function almanacMark(
  y: number,
  m: number,
  d: number,
): AlmanacMark | null {
  const l = safeLunar(y, m, d);
  if (!l) return null;
  const term = l.getJieQi();
  const fest = [...l.getFestivals(), ...l.getOtherFestivals()];
  return {
    lunarDay: l.getDayInChinese(),
    ganzhi: l.getDayInGanZhi(),
    mark: term || fest[0] || "",
    isTerm: !!term,
  };
}

/** 当年第几天（1-based）。 */
export function dayOfYear(y: number, m: number, d: number): number {
  const start = Date.UTC(y, 0, 1);
  return Math.floor((Date.UTC(y, m - 1, d) - start) / 86400000) + 1;
}

/** ISO 8601 周号（周一为一周之始，含 1 月 4 日的那周是第 1 周）。 */
export function isoWeek(y: number, m: number, d: number): number {
  const cur = new Date(Date.UTC(y, m - 1, d));
  const dow = (cur.getUTCDay() + 6) % 7;
  cur.setUTCDate(cur.getUTCDate() - dow + 3);
  const first = new Date(Date.UTC(cur.getUTCFullYear(), 0, 4));
  const fDow = (first.getUTCDay() + 6) % 7;
  first.setUTCDate(first.getUTCDate() - fDow + 3);
  return 1 + Math.round((cur.getTime() - first.getTime()) / (7 * 86400000));
}

/** 某天的完整万年历明细；取不到返回 null。 */
export function almanacDetail(
  y: number,
  m: number,
  d: number,
): AlmanacDetail | null {
  const l = safeLunar(y, m, d);
  if (!l) return null;
  let solar: Solar;
  try {
    solar = Solar.fromYmd(y, m, d);
  } catch {
    return null;
  }
  // 冲煞描述形如「(甲戌)狗」——取括号后的生肖
  const chong = l.getDayChongDesc().replace(
    /^[（(].*?[)）]/,
    "",
  );
  return {
    y,
    m,
    d,
    weekday: solar.getWeek(),
    lunarYearGz: l.getYearInGanZhi(),
    lunarAnimal: l.getYearShengXiao(),
    lunarMonth: l.getMonthInChinese(),
    lunarDay: l.getDayInChinese(),
    ganzhi:
      `${l.getYearInGanZhi()} / ${l.getMonthInGanZhi()} / ` +
      `${l.getDayInGanZhi()}`,
    nayin: l.getDayNaYin(),
    chong,
    sha: l.getDaySha(),
    tianShen: l.getDayTianShen(),
    tianShenType: l.getDayTianShenType(),
    luck: l.getDayTianShenLuck(),
    zhiXing: l.getZhiXing(),
    yi: l.getDayYi(),
    ji: l.getDayJi(),
    pengZuGan: l.getPengZuGan(),
    pengZuZhi: l.getPengZuZhi(),
    xun: l.getDayXun(),
    kong: l.getDayXunKong(),
    zodiac: solar.getXingZuo(),
  };
}
