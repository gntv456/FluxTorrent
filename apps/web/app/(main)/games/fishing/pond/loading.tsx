import { getDict } from "@/i18n/server";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** /games/fishing/pond 路由级加载态（2026-11-26 加速批）：RSC 流式入口——
 *  渔场页服务端拉 /me/overview + 渔场数据，此前无 loading 边界，
 *  TTFB 要等全部数据拉完；补骨架后首屏先出，数据到位再替换。 */
export default async function Loading() {
  const { dict } = await getDict();
  const t = dict.games.fishing.pondCrumb;
  return (
    <div className="mx-auto w-full max-w-[1180px] px-3 pb-24 sm:px-4 md:pb-5">
      <h1 className="font-display text-2xl">{t}</h1>
      <SkeletonBlock height={56} label={t} />
      <SkeletonRows rows={8} />
    </div>
  );
}
