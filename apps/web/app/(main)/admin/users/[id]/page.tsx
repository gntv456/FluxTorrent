import { getDict } from "@/i18n/server";
import { AdminPageShell } from "@/components/admin-page-shell";
import { AdminUserDetailPage } from "@/components/admin-user-detail";

export const dynamic = "force-dynamic";

/** 后台用户详情独立页（好学站 Filament user-profile 口径）：
 *  从管理面板「用户查询」的弹层升级，承载字段全景 + 管理动作 + 关联数据。
 *  外壳与 /admin/settings、/admin/forums 共用 AdminPageShell——此前本页无
 *  左侧导航，进来后与其它后台页视觉断裂，离开只能靠「返回用户列表」。 */
export default async function Page() {
  const { dict } = await getDict();

  return (
    <AdminPageShell active="users">
      <h1 className="sr-only">{dict.admin.userDetailTitle}</h1>
      <AdminUserDetailPage />
    </AdminPageShell>
  );
}
