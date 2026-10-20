import Link from "next/link";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { ErrorPanel } from "@/components/ui/error-panel";

export const dynamic = "force-dynamic";

/**
 * 通知中心（0283 P1-9）：把散落三处的「要看的消息」聚合到一页——
 * 未读私信 / 管理组信箱 / H&R 待办（到期前 7 天内算紧急）。
 * 各面板是入口卡（数字 + 直达链接），不在本页复刻列表。
 *
 * 四态（W-28）：三路全挂（含未登录 401）→ ErrorPanel 整页错误（原实现
 * 恒显三张 0 卡，对着游客演「一切安好」）；任一路成功即正常渲染，
 * 单路失败按原口径静默降级（staff 403 = 卡片不渲染）。加载态由
 * loading.tsx 承担；「全部清零」是成功态文案（allClear），空态不适用。
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
interface HomeResp {
  news?: { id: number; title: string; badge?: string }[];
}

export default async function NotificationsPage() {
  const { dict } = await getDict();
  const results = await Promise.allSettled([
    api.get<OverviewResp>("/api/v1/me/overview"),
    api.get<HrResp>("/api/v1/me/hr"),
    api.get<StaffResp>("/api/v1/messages/staff"),
    // 全站动态（0405）：首页公告区最近条目（含字幕上架/评选揭晓播报）
    api.get<HomeResp>("/api/v1/home"),
  ]);
  const ov = results[0].status === "fulfilled" ? results[0].value : null;
  const hr = results[1].status === "fulfilled" ? results[1].value : null;
  const staff = results[2].status === "fulfilled" ? results[2].value : null;
  const home = results[3].status === "fulfilled" ? results[3].value : null;
  const t = dict.notifications;

  // 三路全挂 = 会话失效或 API 整体故障：数字全是演的，不如诚实报错
  if (ov === null && hr === null && staff === null) {
    const firstErr = results.find(
      (r) => r.status === "rejected",
    ) as PromiseRejectedResult | undefined;
    return (
      <div className="flex flex-col gap-4">
        <h1 className="font-display text-2xl">{t.title}</h1>
        <ErrorPanel
          error={firstErr?.reason ?? { message: dict.common.loadFailed }}
          retryHref="/notifications"
          desc={t.errorDesc}
        />
      </div>
    );
  }

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

  const cards = [
    {
      href: "/messages?unread=true",
      title: t.pmCard,
      count: unread,
      tone: unread > 0 ? "hot" : "idle",
    },
    // 管理组信箱只对进得去 /staffbox 的人存在：成员那边接口 403 → staff=null，
    // 这张卡此前照样渲染一个恒 0 的数字，等于对着成员演「信箱是空的」。
    ...(staff !== null
      ? [
          {
            href: "/staffbox",
            title: t.staffCard,
            count: staffUnread,
            tone: staffUnread > 0 ? ("warm" as const) : ("idle" as const),
          },
        ]
      : []),
    // 全站动态（0405）：公告区最近一条直达首页；无动态不渲染（不演空卡）
    ...(home?.news?.length
      ? [
          {
            href: "/",
            title: t.activityCard,
            count: home.news.length,
            tone: ("idle" as const),
          },
        ]
      : []),
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
