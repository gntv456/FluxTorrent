"use client";

/**
 * 管理后台 renderTool 简单页签分流（从 app/(main)/admin/page.tsx
 * 按域拆出，295 行门禁）：一个工具名 → 一个面板组件的直排 case，
 * 以及 staff-tools 承载页签的兜底。纯搬移，无逻辑改动。
 *
 * C 批首屏 JS 清偿（参照 W-33 games/_inner 模式）：直排面板全部改
 * next/dynamic 懒加载——admin 路由 chunk 曾 416kB（35+ 面板聚合），
 * 任一面板只有被 `?tool=` 命中时才拉对应 chunk（实测入口 chunk
 * 416→37kB）。面板清单本体按域拆至 ./admin-tool-map.tsx（300 行
 * 门禁），本文件只留分发与共享 flash。
 */

import type { ReactNode } from "react";
import { useI18n } from "@/i18n/client";
import type { ToolTab } from "@/components/staff-tools";
import { LAZY_PANELS } from "./admin-tool-map";

/** page.tsx 的 STAFF_TOOL_TABS 分支用的 staff-tools 懒壳（同源防重） */
export const StaffToolsLazy = LAZY_PANELS.StaffTools;

/** 由 staff-tools 承载的工具页签（tab_key 与 ToolTab 同名）
 *
 *  ⚠️ 这份清单、`staff-tools.tsx` 的 ToolTab 联合类型、以及 DB
 *  `staff_panel_entries.tab_key` 三者必须一致，否则 `?tool=X` 会落进
 *  `panelEmpty` 空面板且无任何报错。改完请跑 `_admin_nav_align.py` 对差集。
 *  `promo` 已移除：其面板（OpsPromoTab）早被 FreeleechPanel 取代且从未渲染，
 *  旧书签由 admin-shared 的 LEGACY_TOOL 映射到 freeleech。 */
export const STAFF_TOOL_TABS: ToolTab[] = [
  "faq", "rules", "cats", "bans", "mail", "staffmess", "adduser",
  "incrementbulk", "warned", "ipcheck", "maxlogin", "resetpass", "deldisabled",
  "emailbans", "testip", "emaildomains", "regquiz", "stats", "cleanup",
  "ads", "notconnect", "uploaders",
  "agents", "polls", "dbstats", "syslog", "locations", "hrpardon", "plugins",
  "agentrules", "forums", "reports", "menu", "roles", "perm", "seedstats",
  "userfields", "pages", "terms", "donateorders", "fundings", "panelentries",
  "wiki",
];

/** renderTool 的简单直排分流（复杂 case 由 page.tsx 内联处理） */
export function renderSimpleTool(
  tool: string,
  a: Record<string, string>,
  dict: ReturnType<typeof useI18n>["dict"],
): ReactNode {
  const P = LAZY_PANELS;
  switch (tool) {
    case "users":
      return <P.AdminUsers classes={dict.admin.classList} />;
    case "torrents":
      return <P.AdminTorrents />;
    // 举报处理（0336 trumping 的版主入口；导航项见迁移 0339）
    case "trumps":
      return <P.AdminTrumps />;
    case "content":
      return (
        <section className="nexus-detail">
          <h2 className="mb-3 text-base font-bold text-ink">
            {a.sectionContent ?? "content"}
          </h2>
          <P.ContentManage />
        </section>
      );
    case "freeleech":
      // 站方级促销 + intent 审批队列（0356）+ 用户自购档位注册表（0213）
      return (
        <>
          <P.FreeleechPanel />
          <P.PromoIntentsPanel />
          <P.AdminPromoKinds />
        </>
      );
    case "clearcache":
      return <P.ClearCachePanel />;
    case "p2tools":
      return <P.AdminP2Tools />;
    // 第八轮 P3 套件（好学站后台逐页深挖落地）
    case "hr":
      return <P.AdminHr />;
    // P-19：等级规则（class_rules + user_classes 的后台编辑面）
    case "classes":
      return <P.AdminClasses />;
    case "invites":
      return <P.AdminInvites />;
    case "userlogs":
      return <P.AdminUserLogs />;
    case "attendance":
      return <P.AdminAttendance />;
    case "tagdict":
      return <P.AdminTagDict />;
    case "sections":
      return <P.AdminSections />;
    case "medals":
      return <P.AdminMedals />;
    // 金字幕评选管理（0150）：候选/授金
    case "subawards":
      return <P.AdminSubtitlesAwards />;
    case "props":
      return <P.AdminProps />;
    case "exams":
      return <P.AdminExams />;
    case "jixiao":
      return <P.AdminJixiao />;
    case "tasks":
      return <P.AdminTasks />;
    case "trackers":
      return <P.AdminTrackers />;
    // 运维三件套（0078）：版本信息 / 备份面板 / 任务手动触发
    case "ops":
      return <P.AdminOpsPanel />;
    // 首页排版（0089）：板块顺序/宽度/显隐可视化编辑
    case "homelayout":
      return (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 font-display text-lg">
            {dict.homeLayout.title}
          </h2>
          <P.HomeLayoutEditor />
        </section>
      );
    // 视频内嵌规则管理（0189）
    case "embedrules":
      return <P.AdminEmbedRules />;
    // C4 新手运营模板（0226）
    case "onboarding":
      return <P.AdminOnboarding />;
    // C5 经济反通胀运营面板（0226）
    case "economy":
      return <P.AdminEconomyDashboard />;
    // 申请制入站审核（0227）
    case "applications":
      return <P.AdminApplications />;
    // 抽卡运营（G31-C 后台入口：发放+巡检）
    case "gacha":
      return <P.AdminGacha />;
    // 娱乐屋运营（P3 后台入口：EV 三口径 / 门禁 / 参数 / 定义）
    case "arcade":
      return <P.AdminArcade />;
    // 视图布局（E6）：列表列/详情段落显隐
    case "viewlayout":
      return (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 font-display text-lg">
            {dict.viewLayout.title}
          </h2>
          <P.ViewHiddenEditor />
        </section>
      );
    default:
      return undefined;
  }
}
