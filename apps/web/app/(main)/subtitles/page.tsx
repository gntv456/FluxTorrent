import { getDict } from "@/i18n/server";
import { SubtitleBoard } from "@/components/subtitle-board";

export const dynamic = "force-dynamic";

/** 字幕区（包子站 subtitles.php 同款）：上传字幕 +5 火花 */
export default async function SubtitlesPage() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.subtitles.title}</h1>
        <span className="text-sm text-sub">{dict.subtitles.subtitle}</span>
      </div>
      <SubtitleBoard empty={dict.subtitles.empty} />
    </div>
  );
}
