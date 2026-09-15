import { ResurrectionPanel } from "@/components/resurrection-panel";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

/** 复活任务独立页（0073）：面板复用（/preserve 内嵌同款），对标 U3D Graveyard */
export default async function ResurrectionsPage() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.resPage.title}</h1>
      <ResurrectionPanel />
    </div>
  );
}
