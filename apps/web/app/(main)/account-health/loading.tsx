import { getDict } from "@/i18n/server";
import { accountHealthCopy } from "@/i18n/account-health";
import { PageHeader } from "@/components/ui/page-header";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** app/(main)/account-health 路由级加载态：三卡（风险/引导/规则）
 *  首屏骨架，与页面本体同一 PageHeader 节奏。 */
export default async function Loading() {
  const { dict, locale } = await getDict();
  return (
    <div className="flex flex-col gap-3">
      <PageHeader
        eyebrow={dict.my.title}
        title={accountHealthCopy[locale].title}
      />
      <SkeletonBlock height={160} label={accountHealthCopy[locale].title} />
      <SkeletonRows rows={3} />
    </div>
  );
}
