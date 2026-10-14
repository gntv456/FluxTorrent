import { getDict } from "@/i18n/server";
import { SkeletonRows } from "@/components/ui/skeleton";

/** /links/apply 路由级加载态（DS §9 验收第 2 条清偿）：轻数据页
 *  （dict 快接口），铺轻量骨架对齐 RSC 流式口径，防白屏。 */
export default async function Loading() {
  const { dict } = await getDict();
  const t = dict.linkapply.title;
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{t}</h1>
      <SkeletonRows rows={4} />
    </div>
  );
}
