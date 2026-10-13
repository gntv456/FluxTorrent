import { getDict } from "@/i18n/server";
import { SkeletonBlock } from "@/components/ui/skeleton";

/** /upload 路由级加载态（DS-PLAN-01 W-29）：发种表单骨架（表单态术语）。
 *  发种页是重表单（文件解析/字段众多），首屏骨架对齐真实表单高度，
 *  避免加载完 CLS 跳动（§12.1 CLS≤0.1 的根治手段）。 */
export default async function Loading() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{dict.ui.loadingForm}</h1>
      <SkeletonBlock height={44} label={dict.ui.loadingForm} />
      <SkeletonBlock height={180} label={dict.ui.loadingForm} />
      <SkeletonBlock height={120} label={dict.ui.loadingForm} />
    </div>
  );
}
