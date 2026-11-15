"use client";

import { useEffect, useState } from "react";
import { api, hasSessionCookie } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** /me/badges 返回口径（me_badges handler） */
interface ToolBadges {
  unread: number;
  tickets: number;
  friend_requests: number;
  staff: { reports: number; cheaters: number; staffbox: number } | null;
}

/** 工具行角标（/me/badges 口径）：UserTools 仅在下拉打开时挂载，
 *  mount 即取一次——其余计数只在打开瞬间有意义，不值得常驻轮询；
 *  null=未登录/取数失败，此时收件箱回落 /me 的 unread prop。 */
function useToolBadges(): ToolBadges | null {
  const [badges, setBadges] = useState<ToolBadges | null>(null);
  useEffect(() => {
    if (!hasSessionCookie()) return;
    api
      .get<ToolBadges>("/api/v1/me/badges")
      .then(setBadges)
      .catch(() => {});
  }, []);
  return badges;
}

/** 线性图标（24×24 stroke，仿同类站 userbar 图标语义） */
function Icon({ d, extra }: { d: string; extra?: string }) {
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.9"
      strokeLinecap="round"
      strokeLinejoin="round"
      className="userbar-tool__icon"
    >
      <path d={d} />
      {extra && <path d={extra} />}
    </svg>
  );
}

const ICONS = {
  inbox: { d: "M3 12 L7 6 H17 L21 12 V19 H3 Z M3 12 H9 A3 3 0 0 0 15 12 H21" },
  sent: { d: "M21 3 L10 14 M21 3 L14 21 L10 14 L3 10 Z" },
  cheaters: {
    d: "M8 11 A3.5 3.5 0 1 0 8 4 A3.5 3.5 0 1 0 8 11 M2.5 20 A5.5 5.5 0 0 1 13.5 20",
    extra: "M15.5 8.5 L21.5 14.5 M21.5 8.5 L15.5 14.5",
  },
  flag: {
    d: "M5 21 V4 M5 4 C8 2.5 11 5.5 14.5 4.5 C16.5 4 18 3.5 19 4 V12.5 C18 13 16.5 13.5 14.5 14 C11 15 8 12 5 13.5",
  },
  staff: {
    d: "M3 6 H21 V18 H3 Z M3 7 L12 13 L21 7 M8 18 L4.5 21.5 M16 18 L19.5 21.5",
  },
  social: {
    d: "M9 11 A3.5 3.5 0 1 0 9 4 A3.5 3.5 0 1 0 9 11 M2.5 20 A6.5 6.5 0 0 1 15.5 20 M16 4.5 A3.2 3.2 0 1 1 16.5 10.5 M17.5 14.5 A5.8 5.8 0 0 1 21.5 19.8",
  },
  rss: { d: "M5 11 A8.5 8.5 0 0 1 13.5 19.5 M5 5.5 A14 14 0 0 1 19 19.5" },
};

/** 角标（与收件箱同款样式；>99 显示 99+） */
function Badge({ n }: { n: number }) {
  if (n <= 0) return null;
  return (
    <span className="userbar-tool__badge num">
      {n > 99 ? "99+" : n}
    </span>
  );
}

/** 快捷工具条（同类站 userbar 口径）：收件箱/发件箱/作弊者/举报信箱/管理组信箱/社交/RSS。
 *  仅图标按钮，4+3 上下两行；RSS 直达获取RSS页（getrss.php 同款）。
 *  角标口径（/me/badges，0491）：每个工具渲染自己的可操作计数：
 *  收件箱=未读私信；管理组信箱=员工的未答队列、会员的工单待确认；
 *  举报信箱=待处理举报（员工）；作弊者=嫌疑会话（员工）；社交=好友申请。
 *  发件箱/RSS 无待办语义不设角标。staff=null（无权限）时员工侧数字
 *  整组不渲染，不演「队列清零」。 */
export function UserTools({
  unread = 0,
  mods = {},
}: {
  unread?: number;
  mods?: Record<string, boolean>;
}) {
  const mod = (k: string) => mods[k] !== false;
  const { dict } = useI18n();
  const t = dict.usertools;
  const badges = useToolBadges();
  const staff = badges?.staff ?? null;
  // 员工管未答队列；普通会员的待办是「已答复待确认」（确认入口在
  // /contactstaff 我的工单——staffbox 会员会被重定向）
  const staffBadge = staff ? staff.staffbox : (badges?.tickets ?? 0);

  return (
    <div className="usertools usertools--grid">
      {mod("messages") && (
      <a
        className="userbar-tool userbar-tool--icon"
        href="/messages"
        title={t.inbox}
        aria-label={t.inbox}
      >
        <span className="relative inline-flex">
          <Icon {...ICONS.inbox} />
          <Badge n={badges ? badges.unread : unread} />
        </span>
      </a>
      )}
      {mod("messages") && (
      <a
        className="userbar-tool userbar-tool--icon"
        href="/messages?box=sent"
        title={t.sentbox}
        aria-label={t.sentbox}
      >
        <Icon {...ICONS.sent} />
      </a>
      )}
      <a
        className="userbar-tool userbar-tool--icon"
        href="/cheaterbox"
        title={t.cheaters}
        aria-label={t.cheaters}
      >
        <span className="relative inline-flex">
          <Icon {...ICONS.cheaters} />
          {staff && <Badge n={staff.cheaters} />}
        </span>
      </a>
      <a
        className="userbar-tool userbar-tool--icon"
        href="/reports"
        title={t.reportBox}
        aria-label={t.reportBox}
      >
        <span className="relative inline-flex">
          <Icon {...ICONS.flag} />
          {staff && <Badge n={staff.reports} />}
        </span>
      </a>
      {mod("messages") && (
      <a
        className="userbar-tool userbar-tool--icon"
        href="/staffbox"
        title={t.staffBox}
        aria-label={t.staffBox}
      >
        <span className="relative inline-flex">
          <Icon {...ICONS.staff} />
          <Badge n={staffBadge} />
        </span>
      </a>
      )}
      {mod("friends") && (
      <a
        className="userbar-tool userbar-tool--icon"
        href="/friends"
        title={t.socialList}
        aria-label={t.socialList}
      >
        <span className="relative inline-flex">
          <Icon {...ICONS.social} />
          <Badge n={badges?.friend_requests ?? 0} />
        </span>
      </a>
      )}
      <a
        className="userbar-tool userbar-tool--icon"
        href="/getrss"
        title={t.getRss}
        aria-label={t.getRss}
      >
        <Icon {...ICONS.rss} />
      </a>
    </div>
  );
}
