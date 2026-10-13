import { getDict } from "@/i18n/server";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** /torrent/[id] 路由级加载态（DS-PLAN-01 W-29）：详情页骨架
 *  （海报+信息块 + 描述行 + 文件行）。 */
export default async function Loading() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{dict.ui.loadingDetail}</h1>
      <div className="flex flex-col gap-3 md:flex-row">
        <SkeletonBlock height={220} label={dict.ui.loadingDetail} />
        <div className="flex-1">
          <SkeletonRows rows={5} />
        </div>
      </div>
      <SkeletonRows rows={6} />
    </div>
  );
}
