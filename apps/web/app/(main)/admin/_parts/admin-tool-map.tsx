"use client";

/**
 * 管理后台直排面板的懒加载注册表（C 批首屏 JS 清偿，从
 * admin-tool-switch.tsx 按域拆出）：33 个工具面板全部
 * next/dynamic——任一面板只有被 `?tool=` 命中时才拉对应 chunk
 * （admin 入口 chunk 416→37kB 实测）。loading 用工具页既有
 * 骨架观感（与 loading.tsx 同款省略号）。
 */

import type { ReactNode } from "react";
import dynamic from "next/dynamic";

function toolFallback() {
  return (
    <p className="py-8 text-center text-sub" aria-busy="true">
      …
    </p>
  );
}

/** 具名导出 → dynamic 包装。入参/返回都放宽（各面板 props 形状
 *  互不相同：initialTab/classes/无 props…，真实约束保留在各模块
 *  签名里），这里只做懒加载壳；调用侧 JSX 的 props 合法性由
 *  各模块签名人工对齐（拆分前同款直排用法，未改任何传参）。 */
function lazy(load: () => unknown) {
  const C = dynamic(load as never, { loading: toolFallback });
  // 面板组件 props 各异，统一擦成「接受任意 props 的组件」
  return C as unknown as (p: Record<string, unknown>) => ReactNode;
}

export const LAZY_PANELS = {
  StaffTools: lazy(() =>
    import("@/components/staff-tools").then((m) => ({
      default: m.StaffTools,
    })),
  ),
    ContentManage: lazy(() =>
    import("@/components/content-manage").then((m) => ({
      default: m.ContentManage,
    })),
  ),
    AdminUsers: lazy(() =>
    import("@/components/admin-users").then((m) => ({
      default: m.AdminUsers,
    })),
  ),
    AdminTorrents: lazy(() =>
    import("@/components/admin-torrents").then((m) => ({
      default: m.AdminTorrents,
    })),
  ),
    AdminTrumps: lazy(() =>
    import("@/components/admin-trumps").then((m) => ({
      default: m.AdminTrumps,
    })),
  ),
    AdminP2Tools: lazy(() =>
    import("@/components/admin-p2-tools").then((m) => ({
      default: m.AdminP2Tools,
    })),
  ),
    AdminHr: lazy(() =>
    import("@/components/admin-hr").then((m) => ({
      default: m.AdminHr,
    })),
  ),
    AdminClasses: lazy(() =>
    import("@/components/admin-classes").then((m) => ({
      default: m.AdminClasses,
    })),
  ),
    AdminInvites: lazy(() =>
    import("@/components/admin-invites").then((m) => ({
      default: m.AdminInvites,
    })),
  ),
    AdminUserLogs: lazy(() =>
    import("@/components/admin-userlogs").then((m) => ({
      default: m.AdminUserLogs,
    })),
  ),
    AdminAttendance: lazy(() =>
    import("@/components/admin-attendance").then((m) => ({
      default: m.AdminAttendance,
    })),
  ),
    AdminTagDict: lazy(() =>
    import("@/components/admin-tagdict").then((m) => ({
      default: m.AdminTagDict,
    })),
  ),
    AdminSections: lazy(() =>
    import("@/components/admin-sections").then((m) => ({
      default: m.AdminSections,
    })),
  ),
    AdminMedals: lazy(() =>
    import("@/components/admin-medals").then((m) => ({
      default: m.AdminMedals,
    })),
  ),
    AdminSubtitlesAwards: lazy(() =>
    import("@/components/admin-subtitles-awards").then((m) => ({
      default: m.AdminSubtitlesAwards,
    })),
  ),
    AdminProps: lazy(() =>
    import("@/components/admin-props").then((m) => ({
      default: m.AdminProps,
    })),
  ),
    AdminExams: lazy(() =>
    import("@/components/admin-exams").then((m) => ({
      default: m.AdminExams,
    })),
  ),
    AdminJixiao: lazy(() =>
    import("@/components/admin-jixiao").then((m) => ({
      default: m.AdminJixiao,
    })),
  ),
    AdminTasks: lazy(() =>
    import("@/components/admin-tasks").then((m) => ({
      default: m.AdminTasks,
    })),
  ),
    AdminTrackers: lazy(() =>
    import("@/components/admin-trackers").then((m) => ({
      default: m.AdminTrackers,
    })),
  ),
    AdminOpsPanel: lazy(() =>
    import("@/components/admin-ops").then((m) => ({
      default: m.AdminOpsPanel,
    })),
  ),
    AdminEmbedRules: lazy(() =>
    import("@/components/admin-embed-rules").then((m) => ({
      default: m.AdminEmbedRules,
    })),
  ),
    AdminOnboarding: lazy(() =>
    import("@/components/admin-onboarding").then((m) => ({
      default: m.AdminOnboarding,
    })),
  ),
    AdminEconomyDashboard: lazy(() =>
    import("@/components/admin-economy-dashboard").then((m) => ({
      default: m.AdminEconomyDashboard,
    })),
  ),
    AdminApplications: lazy(() =>
    import("@/components/admin-applications").then((m) => ({
      default: m.AdminApplications,
    })),
  ),
    AdminGacha: lazy(() =>
    import("@/components/admin-gacha").then((m) => ({
      default: m.AdminGacha,
    })),
  ),
    AdminArcade: lazy(() =>
    import("@/components/admin-arcade").then((m) => ({
      default: m.AdminArcade,
    })),
  ),
    HomeLayoutEditor: lazy(() =>
    import("@/components/home-layout-editor").then((m) => ({
      default: m.HomeLayoutEditor,
    })),
  ),
    ViewHiddenEditor: lazy(() =>
    import("@/components/view-hidden-editor").then((m) => ({
      default: m.ViewHiddenEditor,
    })),
  ),
    FreeleechPanel: lazy(() =>
    import("./admin-freeleech").then((m) => ({
      default: m.FreeleechPanel,
    })),
  ),
    ClearCachePanel: lazy(() =>
    import("./admin-freeleech").then((m) => ({
      default: m.ClearCachePanel,
    })),
  ),
    PromoIntentsPanel: lazy(() =>
    import("./admin-promo-intents").then((m) => ({
      default: m.PromoIntentsPanel,
    })),
  ),
    AdminPromoKinds: lazy(() =>
    import("./admin-promo-kinds").then((m) => ({
      default: m.AdminPromoKinds,
    })),
  ),
} as const;
