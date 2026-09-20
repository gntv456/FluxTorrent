"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { IncrementBulk } from "@/components/increment-bulk";
import { StaffContentPanel } from "@/components/staff-tools-content";
import { StaffSecurityPanel } from "@/components/staff-tools-security";
import { StaffOpsPanel } from "@/components/staff-tools-ops";
import { StaffUsersPanel } from "@/components/staff-tools-users";
import { StaffSitePanel } from "@/components/staff-tools-site";
import { StaffSysPanel } from "@/components/staff-tools-sys";
import { StaffForumsPanel } from "@/components/staff-tools-forums";
import { StaffReportsPanel } from "@/components/staff-tools-reports";
import { StaffMenuPanel } from "@/components/staff-tools-menu";
import { StaffRolesPanel } from "@/components/staff-tools-roles";
import { StaffPermPanel } from "@/components/staff-tools-perm";
import { StaffSeedStatsPanel } from "@/components/staff-tools-seedstats";

/** staffpanel 管理工具落地页：FAQ 管理/规则管理/分类管理/封禁系统/批量邮件
 *  （hxpt faqmanage/modrules/catmanage/bans/massmail 口径，五个工具 tab）。
 *  300 行门禁已按域拆出十二个面板文件（本文件只留 tab 切换与共享 flash）：
 *  staff-tools-content（faq/rules/cats）、staff-tools-security（bans/mail/
 *  emailbans/testip）、staff-tools-ops（promo/staffmess/adduser）、
 *  staff-tools-users（warned/ipcheck/maxlogin/resetpass/deldisabled/hrpardon）、
 *  staff-tools-site（stats/cleanup/ads/notconnect/uploaders/agents/polls/plugins）、
 *  staff-tools-sys（dbstats/syslog/locations/agentrules）、
 *  staff-tools-forums、staff-tools-reports、staff-tools-menu、
 *  staff-tools-roles、staff-tools-perm、staff-tools-seedstats。 */

export type ToolTab =
  | "faq" | "rules" | "cats" | "bans" | "mail"
  | "promo" | "staffmess" | "adduser" | "incrementbulk" | "warned" | "ipcheck" | "maxlogin"
  | "resetpass" | "deldisabled" | "emailbans" | "testip" | "stats"
  | "cleanup" | "ads" | "notconnect" | "uploaders" | "agents" | "polls"
  | "dbstats" | "syslog" | "locations" | "hrpardon" | "plugins" | "agentrules"
  | "forums" | "reports" | "menu" | "roles" | "perm" | "seedstats";

export function StaffTools({ initialTab }: { initialTab?: ToolTab }) {
  const { dict, currency } = useI18n();
  const t = dict.stafftools;
  const [tab, setTab] = useState<ToolTab>(initialTab ?? "faq");
  // 左侧导航切换 tool 时父组件重渲染但本组件不卸载，useState 不会重新初始化 ——
  // 必须跟着 initialTab 同步，否则 URL 变了内容停在旧工具（「点了没反应」的根因）
  useEffect(() => {
    if (initialTab) setTab(initialTab);
  }, [initialTab]);
  // 权限清单（/me/perms）：Tab 按权限过滤——此前 34 个 Tab 对所有 staff 无差别渲染，
  // 无权限者点击后只见空面板（后端 2003 被 catch 静默吞掉）
  const [permKeys, setPermKeys] = useState<Set<string> | null>(null);
  useEffect(() => {
    api.get<{ perms: string[] }>("/api/v1/me/perms")
      .then((d) => setPermKeys(new Set(d.perms)))
      .catch(() => setPermKeys(null)); // 拉取失败不拦截渲染（退化为全量 Tab）
  }, []);
  const [msg, setMsg] = useState<string | null>(null);
  const flash = useCallback((m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 2500);
  }, []);

  const TAB_PERM: Partial<Record<ToolTab, string>> = {
    faq: "faq.manage", rules: "rules.manage", cats: "categories.manage",
    bans: "bans.manage", mail: "staffmess", adduser: "user.create",
    incrementbulk: "user.amountbonus", warned: "user.status", ipcheck: "ip.check",
    maxlogin: "maxlogin.view", resetpass: "user.resetpass", deldisabled: "user.delete_disabled",
    emailbans: "emailban.manage", testip: "testip", stats: "stats.view",
    cleanup: "cleanup.run", ads: "ads.manage", notconnect: "notconnectable.view",
    uploaders: "uploaders.view", agents: "agents.view", polls: "polls.manage",
    dbstats: "dbstats.view", syslog: "syslog.view", locations: "locations.manage",
    hrpardon: "hr.pardon", plugins: "plugins.manage",
    agentrules: "agents.view", forums: "forums.manage", reports: "appeal.handle",
    roles: "roles.manage", perm: "settings.manage", seedstats: "seed.stats.view",
  };

  const TABS: [ToolTab, string][] = [
    ["faq", t.tabFaq], ["rules", t.tabRules], ["cats", t.tabCats], ["bans", t.tabBans], ["mail", t.tabMail],
    ["promo", t.tabPromo], ["staffmess", t.tabStaffmess], ["adduser", t.tabAdduser],
    ["incrementbulk", "批量发放"], ["warned", t.tabWarned], ["ipcheck", t.tabIpcheck], ["maxlogin", t.tabMaxlogin],
    ["resetpass", t.tabResetpass], ["deldisabled", t.tabDeldisabled],
    ["emailbans", t.tabEmailbans], ["testip", t.tabTestip], ["stats", t.tabStats],
    ["cleanup", t.tabCleanup], ["ads", t.tabAds],
    ["notconnect", t.tabNotconnect], ["uploaders", t.tabUploaders], ["agents", t.tabAgents], ["polls", t.tabPolls],
    ["dbstats", t.tabDbstats], ["syslog", t.tabSyslog], ["locations", t.tabLocations],
    ["hrpardon", t.tabHrpardon],
    ["plugins", t.tabPlugins ?? "插件"],
    ["agentrules", dict.agentRules2?.tab ?? "客户端名单"],
    ["forums", "论坛版块"],
    ["reports", "举报处理"],
    ["menu", "导航菜单"],
    ["roles", "职务管理"],
    ["perm", "权限配置"],
    ["seedstats", "保种统计"],
  ];

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap gap-2" role="tablist">
        {TABS.filter(([k]) => !permKeys || !TAB_PERM[k] || permKeys.has(TAB_PERM[k]!)).map(([k, label]) => (
          <button key={k} role="tab" aria-selected={tab === k} onClick={() => setTab(k)}
            className={`min-h-[40px] rounded-full px-4 text-sm font-bold ${tab === k ? "bg-sky text-white" : "border border-line bg-[var(--surface-card)] text-sub"}`}>
            {label}
          </button>
        ))}
      </div>
      {msg && <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>}

      <StaffContentPanel tab={tab} flash={flash} />
      <StaffSecurityPanel tab={tab} flash={flash} />
      <StaffOpsPanel tab={tab} flash={flash} />
      <StaffUsersPanel tab={tab} flash={flash} />
      <StaffSitePanel tab={tab} flash={flash} />
      <StaffSysPanel tab={tab} flash={flash} />

      {tab === "incrementbulk" && <IncrementBulk />}

      {/* 魔力增减/上传量增减 已合并到「批量发放」（0065） */}
      {(tab === ("bonus" as ToolTab)) && (
        <section className="baozi-panel p-4">
          <h2 className="mb-1 text-base font-bold">已合并到「批量发放」</h2>
          <p className="mb-3 text-xs text-sub">
            魔力增减与上传量增减已合并为统一的批量发放工具：支持{currency} / 上传量 / 邀请 / 补签卡，
            可按等级、职务或指定用户批量执行，并群发 PM 通知。
          </p>
          <button
            className="min-h-[40px] rounded-full bg-sky px-5 text-sm font-bold text-white"
            onClick={() => { setTab("incrementbulk"); window.history.replaceState(null, "", "/admin?tool=incrementbulk"); }}
          >
            前往批量发放
          </button>
        </section>
      )}

      {tab === "forums" && <StaffForumsPanel flash={flash} />}
      {tab === "reports" && <StaffReportsPanel flash={flash} />}
      {tab === "menu" && <StaffMenuPanel flash={flash} />}
      {tab === "roles" && <StaffRolesPanel flash={flash} />}
      {tab === "perm" && <StaffPermPanel flash={flash} />}
      {tab === "seedstats" && <StaffSeedStatsPanel />}
    </div>
  );
}
