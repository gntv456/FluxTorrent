/** 路由切换骨架（移动端点击反馈 P0）：App Router 的 loading.tsx 专属件。
 *
 *  背景：全站此前 0 个 loading.tsx，页面切换期间（RSC 响应回来之前）
 *  界面完全静止——手机弱网下这段真空期数秒，主观感受就是
 *  「点了没反应，等一会儿才跳」。loading.tsx 让点击后**瞬间**出骨架。
 *
 *  渲染时布局（顶栏/底 Tab）仍在，只有 <main> 内容换成骨架，
 *  导航自身不闪。文案 common.pageLoading 已入四语字典。
 *
 *  服务端组件（loading.tsx 在 RSC 侧渲染），不加 "use client"。 */
import { getDict } from "@/i18n/server";
import { SkeletonRows } from "@/components/ui/skeleton";

export async function RouteSkeleton({
  rows = 8,
  height = 38,
}: {
  /** 骨架行数（列表页 8-10，详情页 5） */
  rows?: number;
  /** 单行高度 px，与目标页真实行高对齐减少跳动 */
  height?: number;
}) {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-3 py-2" role="status" aria-busy="true">
      <p className="text-sm text-sub">{dict.common.pageLoading}</p>
      <SkeletonRows rows={rows} height={height} />
    </div>
  );
}
