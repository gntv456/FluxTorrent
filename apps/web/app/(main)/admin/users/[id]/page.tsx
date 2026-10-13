import nextDynamic from "next/dynamic";
import { AdminPageShell } from "@/components/admin-page-shell";
import { SkeletonRows } from "@/components/ui/skeleton";

export const dynamic = "force-dynamic";

// W-33 代码分割：后台重表格族（actions/panels/hooks 十余文件）只在
// 本路由出现，拆出首屏 chunk；loading 骨架防白屏。
const AdminUserDetailPage = nextDynamic(
  () =>
    import("@/components/admin-user-detail").then(
      (m) => m.AdminUserDetailPage,
    ),
  { loading: () => <SkeletonRows rows={8} /> },
);

/** 后台用户详情独立页（好学站 Filament user-profile 口径）：
 *  从管理面板「用户查询」的弹层升级，承载字段全景 + 管理动作 + 关联数据。
 *  外壳与 /admin/settings、/admin/forums 共用 AdminPageShell——此前本页无
 *  左侧导航，进来后与其它后台页视觉断裂，离开只能靠「返回用户列表」。
 *  h1 由 AdminUserDetailPage 的用户名标题自持，此处不再挂 sr-only 标题。 */
export default async function Page() {
  return (
    <AdminPageShell active="users">
      <AdminUserDetailPage />
    </AdminPageShell>
  );
}
