import Link from "next/link";
import { EmptyState } from "@/components/ui/empty-state";
import { formatRatio } from "@/lib/format";
import { timeAgo } from "@/lib/time-ago";
import { fmt, fmtCur } from "@/i18n/config";
import type { Dict } from "@/i18n/server";
import type { ForumUserCard } from "@/lib/data-forum";
import type { ForumSide } from "@fluxtorrent/domain-types";

/** 论坛首页两翼（样图集 § forum：左列用户卡 / 右列四块）。
 *  纯 RSC——翻页、筛选、关注都走链接，这里没有任何客户端状态。
 *  `currency` 喂两处 {magic}：游客引导句、用户卡的魔力档名
 *  （站方可改货币名，故不能写死「魔力」）。 */

/** 右列取数状态。游客与「取档失败」必须分档：两者都渲染成空列表，
 *  就等于替站点说了两句假话（「今天没人发帖」/「论坛是空的」）。 */
export type SideState = "guest" | "ok" | "fail";

const g1 = (n: number) => n.toLocaleString();

/** 左列第一块：头像 + 昵称/等级 + 分享率·{magic}·做种。
 *  `me === null`（未登录或 /me 取档失败）→ 回落一句登录引导，
 *  不渲染 0 值假数据。注册日期样图有、`/me` 没回，故不摆。 */
export function ForumUserCardBlock({
  me,
  dict,
  currency,
}: {
  me: ForumUserCard | null;
  dict: Dict;
  currency: string;
}) {
  const t = dict.forums3;
  const my = dict.my;
  if (!me) {
    return (
      <section className="fh-user fh-user--anon">
        <Link href="/login" className="fh-user__anon">
          {fmtCur(t.anonUserHint, {}, currency)}
        </Link>
      </section>
    );
  }
  const stats: [string, string][] = [
    [my.ratio, formatRatio(me.uploaded, me.downloaded)],
    [
      fmtCur(dict.common.spark, {}, currency),
      me.spark === null ? "—" : g1(me.spark),
    ],
    [my.seedingLabel, g1(me.seeding)],
  ];
  return (
    <section className="fh-user" aria-label={t.userAria}>
      <div className="fh-user__row">
        <span className="fh-user__av">
          {me.avatar_url ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img src={me.avatar_url} alt="" />
          ) : (
            me.username.slice(0, 1).toUpperCase()
          )}
        </span>
        <div className="fh-user__id">
          <Link href="/me" className="fh-user__name">
            {me.username}
          </Link>
          <div className="fh-user__meta">
            {me.class_name ?? `${my.classLabel} —`}
          </div>
        </div>
      </div>
      <div className="fh-user__stats">
        {stats.map(([label, value]) => (
          <div key={label} className="fh-user__stat">
            <b className="num">{value}</b>
            <span>{label}</span>
          </div>
        ))}
      </div>
    </section>
  );
}

/** 左列「我的」面板（样图 § forum 左列第三块）：三条链接都指向关注流页
 *  的三个标签——那三个标签各有真数据源，没有一条是占位。 */
export function ForumMineLinks({ dict }: { dict: Dict }) {
  const t = dict.forums3;
  const links: [string, string][] = [
    ["/forums/feed", t.tabFeed],
    ["/forums/feed?tab=mine", t.tabMyTopics],
    ["/forums/feed?tab=replies", t.tabMyReplies],
  ];
  return (
    <section className="side-card">
      <h2 className="side-card__title">{t.mineTitle}</h2>
      <ul className="fh-mine">
        {links.map(([href, label]) => (
          <li key={href} className="fh-mine__row">
            <Link href={href} className="fh-mine__a">
              {label}
            </Link>
            <span className="fh-mine__caret" aria-hidden="true">
              →
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}

/** 右列四块：论坛统计 / 今日热帖 / 最新回复 / 新手必读。
 *  前三块的数据源是 GET /forums/side（按读权限过滤的跨版块聚合），
 *  第四块是静态引导（样图里那段文案本就不来自接口）。 */
export function ForumSideColumn({
  side,
  state,
  dict,
}: {
  side: ForumSide | null;
  state: SideState;
  dict: Dict;
}) {
  const t = dict.forums3;
  return (
    <aside className="fh-side" aria-label={t.statsTitle}>
      {state === "guest" && (
        <EmptyState
          compact
          icon="info"
          title={t.guestTitle}
          desc={t.guestDesc}
        />
      )}
      {state === "fail" && (
        <EmptyState
          compact
          tone="warn"
          icon="info"
          title={t.sideFailTitle}
          desc={t.sideFailDesc}
        />
      )}
      {state === "ok" && side && (
        <>
          <section className="side-card">
            <h2 className="side-card__title">{t.statsTitle}</h2>
            <dl className="fh-kv">
              <div className="fh-kv__row">
                <dt>{t.statTopics}</dt>
                <dd className="num">{g1(side.stats.topics)}</dd>
              </div>
              <div className="fh-kv__row">
                <dt>{t.statPosts}</dt>
                <dd className="num">{g1(side.stats.posts)}</dd>
              </div>
              <div className="fh-kv__row">
                <dt>{t.statToday}</dt>
                <dd className="num">{g1(side.stats.today_topics)}</dd>
              </div>
            </dl>
          </section>
          <section className="side-card">
            <h2 className="side-card__title">{t.hotTitle}</h2>
            {side.hot.length === 0 ? (
              <p className="side-empty">{t.emptyHot}</p>
            ) : (
              <ol className="fh-hot">
                {side.hot.map((h, i) => (
                  <li key={h.id} className="fh-hot__row">
                    <span className="fh-hot__rk num">{i + 1}</span>
                    <Link
                      href={`/forums/topic/${h.id}`}
                      className="fh-hot__tt"
                    >
                      {h.title}
                    </Link>
                    <span className="fh-hot__rc num">
                      {fmt(t.replyN, { n: h.replies })}
                    </span>
                  </li>
                ))}
              </ol>
            )}
          </section>
          <section className="side-card">
            <h2 className="side-card__title">{t.latestTitle}</h2>
            {side.latest_replies.length === 0 ? (
              <p className="side-empty">{t.emptyLatest}</p>
            ) : (
              <ul className="fh-lr">
                {side.latest_replies.map((r) => (
                  <li key={r.id} className="fh-lr__row">
                    <Link
                      href={`/forums/topic/${r.topic_id}`}
                      className="fh-lr__who"
                    >
                      {r.username ?? "—"}
                    </Link>
                    <span className="fh-lr__what">
                      {r.forum_name} · {r.topic_title}
                    </span>
                    <span className="fh-lr__time">
                      {timeAgo(r.created_at, dict.common)}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </section>
        </>
      )}
      <section className="fh-guide">
        <div className="fh-guide__t">{t.guideTitle}</div>
        <p className="fh-guide__d">{t.guideDesc}</p>
        <Link href="/rules" className="fh-guide__cta">
          {t.guideCta}
        </Link>
      </section>
    </aside>
  );
}
