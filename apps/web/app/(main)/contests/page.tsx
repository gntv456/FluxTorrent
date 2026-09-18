import { getDict } from "@/i18n/server";
import { ContestBoard } from "@/components/plugins";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 大赛（contest 插件复刻）：进行中/历史大赛列表 + 一键报名 */
export default async function ContestsPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("contests");
  if (gate) return gate;

  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.contests2.title}</h1>
      <ContestBoard />
    </div>
  );
}
