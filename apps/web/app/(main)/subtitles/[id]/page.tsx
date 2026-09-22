import { notFound } from "next/navigation";
import { getDict } from "@/i18n/server";
import { requireModule } from "@/components/module-gate";
import { SubtitleDetailView } from "@/components/subtitle-detail-view";

export const dynamic = "force-dynamic";

/** 字幕详情页（0150 缺口1）：/subtitles/{id}——元数据/下载统计/修订版链 */
export default async function SubtitleDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const gate = await requireModule("subtitles");
  if (gate) return gate;
  const { id } = await params;
  const sid = Number(id);
  if (!Number.isFinite(sid) || sid <= 0) notFound();
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="sr-only">{dict.subtitleDetail.title}</h1>
      <SubtitleDetailView sid={sid} />
    </div>
  );
}
