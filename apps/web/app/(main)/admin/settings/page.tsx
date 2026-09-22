import { api } from "@/lib/api-client";
import { AdminPageShell } from "@/components/admin-page-shell";
import type { SettingsSchema } from "@/components/setting-field";
import { SettingsClient } from "./settings-client";

/**
 * 站点设定（方案 §5）：从 /admin 页签升级为独立路由 /admin/settings。
 *
 * 这里作为 RSC 先取好 schema 再交给客户端组件，使 12 分区 / 303 字段首屏即进入
 * HTML（避免「空壳 → 客户端拉取 → 二次渲染」造成的 LCP 拖尾）；
 * 预取失败时传 null，由客户端组件回退到浏览器侧重试。
 *
 * 外壳用 AdminPageShell：本页此前**没有左侧导航**，进来之后无法跳到别的管理
 * 工具（从设置页「走出去」只能靠浏览器后退）。现在与 /admin、/admin/forums
 * 共用同一套职能导航。
 */
export default async function AdminSettingsPage() {
  let initialSchema: SettingsSchema | null = null;
  try {
    initialSchema = await api.get<SettingsSchema>(
      "/api/v1/admin/settings/schema",
    );
  } catch {
    // 预取失败（未登录 / 权限不足 / 服务异常）：交给客户端组件回退重试并提示
    initialSchema = null;
  }
  return (
    <AdminPageShell active="settings">
      <SettingsClient initialSchema={initialSchema} />
    </AdminPageShell>
  );
}
