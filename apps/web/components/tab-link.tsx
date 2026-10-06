"use client";

import Link, { useLinkStatus } from "next/link";
import { usePathname } from "next/navigation";
import type { ReactNode } from "react";

/** 底 Tab 链接（客户端件）：选中态 + 点击 pending 反馈。
 *
 *  - 选中态（P0-2）：usePathname 常驻高亮当前 tab——此前底 Tab
 *    无选中态，用户不知道自己在哪、点击也无确认；
 *  - pending（P1-1）：Next 15.3 useLinkStatus——慢网下点击后
 *    该 tab 图标轻微脉动（CSS .tablink[data-pending]），RSC 响应
 *    回来后停止并切换。与 (main)/loading.tsx 骨架互补：
 *    tab 上是「确认点到」，主区是骨架。
 *
 *  useLinkStatus 必须在 <Link> 的子组件里调用（Next 15 约束：
 *  只有作为 Link 直接子级的组件能读到 pending 上下文）。
 *  active 判定与 main-menu 同口径：根路径精确匹配，其余前缀匹配。 */
function TabItem({
  href,
  label,
  className,
  children,
}: {
  href: string;
  label: string;
  className: string;
  children: ReactNode;
}) {
  const pathname = usePathname();
  const { pending } = useLinkStatus();
  const active =
    href === "/"
      ? pathname === "/"
      : pathname === href || pathname.startsWith(`${href}/`);
  return (
    <Link
      href={href}
      aria-label={label}
      aria-current={active ? "page" : undefined}
      data-active={active ? "true" : undefined}
      data-pending={pending ? "true" : undefined}
      className={className}
    >
      {children}
    </Link>
  );
}

export function TabLink(props: {
  href: string;
  label: string;
  className: string;
  children: ReactNode;
}) {
  return <TabItem {...props} />;
}
