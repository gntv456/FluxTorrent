import { getDict } from "@/i18n/server";
import { SkeletonRows } from "@/components/ui/skeleton";

/** /avatar-frames 路由级加载态（S1 快速修 2026-10-16）：补 loading 缺口——
 *  先出骨架给出形状暗示，避免「加载中 / 真的没数据」歧义。 */
export default async function Loading() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{dict.common.loading}</h1>
      <SkeletonRows rows={6} />
    </div>
  );
}
