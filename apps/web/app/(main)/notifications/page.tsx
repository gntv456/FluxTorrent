import Link from "next/link";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { ErrorPanel } from "@/components/ui/error-panel";
import { PANEL_MD } from "@/lib/ui-classes";
import { timeAgo } from "@/lib/time-ago";

export const dynamic = "force-dynamic";

/**
 * 通知中心（0283 P1-9）：把散落三处的「要看的消息」聚合到一页——
 * 未读私信 / 管理组信箱 / H&R 待办（到期前 7 天内算紧急）。
 * 各面板是入口卡（数字 + 直达链接），不在本页复刻列表；
 * 例外是全站动态：0407 只建了读口、没有独立页面，所以本页在卡下方
 * 渲染最近 50 条正文，卡片锚点直达（原链接指 / 但首页没有动态流）。
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
interface ActivityRow {
  id: number;
  kind: string;
  text: string;
  created_at: string;
}

export default async function NotificationsPage() {
  const { dict } = await getDict();
  const results = await Promise.allSettled([
    api.get<OverviewResp>("/api/v1/me/overview"),
    api.get<HrResp>("/api/v1/me/hr"),
    api.get<StaffResp>("/api/v1/messages/staff"),
    // 全站动态（0407）：独立时间线（字幕上架/评选揭晓等系统事件）
    api.get<ActivityRow[]>("/api/v1/site-activity"),
  ]);
  const ov = results[0].status === "fulfilled" ? results[0].value : null;
  const hr = results[1].status === "fulfilled" ? results[1].value : null;
  const staff = results[2].status === "fulfilled" ? results[2].value : null;
  const activity = results[3].status === "fulfilled" ? results[3].value : null;
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
  // 该端点回的是数组（api.get 已拆信封），原代码按 {data: []} 取，
  // 于是这张卡无论有多少动态都不出现。
  const activities = Array.isArray(activity) ? activity : [];
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
    // 全站动态（0407）：独立流最近条数；无动态不渲染（不演空卡）
    ...(activities.length > 0
      ? [
          {
            href: "#site-activity",
            title: t.activityCard,
            count: activities.length,
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

      {activities.length > 0 && (
        <section id="site-activity" className={PANEL_MD}>
          <h2 className="font-bold">{t.activityCard}</h2>
          <ul className="mt-2 flex list-none flex-col gap-2 p-0">
            {activities.map((a) => (
              <li
                key={a.id}
                className={
                  "flex flex-wrap items-baseline justify-between " +
                  "gap-2 text-sm"
                }
              >
                <span className="min-w-0 flex-1">{a.text}</span>
                <time
                  className="shrink-0 text-xs text-sub"
                  dateTime={a.created_at}
                >
                  {timeAgo(a.created_at, dict.common)}
                </time>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}
