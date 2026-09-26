"use client";

/**
 * 管理后台 renderTool 简单页签分流（从 app/(main)/admin/page.tsx
 * 按域拆出，295 行门禁）：一个工具名 → 一个面板组件的直排 case，
 * 以及 staff-tools 承载页签的兜底。纯搬移，无逻辑改动。
 */

import { useI18n } from "@/i18n/client";
import { ContentManage } from "@/components/content-manage";
import { StaffTools, type ToolTab } from "@/components/staff-tools";
import { AdminUsers } from "@/components/admin-users";
import { AdminTorrents } from "@/components/admin-torrents";
import { AdminP2Tools } from "@/components/admin-p2-tools";
import { AdminHr } from "@/components/admin-hr";
import { AdminInvites } from "@/components/admin-invites";
import { AdminUserLogs } from "@/components/admin-userlogs";
import { AdminAttendance } from "@/components/admin-attendance";
import { AdminTagDict } from "@/components/admin-tagdict";
import { AdminSections } from "@/components/admin-sections";
import { AdminMedals } from "@/components/admin-medals";
import { AdminSubtitlesAwards } from "@/components/admin-subtitles-awards";
import { AdminProps } from "@/components/admin-props";
import { AdminExams } from "@/components/admin-exams";
import { AdminJixiao } from "@/components/admin-jixiao";
import { AdminTasks } from "@/components/admin-tasks";
import { AdminTrackers } from "@/components/admin-trackers";
import { AdminOpsPanel } from "@/components/admin-ops";
import { AdminEmbedRules } from "@/components/admin-embed-rules";
import { HomeLayoutEditor } from "@/components/home-layout-editor";
import { FreeleechPanel, ClearCachePanel } from "./admin-freeleech";
import { AdminPromoKinds } from "./admin-promo-kinds";

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
  "emailbans", "testip", "stats", "cleanup", "ads", "notconnect", "uploaders",
  "agents", "polls", "dbstats", "syslog", "locations", "hrpardon", "plugins",
  "agentrules", "forums", "reports", "menu", "roles", "perm", "seedstats",
  "userfields", "pages", "terms", "donateorders", "fundings", "panelentries",
];

/** renderTool 的简单直排分流（复杂 case 由 page.tsx 内联处理） */
export function renderSimpleTool(
  tool: string,
  a: Record<string, string>,
  dict: ReturnType<typeof useI18n>["dict"],
) {
  switch (tool) {
    case "users":
      return <AdminUsers classes={dict.admin.classList} />;
    case "torrents":
      return <AdminTorrents />;
    case "content":
      return (
        <section className="nexus-detail">
          <h2 className="mb-3 text-base font-bold text-ink">
            {a.sectionContent ?? "content"}
          </h2>
          <ContentManage />
        </section>
      );
    case "freeleech":
      // 站方级促销 + 用户自购档位注册表（0213，同域两面板）
      return (
        <>
          <FreeleechPanel />
          <AdminPromoKinds />
        </>
      );
    case "clearcache":
      return <ClearCachePanel />;
    case "p2tools":
      return <AdminP2Tools />;
    // 第八轮 P3 套件（好学站后台逐页深挖落地）
    case "hr":
      return <AdminHr />;
    case "invites":
      return <AdminInvites />;
    case "userlogs":
      return <AdminUserLogs />;
    case "attendance":
      return <AdminAttendance />;
    case "tagdict":
      return <AdminTagDict />;
    case "sections":
      return <AdminSections />;
    case "medals":
      return <AdminMedals />;
    // 金字幕评选管理（0150）：候选/授金
    case "subawards":
      return <AdminSubtitlesAwards />;
    case "props":
      return <AdminProps />;
    case "exams":
      return <AdminExams />;
    case "jixiao":
      return <AdminJixiao />;
    case "tasks":
      return <AdminTasks />;
    case "trackers":
      return <AdminTrackers />;
    // 运维三件套（0078）：版本信息 / 备份面板 / 任务手动触发
    case "ops":
      return <AdminOpsPanel />;
    // 首页排版（0089）：板块顺序/宽度/显隐可视化编辑
    case "homelayout":
      return (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 font-display text-lg">
            {dict.homeLayout.title}
          </h2>
          <HomeLayoutEditor />
        </section>
      );
    // 视频内嵌规则管理（0189）
    case "embedrules":
      return <AdminEmbedRules />;
    default:
      return undefined;
  }
}
