import { getDict } from "@/i18n/server";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** /rules 路由级加载态（DS-PLAN-01 W-29）：服务端取 rules-content
 *  （未登录/失败回落静态文案）期间，先出目录骨架防白屏。 */
export default async function Loading() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{dict.ui.loadingList}</h1>
      <SkeletonBlock height={56} label={dict.ui.loadingList} />
      <SkeletonRows rows={8} />
    </div>
  );
}
