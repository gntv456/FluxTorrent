import { getDict } from "@/i18n/server";
import { ContestBoard } from "@/components/plugins";

export const dynamic = "force-dynamic";

/** 大赛（contest 插件复刻）：进行中/历史大赛列表 + 一键报名 */
export default async function ContestsPage() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.contests2.title}</h1>
      <ContestBoard />
    </div>
  );
}
