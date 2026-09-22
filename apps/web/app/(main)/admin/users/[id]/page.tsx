import { getDict } from "@/i18n/server";
import { AdminUserDetailPage } from "@/components/admin-user-detail";

export const dynamic = "force-dynamic";

/** 后台用户详情独立页（好学站 Filament user-profile 口径）：
 *  从管理面板「用户查询」的弹层升级，承载字段全景 + 管理动作 + 关联数据。 */
export default async function Page() {
  const { dict } = await getDict();

  return (
    <div className="flex flex-col gap-4">
      <h1 className="sr-only">{dict.admin.userDetailTitle}</h1>
      <AdminUserDetailPage />
    </div>
  );
}
