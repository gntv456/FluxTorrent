/** lunar-javascript 的最小类型声明（该包未自带 .d.ts）。
 *
 *  只声明首页万年历实际用到的那些方法；用错 API 会静默出数——
 *  尤其 `Lunar.fromYmd` 收的是**农历**年月日，太阳历必须走
 *  `Solar.fromYmd(y,m,d).getLunar()`。 */
declare module "lunar-javascript" {
  export class Lunar {
    getYearInChinese(): string;
    getYearInGanZhi(): string;
    getYearShengXiao(): string;
    /** 农历月名（九 / 十 …，不是「冬月」那种别称） */
    getMonthInChinese(): string;
    getMonthInGanZhi(): string;
    getDayInChinese(): string;
    getDayInGanZhi(): string;
    getDay(): number;
    getMonth(): number;
    getDayNaYin(): string;
    /** 形如「(甲戌)狗」 */
    getDayChongDesc(): string;
    getDaySha(): string;
    getDayTianShen(): string;
    getDayTianShenType(): string;
    getDayTianShenLuck(): string;
    /** 建除十二神（建/除/满/平/定/执/破/危/成/收/开/闭） */
    getZhiXing(): string;
    getDayYi(): string[];
    getDayJi(): string[];
    getPengZuGan(): string;
    getPengZuZhi(): string;
    getDayXun(): string;
    getDayXunKong(): string;
    /** 当日节气（无则空串） */
    getJieQi(): string;
    getFestivals(): string[];
    getOtherFestivals(): string[];
  }
  export class Solar {
    static fromYmd(y: number, m: number, d: number): Solar;
    getLunar(): Lunar;
    /** 0=周日 … 6=周六 */
    getWeek(): number;
    /** 星座（天蝎 / 天秤 …，不带「座」字） */
    getXingZuo(): string;
    getFestivals(): string[];
    getOtherFestivals(): string[];
  }
}
