import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { MyHrTable } from "@/components/my-hr";

export const dynamic = "force-dynamic";

interface HrRow {
  torrent_id: number;
  torrent_name: string;
  required_seconds: number;
  seeded_seconds: number;
  deadline: string;
  status: string;
}

/** 我的 H&R（myhr.php 复刻）：完成下载的种子做种时长与达标状态。
 *  H&R 执法（hr_enforce）独立于 exams 模块在跑，用户记录页不再挂
 *  exams 守卫——否则 exams 关闭时形成「执法在跑、记录不可见」断链。 */
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
      <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
        {dict.myhr.rule}
      </p>
      <MyHrTable rows={rows} locale={locale} />
    </div>
  );
}
