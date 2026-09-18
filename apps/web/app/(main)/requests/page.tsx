import { RequestBoard } from "@/components/request-board";
import { getDict } from "@/i18n/server";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 求种区（参考站 viewrequests.php 复刻）：REQUEST CENTER 头 + 六个筛选 + 八列表格 */
export default async function RequestsPage({
  searchParams,
}: {
  searchParams: Promise<{ finished?: string; search?: string }>;
}) {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("requests");
  if (gate) return gate;

  const { dict } = await getDict();
  const sp = await searchParams;
  return (
    <div className="flex flex-col gap-4">
      <h1 className="sr-only">{dict.requests.title}</h1>
      <RequestBoard initialFinished={sp.finished ?? "no"} initialSearch={sp.search ?? ""} />
    </div>
  );
}
