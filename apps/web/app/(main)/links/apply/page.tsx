import { LinkApply } from "@/components/link-apply";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

/** 友情链接申请（参考站 linksmanage.php 复刻） */
export default async function LinkApplyPage() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.linkapply.title}</h1>
      <LinkApply />
    </div>
  );
}
