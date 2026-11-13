import { getDict } from "@/i18n/server";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** app/(main)/forums/(shell)/[id] 版块页加载态：(shell)/loading.tsx 只在段根
 *  生效罩不住子段 [id]，design_cov_guard 判该路由无加载态（99%<100% 红）。
 *  骨架形状对齐 (shell)/loading.tsx（IA-4：带 heading）；三栏两翼由
 *  (shell)/layout.tsx 保持原位，本骨架只填中栏 children 槽。 */
export default async function Loading() {
  const { dict } = await getDict();
  const t = dict.forums.title;
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{t}</h1>
      <SkeletonBlock height={56} label={t} />
      <SkeletonRows rows={10} />
    </div>
  );
}
