import Link from "next/link";
import type { Topic } from "@fluxtorrent/domain-types";
import type { Dict } from "@/i18n/zh-CN";

/** 帖子页左栏：同版块帖子列表（2026-11-12 三栏改版，机窝口径）。
 *
 *  数据 = getTopics(forum_id) 首页窗口（后端已按 minclassread 过滤），
 *  当前帖高亮置顶；超长标题两行截断。server component，无交互。
 *  列表空（权限/后端故障）时整块不渲染——左栏留白好过演空态。 */

export function TopicSiblingList({
  forumId,
  forumName,
  currentId,
  topics,
  dict,
}: {
  forumId: number;
  forumName: string | null;
  currentId: number;
  topics: Topic[];
  dict: Dict;
}) {
  if (topics.length <= 1) return null;
  const cur = topics.find((t) => t.id === currentId);
  const rest = topics.filter((t) => t.id !== currentId);
  const ordered = cur ? [cur, ...rest] : rest;
  return (
    <aside className="side-card tp-sib" aria-label={dict.forums3.sibTitle}>
      <h2 className="side-card__title">
        {dict.forums3.sibTitle}
        {forumName ? ` · ${forumName}` : ""}
      </h2>
      <ul className="tp-sib__list">
        {ordered.slice(0, 12).map((t) => (
          <li key={t.id} className="tp-sib__row">
            <Link
              href={`/forums/topic/${t.id}`}
              className={`tp-sib__a${t.id === currentId ? " is-cur" : ""}`}
              aria-current={t.id === currentId ? "page" : undefined}
            >
              <span className="tp-sib__tt">{t.title}</span>
              <span className="tp-sib__meta num">
                {t.replies > 0 ? `${t.replies} ${dict.forums3.sibReplies}` : ""}
              </span>
            </Link>
          </li>
        ))}
      </ul>
      <Link href={`/forums/${forumId}`} className="tp-sib__more">
        {dict.forums3.sibMore} →
      </Link>
    </aside>
  );
}
