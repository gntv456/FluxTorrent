import { SubtitleBoard } from "@/components/subtitle-board";
import { getDict } from "@/i18n/server";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 字幕区（参考站 subtitles.php 复刻）：规则卡 + 上传表单 + 语言/字母筛选 + 七列表格 */
export default async function SubtitlesPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("subtitles");
  if (gate) return gate;

  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="sr-only">{dict.subtitles.title}</h1>
      <SubtitleBoard />
    </div>
  );
}
