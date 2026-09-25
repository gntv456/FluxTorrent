"use client";

import { useI18n } from "@/i18n/client";

/** 线性图标（24×24 stroke，仿好学站 userbar 图标语义） */
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

/** 快捷工具条（好学站 userbar 口径）：收件箱/发件箱/作弊者/举报信箱/管理组信箱/社交/RSS。
 *  仅图标按钮，4+3 上下两行；RSS 直达获取RSS页（getrss.php 同款）。
 *  收件箱按钮带未读数角标（NP userbar 邮箱图标同款）。 */
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
          {unread > 0 && (
            <span className="userbar-tool__badge num">
              {unread > 99 ? "99+" : unread}
            </span>
          )}
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
        <Icon {...ICONS.cheaters} />
      </a>
      <a
        className="userbar-tool userbar-tool--icon"
        href="/reports"
        title={t.reportBox}
        aria-label={t.reportBox}
      >
        <Icon {...ICONS.flag} />
      </a>
      {mod("messages") && (
      <a
        className="userbar-tool userbar-tool--icon"
        href="/staffbox"
        title={t.staffBox}
        aria-label={t.staffBox}
      >
        <Icon {...ICONS.staff} />
      </a>
      )}
      {mod("friends") && (
      <a
        className="userbar-tool userbar-tool--icon"
        href="/friends"
        title={t.socialList}
        aria-label={t.socialList}
      >
        <Icon {...ICONS.social} />
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
