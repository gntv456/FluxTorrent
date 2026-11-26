import { getDict } from "@/i18n/server";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** /games/lottery 路由级加载态（2026-11-26 加速批）：服务端 requireModule
 *  闸 + 客户端列表首拉期间先出骨架，与 games 各玩法同款样式。 */
export default async function Loading() {
  const { dict } = await getDict();
  const t = dict.lottery.title;
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{t}</h1>
      <SkeletonBlock height={56} label={t} />
      <SkeletonRows rows={6} />
    </div>
  );
}
