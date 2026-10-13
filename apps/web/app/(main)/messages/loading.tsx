import { getDict } from "@/i18n/server";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** /messages 路由级加载态（DS-PLAN-01 W-29）：工具条 + 消息行骨架。
 *  注：MessageCenter 客户端首屏亦有 StatePanel 骨架（双保险），本文件
 *  管 RSC 流式窗口期。 */
export default async function Loading() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{dict.ui.loadingList}</h1>
      <SkeletonBlock height={44} label={dict.ui.loadingList} />
      <SkeletonRows rows={8} />
    </div>
  );
}
