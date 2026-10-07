import { RouteSkeleton } from "@/components/route-skeleton";

/** (main) 布局级兜底 loading：所有未自定义骨架的子路由共享。
 *  点击导航后（main 内任意页）瞬间出反馈，不再静止数秒。 */
export default function Loading() {
  return <RouteSkeleton />;
}
