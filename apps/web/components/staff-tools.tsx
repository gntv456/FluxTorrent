"use client";

import { useCallback, useEffect, useState } from "react";
import { useI18n } from "@/i18n/client";
import { IncrementBulk } from "@/components/increment-bulk";
import { StaffContentPanel } from "@/components/staff-tools-content";
import { StaffSecurityPanel } from "@/components/staff-tools-security";
import { StaffOpsPanel } from "@/components/staff-tools-ops";
import { StaffUsersPanel } from "@/components/staff-tools-users";
import { StaffSitePanel } from "@/components/staff-tools-site";
import { StaffSysPanel } from "@/components/staff-tools-sys";
import { StaffReportsPanel } from "@/components/staff-tools-reports";
import { StaffMenuPanel } from "@/components/staff-tools-menu";
import { StaffRolesPanel } from "@/components/staff-tools-roles";
import { StaffPermPanel } from "@/components/staff-tools-perm";
import { StaffSeedStatsPanel } from "@/components/staff-tools-seedstats";
import { StaffUserFieldsPanel } from "@/components/staff-user-fields";
import { StaffCustomPagesPanel } from "@/components/staff-custom-pages";
import {
  AdminDonateOrders,
  AdminPanelEntries,
} from "@/components/admin-donate-orders";
import { AdminFundings } from "@/components/admin-fundings";
import { StaffTermsPanel } from "@/components/staff-terms";

/** staffpanel 管理工具落地页：FAQ 管理/规则管理/分类管理/封禁系统/批量邮件等
 *  工具面板的**分发器**（hxpt faqmanage/modrules/catmanage/bans/massmail 口径）。
 *  300 行门禁已按域拆出十二个面板文件（本文件只留分发与共享 flash）：
 *  staff-tools-content（faq/rules/cats）、staff-tools-security（bans/mail/
 *  emailbans/testip）、staff-tools-ops（staffmess/adduser）、
 *  staff-tools-users（warned/ipcheck/maxlogin/resetpass/deldisabled/hrpardon）、
 *  staff-tools-site（stats/cleanup/ads/notconnect/uploaders/agents/polls/plugins）、
 *  staff-tools-sys（dbstats/syslog/locations/agentrules）、
 *  staff-tools-reports、staff-tools-menu、
 *  staff-tools-roles、staff-tools-perm、staff-tools-seedstats。
 *  论坛结构已迁至独立页 app/(main)/admin/forums（见 ForumsRedirect）。
 *
 *  ⚠️ 导航唯一来源是 AdminShell 的左侧导航（数据来自 `staff_panel_entries`）。
 *  本组件此前自带一行 35 个药丸按钮，与左侧导航同屏重复，已移除。
 *  另：域面板**必须按 tab 条件挂载**——早先 6 个面板无条件渲染，各自的
 *  useEffect 会把全部管理端点拉一遍，实测单个工具页触发 31 个请求
 *  （29 个不同端点），其中只有 4 个与该工具有关。
 */

export type ToolTab =
  | "faq"
  | "rules"
  | "cats"
  | "bans"
  | "mail"
  | "staffmess"
  | "adduser"
  | "incrementbulk"
  | "warned"
  | "ipcheck"
  | "maxlogin"
  | "resetpass"
  | "deldisabled"
  | "emailbans"
  | "testip"
  | "stats"
  | "cleanup"
  | "ads"
  | "notconnect"
  | "uploaders"
  | "agents"
  | "polls"
  | "dbstats"
  | "syslog"
  | "locations"
  | "hrpardon"
  | "plugins"
  | "agentrules"
  | "forums"
  | "reports"
  | "menu"
  | "roles"
  | "perm"
  | "seedstats"
  | "userfields"
  | "pages"
  | "terms"
  | "donateorders"
  | "fundings"
  | "panelentries";

/** 各域面板负责的 tab —— 既用于条件挂载，也是「谁渲染谁」的唯一声明处。
 *  新增工具时在这里归位，并同步 STAFF_TOOL_TABS（admin-tool-switch.tsx）
 *  与 staff_panel_entries.tab_key。 */
const CONTENT_TABS: ToolTab[] = ["faq", "rules", "cats"];
const SECURITY_TABS: ToolTab[] = ["bans", "mail", "emailbans", "testip"];
const OPS_TABS: ToolTab[] = ["staffmess", "adduser"];
const USERS_TABS: ToolTab[] = [
  "warned",
  "ipcheck",
  "maxlogin",
  "resetpass",
  "deldisabled",
  "hrpardon",
];
const SITE_TABS: ToolTab[] = [
  "stats",
  "cleanup",
  "notconnect",
  "uploaders",
  "agents",
  "polls",
  "ads",
  "plugins",
  "userfields",
  "pages",
  "terms",
  "donateorders",
  "fundings",
  "panelentries",
];
const SYS_TABS: ToolTab[] = ["dbstats", "syslog", "locations", "agentrules"];

/** 论坛结构已升格为独立整页 `/admin/forums`（左分区树 + 右版块表同屏，
 *  替代原先挤在一个 tab 里的表格 + 两个堆叠表单）。
 *  这里只做重定向，让 `?tool=forums` 的旧书签与深链不失效。 */
function ForumsRedirect() {
  const { dict } = useI18n();
  useEffect(() => {
    window.location.replace("/admin/forums");
  }, []);
  return (
    <p className="py-8 text-center text-sm text-sub">
      {dict.stafftools.forumsRedirect}
    </p>
  );
}

export function StaffTools({ initialTab }: { initialTab?: ToolTab }) {
  const { dict } = useI18n();
  // URL 是唯一真相：AdminShell 切工具会重设 ?tool= 并让本组件重渲染。
  // 早先从 initialTab 拷进 useState 再同步，多一份可能与 URL 脱节的本地状态。
  const tab: ToolTab = initialTab ?? "faq";
  const [msg, setMsg] = useState<string | null>(null);
  const flash = useCallback((m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 2500);
  }, []);

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}

      {CONTENT_TABS.includes(tab) && (
        <StaffContentPanel tab={tab} flash={flash} />
      )}
      {SECURITY_TABS.includes(tab) && (
        <StaffSecurityPanel tab={tab} flash={flash} />
      )}
      {OPS_TABS.includes(tab) && <StaffOpsPanel tab={tab} flash={flash} />}
      {USERS_TABS.includes(tab) && (
        <StaffUsersPanel tab={tab} flash={flash} />
      )}
      {SITE_TABS.includes(tab) && <StaffSitePanel tab={tab} flash={flash} />}
      {SYS_TABS.includes(tab) && <StaffSysPanel tab={tab} flash={flash} />}

      {tab === "incrementbulk" && <IncrementBulk />}
      {tab === "forums" && <ForumsRedirect />}
      {tab === "reports" && <StaffReportsPanel flash={flash} />}
      {tab === "menu" && <StaffMenuPanel flash={flash} />}
      {tab === "roles" && <StaffRolesPanel flash={flash} />}
      {tab === "perm" && <StaffPermPanel flash={flash} />}
      {tab === "seedstats" && <StaffSeedStatsPanel />}
      {tab === "userfields" && <StaffUserFieldsPanel flash={flash} />}
      {tab === "pages" && <StaffCustomPagesPanel flash={flash} />}
      {tab === "donateorders" && <AdminDonateOrders />}
      {tab === "fundings" && <AdminFundings />}
      {tab === "panelentries" && <AdminPanelEntries />}
      {tab === "terms" && <StaffTermsPanel flash={flash} />}
    </div>
  );
}
