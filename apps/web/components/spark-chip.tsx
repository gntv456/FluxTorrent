"use client";

import { useI18n } from "@/i18n/client";

/** 顶栏魔力微缩 chip（DS-PLAN-01 W-31 / IA-2 ①，从 user-menu.tsx 拆出：
 *  主件压在 300 行门禁内）。单一货币常态直显——高频信息不藏弹窗
 *  （横评结论 §11.5-2）；点击进 /my-spark 明细；余额为 0 也常显
 *  （数字本身就是信息）。
 *  纯展示：余额文本由主件传入（/me spark_balance 初值 + /me/spark 校准，
 *  签到回调同源刷新）；站长用 site_settings.topbar_spark_chip=no 可整体
 *  隐藏（主件按 site-profile 控制是否渲染）。 */
export function SparkChip({ balance }: { balance: string }) {
  const { dict, currency } = useI18n();
  const label = dict.my.sparkChipTitle.replace("{magic}", currency);
  return (
    <a href="/my-spark" className="usermenu-spark" title={label}>
      <span aria-hidden className="usermenu-spark__dot" />
      <b className="num">{balance}</b>
      <span className="sr-only">{label}</span>
    </a>
  );
}
