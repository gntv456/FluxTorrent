import "@/app/styles/ux-profile-admin.css";
import "@/app/styles/ux-product.css";

/**
 * 管理后台路由级样式挂载点（B4 纪律）：
 * ux-profile-admin.css 与 users/[id]/page.tsx 各自 import 一次，
 * 覆盖 /admin 主面板与 /admin/settings、/admin/forums、/admin/users/[id]
 * 等独立子路由（它们共用 admin-nav-groups / admin-shell）。
 * 本布局不产出任何额外 DOM，不改变现有 (main) 布局与页面结构。
 */
export default function AdminLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return <>{children}</>;
}
