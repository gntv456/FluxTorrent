import Link from "next/link";
import { MarkBadge, StarMark } from "@/components/forum-bits";
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

/** 热度标：浏览过千才显示（样图 hot-tag；数字是真的 views）；
 *  标签词走字典（i18n 门禁），数字部分本地拼 */
const hotTag = (views: number, label: string) =>
  views >= 1000 ? `${label} ${(views / 1000).toFixed(1)}k` : null;

/** MM-DD HH:mm（样图话题行的时间格式；本地时区渲染，理由见使用处） */
const stamp = (iso: string | null) => {
  if (!iso) return null;
  const d = new Date(iso);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(
    d.getMinutes(),
  )}`;
};

export type StreamType = "sticky" | "digest" | undefined;

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
  const filters: [StreamType, string][] = [
    [undefined, t.filterAll],
    ["sticky", t.filterSticky],
    ["digest", t.filterDigest],
  ];
  // 「回复 {n}」是带占位的句子，取列头用去掉占位的纯词
  const replyLabel = t.replies.replace("{n}", "").trim() || t.replies;

  return (
    <div className="fh-stream">
      {/* 筛选行：SSR 链接，与版块页 type 筛选同口径 */}
      <div className="fh-filter">
        {filters.map(([val, label]) => {
          const on = val === type;
          const dot =
            val === "sticky"
              ? "bg-coral"
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
          <b>{t.topicTitleFallback}</b>
          <span className="fh-stream__sp" />
          <span className="fh-stream__col-reply">{replyLabel}</span>
          <span className="fh-stream__col-last">{t.lastPost}</span>
        </div>
        {topics.length === 0 ? (
          <p className="fh-stream__empty">{t.noTopics}</p>
        ) : (
          topics.map((x) => {
            const [bb, bf] = badgePair(x.forum_id);
            const last = x.last_replier ?? x.username ?? "—";
            // 行底色（样图 f3b-topic--pin/--notice 口径；精华行样图没给
            // 底色，按用户要求补 sun-soft，与徽章同色系）：
            // 置顶 sky-soft + 左 3px sky；精华 sun-soft + 左 3px sun
            const rowTone = x.sticky
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
                    {/* 标记顺序照样图：星形图标 → 文字徽章 → 标题 */}
                    {x.sticky && <StarMark tone="coral" />}
                    {x.digest && <StarMark tone="sun" />}
                    {x.sticky && (
                      <MarkBadge kind="pin" label={t.markPin} />
                    )}
                    {x.digest && (
                      <MarkBadge kind="ess" label={t.markEss} />
                    )}
                    <Link
                      href={`/forums/topic/${x.id}`}
                      className="fh-topic__t"
                    >
                      {x.title}
                    </Link>
                    <span className={`fh-topic__board ${bb} ${bf}`}>
                      {x.forum_name}
                    </span>
                    {hotTag(x.views, t.hot) && (
                      <span className="fh-topic__hot">
                        {hotTag(x.views, t.hot)}
                      </span>
                    )}
                  </div>
                  <div className="fh-topic__meta">
                    {t.author}
                    <b>{x.username ?? "—"}</b>
                    <span className="fh-topic__sep">·</span>
                    {/* 容器跑 UTC、浏览器在本地时区，文本必然不同——这是
                        有意的差异（与版块页同一口径），抑制 hydration 告警 */}
                    <span suppressHydrationWarning>
                      {stamp(x.last_post_at) ?? "—"}
                    </span>
                  </div>
                </div>
                <div className="fh-topic__stats">
                  <div className="fh-topic__reply">
                    <b className="num">{Math.max(x.replies, 0)}</b>
                    <small>{replyLabel}</small>
                  </div>
                  <div className="fh-topic__last">
                    <b>{last}</b>
                    <small>
                      {t.lastPost}{" "}
                      <span suppressHydrationWarning>
                        {stamp(x.last_post_at) ?? ""}
                      </span>
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
