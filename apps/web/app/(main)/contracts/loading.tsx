import { getDict } from "@/i18n/server";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** app/(main)/contracts 路由级加载态（design_cov 100% 纪律）。 */
export default async function Loading() {
  const { dict } = await getDict();
  const t = dict.contracts.title;
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{t}</h1>
      <SkeletonBlock height={56} label={t} />
      <SkeletonRows rows={6} />
    </div>
  );
}
