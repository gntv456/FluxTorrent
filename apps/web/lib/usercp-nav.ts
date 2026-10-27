/** 控制面板 tab 清单与侧栏导航（中性模块，无 "use client"）。
 *
 *  `/my` 是 server component，要在服务端读 USERCP_NAV（关心愿单模块时
 *  过滤掉那一档）。放在 client 模块 usercp.tsx 里时，服务端拿到的只是
 *  client reference——取值恒 undefined，`.filter()` 那支直接抛。
 */

/** 控制面板六个 tab + 心愿单（0209 P2-15：心愿单受模块开关） */
export type UsercpTab =
  | "overview"
  | "personal"
  | "fields"
  | "tracker"
  | "forum"
  | "security"
  | "wishlist";

/** 侧栏导航图标：走 TIDE 图标系统（components/icons.tsx 的语义 SVG），
 *  颜色继承 currentColor，浅色/夜间自动适配。
 *  2026-10-03 修：此前用 ⌂♙✎⚙♧⌾☆ 七个键盘符号/装饰字符当图标，
 *  其中 ✎(U+270E) 缺字形在多数字体下渲染成「、」，控制面板第 4 项
 *  只剩一个孤立顿号；且与全站其他页面的图标体系不统一。 */
export const USERCP_NAV: { key: UsercpTab; icon: string }[] = [
  { key: "overview", icon: "gauge" },
  { key: "personal", icon: "user" },
  { key: "fields", icon: "edit" },
  { key: "tracker", icon: "settings" },
  { key: "forum", icon: "forums" },
  { key: "security", icon: "shield" },
  { key: "wishlist", icon: "star" },
];
