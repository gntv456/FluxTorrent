import { getDict } from "@/i18n/server";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** /users/[id] 路由级加载态（DS-PLAN-01 W-29）：用户档案骨架
 *  （头像信息块 + 数据行）。/me 重定向壳的落地页。 */
export default async function Loading() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{dict.ui.loadingDetail}</h1>
      <div className="flex flex-col gap-3 md:flex-row">
        <SkeletonBlock height={140} label={dict.ui.loadingDetail} />
        <div className="flex-1">
          <SkeletonRows rows={4} />
        </div>
      </div>
      <SkeletonRows rows={6} />
    </div>
  );
}
