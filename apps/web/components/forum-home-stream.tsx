import Link from "next/link";
import { MarkBadge } from "@/components/forum-bits";
import type { Dict } from "@/i18n/server";
import type { ForumRecentTopic } from "@fluxtorrent/domain-types";

/** 论坛首页中栏话题流（样图集 § tplForum 中列）。
 *
 *  纯 RSC——筛选走 SSR 链接（?type=sticky|digest），无客户端状态；
 *  版主任免等管理动作在版块页，首页只做浏览入口。
 *
 *  两处按真站口径做的取舍：
 *   - 版块徽章配色按版块 id 散列到固定色板（样图按版块名写死了六色，
 *     真站版块动态增长，不能建一张永远追不上的映射表）；
 *   - 筛选行只有 全部/置顶/精华 三档——本站 topic_type 没有「公告」
 *     这一类型，不为一枚装饰徽章凭空加字段（与版块页同一口径）。
 */

/** 版块徽章色板（样图 bs 映射的六对，打散按 id 取） */
const BADGE_PAIRS: [string, string][] = [
  ["bg-[#e8f4ff]", "text-[#1e8ae8]"],
  ["bg-[#dff7f0]", "text-[#1f9c7d]"],
  ["bg-[#fff4d6]", "text-[#b8860b]"],
  ["bg-[#eaebff]", "text-[#5b6bf5]"],
  ["bg-[#ffe9f4]", "text-[#e06aa8]"],
  ["bg-[#ffe9e2]", "text-[#e2553a]"],
];

/** 头像渐变色板（样图 avColor 的五条，按用户名散列） */
const AV_GRADIENTS = [
  "linear-gradient(135deg,#2fa8ff,#5b6bf5)",
  "linear-gradient(135deg,#2fbf9b,#2fa8ff)",
  "linear-gradient(135deg,#ff7a59,#ffc93c)",
  "linear-gradient(135deg,#5b6bf5,#ff8fc7)",
  "linear-gradient(135deg,#ff7a59,#5b6bf5)",
];

const hash = (s: string) =>
  [...s].reduce((h, c) => (h * 31 + c.codePointAt(0)!) % 100000, 7);

const badgePair = (id: number) => BADGE_PAIRS[id % BADGE_PAIRS.length];
const avatarBg = (name: string) =>
  AV_GRADIENTS[hash(name) % AV_GRADIENTS.length];

/** 热度标（样图 hot-tag「热 1.2k」）：**判定在后端**——阈值读管理端
 *  设置 site_settings.forum_hot_views（0397，0 = 关闭），接口回 hot 布尔；
 *  这里只拼文案与数量：过千折成 1.2k，未过千显示原数（阈值调低时
 *  98 次浏览显示「热 98」而不是「热 0.1k」）。 */
const hotTag = (views: number, label: string, hot: boolean) => {
  if (!hot) return null;
  const n = views >= 1000 ? `${(views / 1000).toFixed(1)}k` : String(views);
  return `${label} ${n}`;
};

/** MM-DD HH:mm（样图话题行的时间格式；本地时区渲染，理由见使用处） */
const stamp = (iso: string | null) => {
  if (!iso) return null;
  const d = new Date(iso);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(
    d.getMinutes(),
  )}`;
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
            // 行底色（样图 f3b-topic 三档，优先级 公告 > 置顶 > 精华）：
            // 公告 coral-soft + 左 3px coral；置顶 sky-soft + sky；
            // 精华 sun-soft + sun（精华行底色样图未给，按约定补）
            const rowTone = x.notice
              ? " fh-topic--notice"
              : x.sticky
                ? " fh-topic--pin"
                : x.digest
                  ? " fh-topic--ess"
                  : "";
            return (
              <div key={x.id} className={`fh-topic${rowTone}`}>
                <span
                  className="fh-topic__av"
                  style={{ background: avatarBg(x.username ?? "?") }}
                  aria-hidden="true"
                >
                  {(x.username ?? "?").slice(0, 1).toUpperCase()}
                </span>
                <div className="fh-topic__main">
                  <div className="fh-topic__title">
                    {/* 标记只用**文字徽章**（用户要求：不再要图钉/星形图标）：
                        公告 > 置顶 > 精华 依次出，多个标记同时存在就都显示，
                        不靠图标表达第二层含义 */}
                    {x.notice && (
                      <MarkBadge kind="notice" label={t.markNotice} />
                    )}
                    {x.sticky && <MarkBadge kind="pin" label={t.markPin} />}
                    {x.digest && <MarkBadge kind="ess" label={t.markEss} />}
                    <Link
                      href={`/forums/topic/${x.id}`}
                      className="fh-topic__t"
                    >
                      {x.title}
                    </Link>
                    {hotTag(x.views, t.hot, Boolean(x.hot)) && (
                      <span className="fh-topic__hot">
                        {hotTag(x.views, t.hot, Boolean(x.hot))}
                      </span>
                    )}
                  </div>
                  <div className="fh-topic__meta">
                    {/* 版块名在作者前面（用户要求；也替代了标题行的徽章位） */}
                    <span className={`fh-topic__board ${bb} ${bf}`}>
                      {x.forum_name}
                    </span>
                    {t.authorMeta}
                    <b>{x.username ?? "—"}</b>
                    <span className="fh-topic__sep">·</span>
                    {/* meta = **发布时间**（与右侧最后回复时间分开）；
                        容器跑 UTC、浏览器在本地时区，文本必然不同——这是
                        有意的差异（与版块页同一口径），抑制 hydration 告警 */}
                    <span suppressHydrationWarning>
                      {stamp(x.created_at ?? x.last_post_at) ?? "—"}
                    </span>
                  </div>
                </div>
                <div className="fh-topic__stats">
                  {/* 回复列已去掉（用户要求）；最后回复不再带「最后回复」
                      四个字，只留回复人与时间 */}
                  <div className="fh-topic__last">
                    <b>{last}</b>
                    <small suppressHydrationWarning>
                      {stamp(x.last_post_at) ?? ""}
                    </small>
                  </div>
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
