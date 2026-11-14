import Link from "next/link";
import { ForumTopicRow } from "@/components/forum-topic-row";
import { topicStamp } from "@/lib/topic-stamp";
import type { Dict } from "@/i18n/server";
import type { ForumRecentTopic } from "@fluxtorrent/domain-types";

/** 论坛首页中栏话题流（样图集 § tplForum 中列）。
 *
 *  纯 RSC——筛选走 SSR 链接（?type=sticky|digest），无客户端状态；
 *  版主任免等管理动作在版块页，首页只做浏览入口。
 *
 *  行样式已收编到 forum-topic-row.tsx（与版块页主题列表同一套
 *  fh-topic 行式），本组件只装配首页特有内容：版块徽章 + 热度标 +
 *  最后回复人右列。
 *
 *  两处按真站口径做的取舍：
 *   - 版块徽章配色按版块 id 散列到固定色板（样图按版块名写死了六色，
 *     真站版块动态增长，不能建一张永远追不上的映射表）；
 *   - 筛选行只有 全部/置顶/精华 三档——本站 topic_type 没有「公告」
 *     这一类型，不为一枚装饰徽章凭空加字段（与版块页同一口径）。
 */

/** 版块徽章色板（样图 bs 映射的六对，打散按 id 取）。
 *  P2-5 令牌化：色值真源在 base.css --fbadge-*（固定 hue 轮换，
 *  明暗同值，口径见该处注释）。 */
const BADGE_PAIRS: [string, string][] = [
  ["bg-[var(--fbadge-sky-bg)]", "text-[var(--fbadge-sky-fg)]"],
  ["bg-[var(--fbadge-mint-bg)]", "text-[var(--fbadge-mint-fg)]"],
  ["bg-[var(--fbadge-sun-bg)]", "text-[var(--fbadge-sun-fg)]"],
  ["bg-[var(--fbadge-indigo-bg)]", "text-[var(--fbadge-indigo-fg)]"],
  ["bg-[var(--fbadge-candy-bg)]", "text-[var(--fbadge-candy-fg)]"],
  ["bg-[var(--fbadge-coral-bg)]", "text-[var(--fbadge-coral-fg)]"],
];

const badgePair = (id: number) => BADGE_PAIRS[id % BADGE_PAIRS.length];

/** 热度标（样图 hot-tag「热 1.2k」）：**判定在后端**——阈值读管理端
 *  设置 site_settings.forum_hot_views（0397，0 = 关闭），接口回 hot 布尔；
 *  这里只拼文案与数量：过千折成 1.2k，未过千显示原数（阈值调低时
 *  98 次浏览显示「热 98」而不是「热 0.1k」）。 */
const hotTag = (views: number, label: string, hot: boolean) => {
  if (!hot) return null;
  const n = views >= 1000 ? `${(views / 1000).toFixed(1)}k` : String(views);
  return `${label} ${n}`;
};

export type StreamType = "notice" | "sticky" | "digest" | undefined;

export function ForumHomeStream({
  topics,
  type,
  dict,
}: {
  topics: ForumRecentTopic[];
  /** 当前筛选（控制筛选行高亮；undefined = 全部） */
  type: StreamType;
  dict: Dict;
}) {
  const t = dict.forums;
  // 样图筛选行四档：全部话题 / 只看公告 / 只看置顶 / 只看精华
  const filters: [StreamType, string][] = [
    [undefined, t.filterAll],
    ["notice", t.filterNotice],
    ["sticky", t.filterSticky],
    ["digest", t.filterDigest],
  ];

  return (
    <div className="fh-stream">
      {/* 筛选行：SSR 链接，与版块页 type 筛选同口径 */}
      <div className="fh-filter">
        {filters.map(([val, label]) => {
          const on = val === type;
          // 圆点色（样图）：全部 sky / 公告 coral / 置顶 indigo / 精华 sun
          const dot =
            val === "notice"
              ? "bg-coral"
              : val === "sticky"
                ? "bg-indigo"
                : val === "digest"
                  ? "bg-sun"
                  : "bg-sky";
          return (
            <Link
              key={val ?? "all"}
              href={val ? `/forums?type=${val}` : "/forums"}
              aria-current={on ? "true" : undefined}
              className={on ? "on" : undefined}
            >
              <span className={`fh-filter__dot ${dot}`} aria-hidden="true" />
              {label}
            </Link>
          );
        })}
      </div>

      <div className="fh-stream__list">
        <div className="fh-stream__head">
          <b>{t.colTopic}</b>
          <span className="fh-stream__sp" />
          <span className="fh-stream__col-last">{t.colLast}</span>
        </div>
        {topics.length === 0 ? (
          <p className="fh-stream__empty">{t.noTopics}</p>
        ) : (
          topics.map((x) => {
            const [bb, bf] = badgePair(x.forum_id);
            const last = x.last_replier ?? x.username ?? "—";
            return (
              <ForumTopicRow
                key={x.id}
                topic={x}
                marks={{
                  notice: t.markNotice,
                  pin: t.markPin,
                  ess: t.markEss,
                  locked: t.locked,
                }}
                slots={{
                  board: (
                    <span className={`fh-topic__board ${bb} ${bf}`}>
                      {x.forum_name}
                    </span>
                  ),
                  afterTitle: (() => {
                    const hot = hotTag(x.views, t.hot, Boolean(x.hot));
                    return hot ? (
                      <span className="fh-topic__hot">{hot}</span>
                    ) : null;
                  })(),
                  // 发布时间（recent 接口回 created_at）
                  metaTime: x.created_at ? topicStamp(x.created_at) : null,
                  // 回复列已去掉（用户要求）；最后回复不再带「最后回复」
                  // 四个字，只留回复人与时间
                  right: (
                    <div className="fh-topic__last">
                      <b>{last}</b>
                      <small suppressHydrationWarning>
                        {topicStamp(x.last_post_at) ?? ""}
                      </small>
                    </div>
                  ),
                }}
              />
            );
          })
        )}
      </div>
    </div>
  );
}
