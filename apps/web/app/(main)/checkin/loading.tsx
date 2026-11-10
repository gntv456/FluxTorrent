import { getDict } from "@/i18n/server";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** app/(main)/checkin 路由级加载态（design_cov 路由加载覆盖 100% 口径）：
 *  签到页数据（状态 + 日历两 API）到位前先出骨架（IA-4 同 about 惯例）。 */
export default async function Loading() {
  const { dict } = await getDict();
  const t = dict.home2.attendanceTitle;
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{t}</h1>
      <SkeletonRows rows={2} />
      <SkeletonBlock />
    </div>
  );
}
