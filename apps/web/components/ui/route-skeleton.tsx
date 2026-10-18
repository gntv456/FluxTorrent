import { getDict } from "@/i18n/server";
import { SkeletonRows } from "@/components/ui/skeleton";

/** 通用路由级加载态（懒加载批 2026-10-19）：补齐缺 loading.tsx 的路由。
 *  骨架标题走 dict.<域>.title（与目标页 h1 同源），列表型页面统一
 *  SkeletonRows；重表单/面板页可后续按需换 SkeletonBlock。 */
export function RouteSkeleton({
  title,
  rows = 4,
}: {
  title: string;
  rows?: number;
}) {
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{title}</h1>
      <SkeletonRows rows={rows} />
    </div>
  );
}

/** 按「页面 → 骨架标题键」生成 loading 组件的工厂（每路由一个薄壳文件，
 *  保持 App Router loading.tsx 约定；工厂只供这些薄壳复用）。 */
export function loadingFor(dictPath: string, rows = 4) {
  async function Loading() {
    const { dict } = await getDict();
    // 两级取值（dict.forums.title / dict.games.bigsmall.title）
    const segs = dictPath.split(".");
    let t: unknown = dict as unknown as Record<string, unknown>;
    for (const s of segs) {
      t = (t as Record<string, unknown>)?.[s];
      if (t == null) break;
    }
    const title = typeof t === "string" ? t : dict.ui.loadingList;
    return <RouteSkeleton title={title} rows={rows} />;
  }
  return Loading;
}
