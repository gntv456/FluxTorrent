import nextDynamic from "next/dynamic";
import Link from "next/link";
import { getDict } from "@/i18n/server";
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

/** 后台用户详情独立页（同类站 Filament user-profile 口径）：
 *  从管理面板「用户查询」的弹层升级，承载字段全景 + 管理动作 + 关联数据。
 *  外壳与 /admin/settings、/admin/forums 共用 AdminPageShell——此前本页无
 *  左侧导航，进来后与其它后台页视觉断裂，离开只能靠「返回用户列表」。
 *
 *  页头（对齐样图 § 用户详情）：标题「用户详情」+ 副标题 + 右侧返回按钮；
 *  h1 由 AdminPageShell 的 PageHeader 自持，用户名身份条在内容区首行
 *  （数据加载完成后才可知，放不进服务端页头）。 */
export default async function Page() {
  const { dict } = await getDict();
  const u = dict.userDetail;
  return (
    <AdminPageShell
      active="users"
      headTitle={u.pageTitle}
      headSub={u.pageSub}
      headAside={
        <Link
          href="/admin?tool=users"
          className="min-h-[36px] rounded-full border border-line
            px-4 text-xs font-bold text-ink"
        >
          ‹ {u.backToUsers}
        </Link>
      }
    >
      <AdminUserDetailPage />
    </AdminPageShell>
  );
}
