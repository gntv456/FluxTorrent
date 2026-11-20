import { SkeletonRows } from "@/components/ui/skeleton";
import { PANEL_MD } from "@/lib/ui-classes";

/** 游戏生涯路由级加载态：与 page.tsx 的统计卡 + 空态形状对齐。 */
export default function Loading() {
  return (
    <div className="flex flex-col gap-4">
      <div className={PANEL_MD}>
        <SkeletonRows rows={3} />
      </div>
      <div className={PANEL_MD}>
        <SkeletonRows rows={5} />
      </div>
    </div>
  );
}
