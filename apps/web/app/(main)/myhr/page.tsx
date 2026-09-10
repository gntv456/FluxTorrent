import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { MyHrTable } from "@/components/my-hr";

export const dynamic = "force-dynamic";

interface HrRow {
  torrent_id: number;
  name: string;
  size: number;
  completed_at: string | null;
  seeded_seconds: number;
  hr_flag: boolean;
  remaining_seconds: number | null;
}

/** 我的 H&R（myhr.php 复刻）：完成下载的种子做种时长与达标状态 */
export default async function MyHrPage() {
  const { dict, locale } = await getDict();
  let rows: HrRow[] = [];
  try {
    rows = await api.get<HrRow[]>("/api/v1/me/hr");
  } catch {
    // 未登录/后端不可达 → 空表
  }
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.myhr.title}</h1>
      <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{dict.myhr.rule}</p>
      <MyHrTable rows={rows} locale={locale} />
    </div>
  );
}
