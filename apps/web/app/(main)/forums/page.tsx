import Link from "next/link";
import { getForumIndex } from "@/lib/data";
import { getDict } from "@/i18n/server";
import { ForumSearch } from "@/components/forum-search";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 论坛首页：按分区分组（分区是独立实体，**含空分区**）。
 *  每张卡片：版块名/描述/话题数/帖子数 + 最新话题标题预览。
 *  空分区渲染占位卡而非整段消失——旧版分组由版块列表反推，
 *  新建的分区（还没有版块）在前台会直接不见。 */
export default async function ForumsPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("forums");
  if (gate) return gate;

  const { dict } = await getDict();
  const { categories, forums } = await getForumIndex();

  // 分区是分组骨架：没有版块的分区照样成组（下面渲染空态占位）
  const groups = categories.map((c) => ({
    key: `cat-${c.id}`,
    name: c.name,
    hidden: !c.visible,
    items: forums.filter((f) => f.category_id === c.id),
  }));
  // 「未分组」= category_id 为空，或指向本次未返回的分区
  const orphans = forums.filter(
    (f) =>
      f.category_id == null ||
      !categories.some((c) => c.id === f.category_id),
  );
  if (orphans.length > 0) {
    groups.push({
      key: "cat-none",
      name: dict.forums.uncategorized,
      hidden: false,
      items: orphans,
    });
  }
  const totalTopics = forums.reduce((a, f) => a + f.topics, 0);

  return (
    <div className="pting-forums">
      {/* 左侧分区栏（按分区导航，锚点跳转） */}
      <aside className="pting-sidenav">
        <h2>{dict.forums2.allNodes}</h2>
        <nav aria-label={dict.forums.title}>
          {groups.map((g) => (
            <a key={g.key} href={`#${g.key}`}>
              <span className="pting-sidenav__dot" aria-hidden="true">
                {g.name.slice(0, 1)}
              </span>
              {g.name}
            </a>
          ))}
        </nav>
      </aside>

      {/* 版块卡片流（按分区分组） */}
      <div className="pting-board">
        <header className="pting-board__head">
          <ForumSearch
            placeholder={dict.forums2.searchPh}
            button={dict.forums2.searchBtn}
          />
          <div>
            <h1>{dict.forums.title}</h1>
            <p>
              {dict.forums2.totalPrefix}
              {totalTopics}
              {dict.forums2.totalSuffix}
            </p>
          </div>
          <Link
            href="/forums/feed"
            className="rounded-full border border-line px-4 py-1.5 text-sm font-bold text-sky transition hover:border-sky"
          >
            {dict.forums.feedTitle}
          </Link>
          <Link href="/forums/new" className="baozi-button">
            {dict.forums2.newTopic}
          </Link>
        </header>

        {groups.map((g) => (
          <section key={g.key} id={g.key} className="mt-6">
            <h2 className="mb-3 flex items-center gap-2 font-display text-lg text-ink">
              <span
                className="inline-block h-1.5 w-1.5 rounded-full bg-sky"
                aria-hidden="true"
              />
              {g.name}
              {g.hidden && (
                <span className="pting-card__count">
                  {dict.forums2.hiddenTag}
                </span>
              )}
              <span className="text-xs font-normal text-sub">
                {g.items.length} {dict.forums2.topicsUnit}
              </span>
            </h2>
            {g.items.length === 0 ? (
              <p className="pting-card__empty">{dict.forums2.emptyBoard}</p>
            ) : (
              <div className="pting-cards">
                {g.items.map((f) => (
                  <article key={f.id} className="pting-card">
                    <header>
                      <Link
                        href={`/forums/${f.id}`}
                        className="pting-card__title"
                      >
                        {f.name}
                      </Link>
                      <span className="pting-card__count num">
                        {f.topics} {dict.forums2.topicsUnit} · {f.posts}{" "}
                        {dict.forums2.postsUnit}
                      </span>
                    </header>
                    {f.descr && <p className="pting-card__descr">{f.descr}</p>}
                    <footer className="pting-card__latest">
                      <span className="pting-card__latest-label">
                        {dict.forums2.latest}
                      </span>
                      <Link
                        href={`/forums/${f.id}`}
                        className="pting-card__latest-title"
                      >
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
              </div>
            )}
          </section>
        ))}
        {groups.length === 0 && (
          <p className="pting-card__empty">{dict.forums2.goBoard} →</p>
        )}
      </div>
    </div>
  );
}
