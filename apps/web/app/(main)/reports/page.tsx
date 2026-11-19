import { ReportBox } from "@/components/report-box";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

/** 举报信箱（同类站 reports.php 口径）：管理组处理队列 + 提交新举报 */
export default async function ReportsPage({
  searchParams,
}: {
  searchParams: Promise<{ type?: string | string[]; id?: string | string[] }>;
}) {
  const { dict } = await getDict();
  const sp = await searchParams;
  const one = (v: string | string[] | undefined) =>
    Array.isArray(v) ? v[0] ?? "" : v ?? "";
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.reportbox.title}</h1>
      <ReportBox
        initialType={one(sp.type)}
        initialId={one(sp.id)}
      />
    </div>
  );
}
