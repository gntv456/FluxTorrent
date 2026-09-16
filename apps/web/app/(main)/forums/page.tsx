import Link from "next/link";
import { getForums } from "@/lib/data";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";
import { ForumSearch } from "@/components/forum-search";

export const dynamic = "force-dynamic";

/** 论坛（pting.club 布局复刻）：左侧兴趣节点栏 + 版块卡片流
 *  每张卡片：版块名/描述/话题数/帖子数 + 最新话题标题预览 */
export default async function ForumsPage() {
  const { dict, locale } = await getDict();
  const forums = await getForums();
  return (
    <div className="pting-forums">
      {/* 左侧节点栏（pting complementary nav 同构） */}
      <aside className="pting-sidenav">
        <h2>{dict.forums2.allNodes}</h2>
        <nav aria-label={dict.forums.title}>
          {forums.map((f) => (
            <a key={f.id} href={`/forums/${f.id}`}>
              <span className="pting-sidenav__dot" aria-hidden="true">
                {f.name.slice(0, 1)}
              </span>
              {f.name}
            </a>
          ))}
        </nav>
      </aside>

      {/* 版块卡片流 */}
      <div className="pting-board">
        <header className="pting-board__head">
          <ForumSearch placeholder={dict.forums2.searchPh} button={dict.forums2.searchBtn} />
          <div>
            <h1>{dict.forums.title}</h1>
            <p>
              {dict.forums2.totalPrefix}
              {forums.reduce((a, f) => a + f.topics, 0)}
              {dict.forums2.totalSuffix}
            </p>
          </div>
          <Link href="/forums/new" className="baozi-button">
            {dict.forums2.newTopic}
          </Link>
        </header>
        <div className="pting-cards">
          {forums.map((f) => (
            <article key={f.id} className="pting-card">
              <header>
                <Link href={`/forums/${f.id}`} className="pting-card__title">
                  {f.name}
                </Link>
                <span className="pting-card__count num">
                  {f.topics} {dict.forums2.topicsUnit} · {f.posts} {dict.forums2.postsUnit}
                </span>
              </header>
              {f.descr && <p className="pting-card__descr">{f.descr}</p>}
              <footer className="pting-card__latest">
                <span className="pting-card__latest-label">{dict.forums2.latest}</span>
                <Link href={`/forums/${f.id}`} className="pting-card__latest-title">
                  {f.latest_topic ?? `${dict.forums2.goBoard} →`}
                </Link>
                {f.latest_author && (
                  <span className="pting-card__count">
                    {dict.forums2.by}
                    {f.latest_author}
                  </span>
                )}
              </footer>
            </article>
          ))}
          {forums.length === 0 && (
            <p className="pting-card__empty">{dict.forums2.goBoard} →</p>
          )}
        </div>
      </div>
    </div>
  );
}
