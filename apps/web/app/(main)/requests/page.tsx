import { RequestBoard } from "@/components/request-board";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

/** 求种区（包子站 viewrequests.php 复刻）：REQUEST CENTER 头 + 六个筛选 + 八列表格 */
export default async function RequestsPage({
  searchParams,
}: {
  searchParams: Promise<{ finished?: string; search?: string }>;
}) {
  const { dict } = await getDict();
  const sp = await searchParams;
  return (
    <div className="flex flex-col gap-4">
      <h1 className="sr-only">{dict.requests.title}</h1>
      <RequestBoard initialFinished={sp.finished ?? "no"} initialSearch={sp.search ?? ""} />
    </div>
  );
}
