"use client";

import { useState } from "react";
import { NavDrawer, NavDrawerTrigger } from "@/components/nav-drawer";
import { useI18n } from "@/i18n/client";
import type { NavItem, NavGroup } from "@/lib/nav-menu";

/** 移动端顶栏汉堡壳（M2）：客户端组件持有抽屉开关态。
 *  配置（primary/groups）与用户摘要由 RSC 侧算好以 props 传入——
 *  与桌面 Header 完全同源（lib/nav-menu.ts），不在客户端重复取档。 */
export function MobileNavShell({
  primary,
  groups,
  summary,
}: {
  primary: NavItem[];
  groups: NavGroup[];
  summary: {
    username: string;
    className: string;
    uploaded: string;
    spark: string;
  } | null;
}) {
  const [open, setOpen] = useState(false);
  const { dict } = useI18n();
  return (
    <>
      <NavDrawerTrigger
        onClick={() => setOpen(true)}
        label={dict.navDrawer.open}
      />
      <NavDrawer
        open={open}
        onClose={() => setOpen(false)}
        primary={primary}
        groups={groups}
        summary={summary}
      />
    </>
  );
}
