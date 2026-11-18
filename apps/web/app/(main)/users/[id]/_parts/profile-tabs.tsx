/**
 * 用户公开主页·标签页内容（从 app/(main)/users/[id]/page.tsx 按域拆出）：
 * renderTorrentList 种子列表 tab 表格、PostsTab 论坛动态、CommentsTab 种子评论。
 * TorrentListTable 是 server component；compact 行卡片态由 CSS
 * （.profilet 桌面表格 / .profilet-cards <768 双态同 DOM）承载，无 hooks。
 */

import Link from "next/link";
import type { Locale } from "@/i18n/config";
import { dateLocale } from "@/i18n/config";
import type { ProfileData, TorrentHistRow, Tab } from "./profile-types";

/** 种子列表 tab 渲染（每 tab 复用同一张表：名称/大小 + S/L；uploads 标注仅公开）
 *  M6.2：表格本体加 .profilet——<768 时 thead 隐藏、行转块状行卡片
 *  （两态同 DOM，server component 无需 useIsCompact）。 */
export function TorrentListTable({
  rows,
  emptyText,
  t,
  gb,
  publicOnly = false,
  withMeta = false,
  locale,
}: {
  locale: Locale;
  rows: TorrentHistRow[];
  emptyText: string;
  t: Record<string, string>;
  gb: (n: number) => string;
  publicOnly?: boolean;
  /** 「我的发布」这一路才带分类与发布时间（后端只对 uploads 投影） */
  withMeta?: boolean;
}) {
  const label = `${t.torrentName}${publicOnly ? `（${t.histPublicOnly}）` : ""}`;
  return (
    <div className="brand-wide-table-scroll">
      <table className="nexus-table profilet">
        <thead>
          <tr>
            <th scope="col" className="colhead">
              {label}
            </th>
            <th scope="col" className="colhead w-28 text-right">
              {t.colSize}
            </th>
            <th scope="col" className="colhead w-20 text-right">
              {t.colSL}
            </th>
            {withMeta && (
              <th scope="col" className="colhead w-20">
                {t.colCategory}
              </th>
            )}
            {withMeta && (
              <th scope="col" className="colhead w-24">
                {t.colTime}
              </th>
            )}
          </tr>
        </thead>
        <tbody>
          {rows.map((u) => (
            <tr key={u.torrent_id}>
              <td className="max-w-[420px] truncate">
                <span className="profilet__label">{label}</span>
                <Link
                  href={`/torrent/${u.torrent_id}`}
                  className="text-sky-deep hover:underline"
                >
                  {u.name}
                </Link>
              </td>
              <td className="num shrink-0 text-right text-sub">
                <span className="profilet__label">{t.colSize}</span>
                {gb(u.size)}
              </td>
              <td className="num shrink-0 text-right text-sub">
                <span className="profilet__label">{t.colSL}</span>
                {u.seeders} / {u.leechers}
              </td>
              {withMeta && (
                <td className="shrink-0 text-xs text-sub">
                  <span className="profilet__label">{t.colCategory}</span>
                  {u.category ?? "—"}
                </td>
              )}
              {withMeta && (
                <td className="shrink-0 text-xs text-sub">
                  <span className="profilet__label">{t.colTime}</span>
                  {u.published_at
                    ? new Date(u.published_at).toLocaleDateString(
                        dateLocale(locale),
                      )
                    : "—"}
                </td>
              )}
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td
                colSpan={withMeta ? 5 : 3}
                className="py-6 text-center text-sub"
              >
                {emptyText}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}

export function PostsTab({
  data,
  locale,
  t,
}: {
  data: ProfileData;
  locale: Locale;
  t: Record<string, string>;
}) {
  return (
    <section className="nexus-detail">
      <table className="nexus-table">
        <thead>
          <tr>
            <th scope="col" className="colhead">
              {t.recentPosts}
            </th>
          </tr>
        </thead>
        <tbody>
          {data.recent_posts.map((r) => (
            <tr key={r.id}>
              <td>
                <Link
                  href={`/forums/topic/${r.topic_id}`}
                  className="text-sm font-bold text-sky-deep"
                >
                  {r.topic_title}
                </Link>
                <p className="text-xs text-sub">
                  <Link
                    href={`/forums/${r.forum_id}`}
                    className="hover:underline"
                  >
                    {r.forum_name}
                  </Link>
                  {" · "}
                  {new Date(r.created_at).toLocaleString(dateLocale(locale))}
                  {" · "}
                  {t.colReplies} {r.replies}
                </p>
              </td>
            </tr>
          ))}
          {data.recent_posts.length === 0 && (
            <tr>
              <td className="py-6 text-center text-sub">{t.noPosts}</td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}

export function CommentsTab({
  data,
  locale,
  t,
}: {
  data: ProfileData;
  locale: Locale;
  t: Record<string, string>;
}) {
  return (
    <section className="nexus-detail">
      <table className="nexus-table">
        <thead>
          <tr>
            <th scope="col" className="colhead">
              {t.recentComments}
            </th>
          </tr>
        </thead>
        <tbody>
          {data.recent_comments.map((c, i) => (
            <tr key={i}>
              <td>
                <Link
                  href={`/torrent/${c.torrent_id}`}
                  className="text-xs font-bold text-sky"
                >
                  #{c.torrent_id}
                </Link>
                <p className="text-sm whitespace-pre-wrap">{c.body}</p>
                <p className="text-xs text-sub">
                  {new Date(c.created_at).toLocaleString(dateLocale(locale))}
                </p>
              </td>
            </tr>
          ))}
          {data.recent_comments.length === 0 && (
            <tr>
              <td className="py-6 text-center text-sub">{t.noComments}</td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}

/** B3-6：四组收纳页签导航（概览 / 我的资源 / 社区 / 成长）。
 *  分组标题由调用方从 i18n 注入（不写中文字面量）；原页签全部保留可达，
 *  默认中心页即「概览」组。SSR 链接切换 ?tab=，无客户端状态。 */
export interface TabMeta {
  key: Tab;
  label: string;
  count?: number;
}

/** 页签 → 分组序号：0 概览 / 1 我的资源 / 2 社区 / 3 成长 */
const TAB_GROUP: Record<Tab, number> = {
  center: 0,
  uploads: 1,
  seeding: 1,
  leeching: 1,
  completed: 1,
  incomplete: 1,
  preserved: 1,
  posts: 2,
  comments: 2,
  changes: 3,
};

export function ProfileTabGroups({
  tabMeta,
  active,
  uid,
  groupLabels,
  ariaLabel,
}: {
  tabMeta: TabMeta[];
  active: Tab;
  uid: number;
  groupLabels: string[];
  ariaLabel: string;
}) {
  return (
    <nav className="ux-tabgroups" aria-label={ariaLabel}>
      {groupLabels.map((label, gi) => {
        const items = tabMeta.filter((m) => TAB_GROUP[m.key] === gi);
        if (items.length === 0) return null;
        return (
          <section key={label} className="ux-tabgroup">
            <h2 className="ux-tabgroup__label">{label}</h2>
            <div className="ux-tabgroup__links">
              {items.map((m) => (
                <Link
                  key={m.key}
                  href={
                    m.key === "center"
                      ? `/users/${uid}`
                      : `/users/${uid}?tab=${m.key}`
                  }
                  className={`ux-tablink${
                    active === m.key ? " is-active" : ""
                  }`}
                  aria-current={active === m.key ? "page" : undefined}
                >
                  {/* 计数拼进单字符串，保持 SSR 与水合文本节点一致 */}
                  {`${m.label}${
                    typeof m.count === "number" ? ` (${m.count})` : ""
                  }`}
                </Link>
              ))}
            </div>
          </section>
        );
      })}
    </nav>
  );
}
