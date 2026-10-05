import Link from "next/link";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

/**
 * 通知中心（0283 P1-9）：把散落三处的「要看的消息」聚合到一页——
 * 未读私信 / 管理组信箱 / H&R 待办（到期前 7 天内算紧急）。
 * 各面板是入口卡（数字 + 直达链接），不在本页复刻列表。
 */

interface OverviewResp {
  unread_messages?: number;
}
interface HrResp {
  rows?: {
    torrent_id: number;
    torrent_name: string;
    deadline: string;
    status: string;
    seeded_seconds: number;
    required_seconds: number;
  }[];
}
interface StaffResp {
  data?: unknown[] | { items?: unknown[] };
}

export default async function NotificationsPage() {
  const { dict } = await getDict();
  const [ov, hr, staff] = await Promise.all([
    api.get<OverviewResp>("/api/v1/me/overview").catch(() => null),
    api.get<HrResp>("/api/v1/me/hr").catch(() => null),
    api.get<StaffResp>("/api/v1/messages/staff").catch(() => null),
  ]);
  const unread = ov?.unread_messages ?? 0;
  const staffRows = Array.isArray(staff?.data)
    ? staff!.data!
    : (staff?.data?.items ?? []);
  const staffUnread = staffRows.length;
  const week = Date.now() + 7 * 86400_000;
  const hrOpen = (hr?.rows ?? []).filter((r) => r.status === "open");
  const hrUrgent = hrOpen.filter(
    (r) => new Date(r.deadline).getTime() < week,
  ).length;
  const t = dict.notifications;

  const cards = [
    {
      href: "/messages?unread=true",
      title: t.pmCard,
      count: unread,
      tone: unread > 0 ? "hot" : "idle",
    },
    {
      href: "/staffbox",
      title: t.staffCard,
      count: staffUnread,
      tone: staffUnread > 0 ? "warm" : "idle",
    },
    {
      href: "/myhr",
      title: t.hrCard,
      count: hrOpen.length,
      extra:
        hrUrgent > 0 ? t.hrUrgent.replace("{n}", String(hrUrgent)) : undefined,
      tone: hrUrgent > 0 ? "hot" : hrOpen.length > 0 ? "warm" : "idle",
    },
  ];
  const total = unread + staffUnread + hrUrgent;

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{t.title}</h1>
      <p className="text-sm text-muted">
        {total > 0 ? t.summary.replace("{n}", String(total)) : t.allClear}
      </p>
      <div className="grid gap-3 sm:grid-cols-3">
        {cards.map((c) => (
          <Link
            key={c.href}
            href={c.href}
            className={`ntf-card ntf-card--${c.tone}`}
          >
            <span className="ntf-card__title">{c.title}</span>
            <span className="ntf-card__count">{c.count}</span>
            {c.extra && <span className="ntf-card__extra">{c.extra}</span>}
          </Link>
        ))}
      </div>
    </div>
  );
}
