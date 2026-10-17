import Link from "next/link";
import { redirect } from "next/navigation";
import { api } from "@/lib/api-client";
import { getMyFollows, getMyBookmarks } from "@/lib/data";
import { getDict } from "@/i18n/server";
import { BTN_SM_BOLD, PANEL_MD } from "@/lib/ui-classes";
import { UnsubscribeButton } from "@/components/unsubscribe-button";
import { FollowButton } from "@/components/forum-follow";
import { MyBookmarks } from "@/components/my-bookmarks";
import { PageHeader } from "@/components/ui/page-header";
import { TabPanel, Tabs } from "@/components/ui/tabs";
import { EmptyState } from "@/components/ui/empty-state";

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

/** 标签白名单。 */
const TABS = ["groups", "authors", "bookmarks"] as const;

/** 追更中心（0328 剧集批补齐前端）。
 *  后端两读口此前无入口：/me/subscriptions/groups（进度）、
 *  /me/subscriptions/calendar（近 14 天按天分组）。本页合并成
 *  「我在追的剧集」+「更新日历」两栏，退订走 client 组件。
 *  未登录跳 /login（与 /me 同款：redirect 必须在 try 外）。
 *
 *  本轮对齐样图 § subscriptions：两栏上下堆叠改成 `?tab=` 标签档
 *  （资源组 / 我关注的作者 / 我的收藏），页头补「添加订阅」入口——订阅动作在
 *  种子页发起，站内没有独立的新增路由，所以入口指向 /torrents。 */
export default async function SubscriptionsPage({
  searchParams,
}: {
  searchParams: Promise<{ tab?: string }>;
}) {
  const sp = await searchParams;
  const tab = (TABS as readonly string[]).includes(sp.tab ?? "")
    ? (sp.tab as (typeof TABS)[number])
    : "groups";
  const { dict } = await getDict();
  const t = dict.subscriptions;

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
  // 关注名单只在作者标签下才取：`getMyFollows` 自己回落空数组，
  // 不 throw，所以不会把上面那条 redirect 的语义搅浑。
  const follows = tab === "authors" ? await getMyFollows() : null;
  const authors = follows?.users ?? [];
  // 收藏清单同理只在第三档取；helper 回 null = 读失败（≠「还没收藏」）
  const bookmarks = tab === "bookmarks" ? await getMyBookmarks() : null;

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        eyebrow={`${dict.my.title} / ${t.title}`}
        title={t.title}
        sub={t.subtitle}
        aside={
          <Link href="/torrents" className={BTN_SM_BOLD}>
            {t.addSub}
          </Link>
        }
      />

      <Tabs
        idPrefix="subs"
        label={t.title}
        value={tab}
        items={[
          { key: "groups", label: t.tabGroups },
          { key: "authors", label: t.tabAuthors },
          { key: "bookmarks", label: t.tabBookmarks },
        ]}
        hrefs={[
          "/me/subscriptions",
          "/me/subscriptions?tab=authors",
          "/me/subscriptions?tab=bookmarks",
        ]}
      />

      {tab === "groups" && (
        <TabPanel idPrefix="subs" tabKey="groups">
          <div className="flex flex-col gap-6">
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
                      <div className="text-xs font-bold text-sub">
                        {d.date}
                      </div>
                      <ul className="flex list-none flex-col gap-1 p-0">
                        {d.items.map((it) => (
                          <li
                            key={it.torrent_id}
                            data-torrent-id={it.torrent_id}
                          >
                            <Link
                              href={`/torrent/${it.torrent_id}`}
                              className={
                                "flex items-baseline gap-2 text-sm " +
                                "hover:text-[var(--sky-deep)]"
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
        </TabPanel>
      )}

      {tab === "authors" && (
        <TabPanel idPrefix="subs" tabKey="authors">
          {authors.length === 0 ? (
            <EmptyState
              icon="users"
              title={t.authorsEmpty}
              desc={t.authorsEmptyHint}
            />
          ) : (
            <div className="baozi-panel overflow-x-auto">
              <table className="nexus-table">
                <thead>
                  <tr>
                    <td className="colhead">{t.colAuthor}</td>
                    <td className="colhead w-32 text-right">{t.colAction}</td>
                  </tr>
                </thead>
                <tbody>
                  {authors.map((u) => (
                    <tr key={u.id}>
                      <td>
                        <Link
                          href={`/users/${u.id}`}
                          className="font-bold text-ink"
                        >
                          {u.name}
                        </Link>
                      </td>
                      <td className="text-right">
                        {/* initial 直接告知「已关注」：名单本身就来自
                            /follows/mine，再逐行打一次 status 是白问 */}
                        <FollowButton
                          targetType="user"
                          targetId={u.id}
                          initial={{
                            following: true,
                            followers: 0,
                            is_self: false,
                          }}
                        />
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </TabPanel>
      )}

      {tab === "bookmarks" && (
        <TabPanel idPrefix="subs" tabKey="bookmarks">
          <MyBookmarks rows={bookmarks} />
        </TabPanel>
      )}
    </div>
  );
}
