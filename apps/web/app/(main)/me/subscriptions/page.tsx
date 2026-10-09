import Link from "next/link";
import { redirect } from "next/navigation";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { PANEL_MD } from "@/lib/ui-classes";
import { UnsubscribeButton } from "@/components/unsubscribe-button";

export const dynamic = "force-dynamic";

const ROW_CLS =
  "flex flex-wrap items-baseline justify-between gap-2 " +
  "rounded-lg border border-line bg-card px-4 py-3";

interface SubRow {
  group_id: number;
  name: string;
  latest_episode: number | null;
  last_update: string | null;
  releases: number;
}

interface CalendarDay {
  date: string;
  items: {
    torrent_id: number;
    name: string;
    group_id: number;
    episode: number | null;
  }[];
}

/** 追更中心（0328 剧集批补齐前端）。
 *  后端两读口此前无入口：/me/subscriptions/groups（进度）、
 *  /me/subscriptions/calendar（近 14 天按天分组）。本页合并成
 *  「我在追的剧集」+「更新日历」两栏，退订走 client 组件。
 *  未登录跳 /login（与 /me 同款：redirect 必须在 try 外）。 */
export default async function SubscriptionsPage() {
  let rows: SubRow[] = [];
  let days: CalendarDay[] = [];
  let failed = false;
  try {
    const r = await api.get<SubRow[]>("/api/v1/me/subscriptions/groups");
    rows = Array.isArray(r) ? r : [];
    const c = await api.get<CalendarDay[]>(
      "/api/v1/me/subscriptions/calendar",
    );
    days = Array.isArray(c) ? c : [];
  } catch {
    failed = true;
  }
  if (failed) redirect("/login?next=/me/subscriptions");
  const { dict } = await getDict();
  const t = dict.subscriptions;

  return (
    <div className="flex flex-col gap-6">
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">Subscriptions</div>
          <h1 className="font-display text-2xl">{t.title}</h1>
        </div>
        <span className="sub">{t.subtitle}</span>
      </div>

      <section className={PANEL_MD} data-sub-list>
        <h2 className="font-bold">{t.title}</h2>
        {rows.length === 0 ? (
          <p className="py-6 text-center text-sub">{t.empty}</p>
        ) : (
          <ul className="mt-2 flex list-none flex-col gap-2 p-0">
            {rows.map((r) => (
              <li key={r.group_id} data-group-id={r.group_id}>
                <div className={ROW_CLS}>
                  <Link
                    href={`/torrents?group=${r.group_id}`}
                    className="min-w-0 flex-1 truncate font-bold"
                  >
                    {r.name}
                  </Link>
                  <span className="text-xs text-sub">
                    {t.latestEp}
                    {r.latest_episode ?? "-"}
                    {t.epLabel}
                    {" · "}
                    {t.lastUpdate} {r.last_update ?? "-"}
                    {" · "}
                    {t.releases} {r.releases}
                  </span>
                  <UnsubscribeButton groupId={r.group_id} />
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className={PANEL_MD} data-sub-calendar>
        <h2 className="font-bold">{t.calendarTitle}</h2>
        {days.length === 0 ? (
          <p className="py-6 text-center text-sub">{t.calendarEmpty}</p>
        ) : (
          <div className="mt-2 flex flex-col gap-3">
            {days.map((d) => (
              <div key={d.date} data-day={d.date}>
                <div className="text-xs font-bold text-sub">{d.date}</div>
                <ul className="flex list-none flex-col gap-1 p-0">
                  {d.items.map((it) => (
                    <li key={it.torrent_id} data-torrent-id={it.torrent_id}>
                      <Link
                        href={`/torrent/${it.torrent_id}`}
                        className={
                          "flex items-baseline gap-2 text-sm " +
                          "hover:text-sky"
                        }
                      >
                        <span className="min-w-0 flex-1 truncate">
                          {it.name}
                        </span>
                        {it.episode !== null && (
                          <span className="shrink-0 text-xs text-sub">
                            {it.episode}
                            {t.epLabel}
                          </span>
                        )}
                      </Link>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
