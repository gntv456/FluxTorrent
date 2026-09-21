import Link from "next/link";
import { getForums } from "@/lib/data";
import { getDict } from "@/i18n/server";
import { ForumSearch } from "@/components/forum-search";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 论坛首页：按分区/节点分组（0115）+ 左侧分区导航 + 版块卡片流
 *  每张卡片：版块名/描述/话题数/帖子数 + 最新话题标题预览 */
export default async function ForumsPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("forums");
  if (gate) return gate;

  const { dict } = await getDict();
  const forums = await getForums();

  // 按分类分组（无分类归入「未分组」）；后端已按 category.sort 排序，保持顺序
  const groups: { name: string; items: typeof forums }[] = [];
  for (const f of forums) {
    const name = f.category_name ?? dict.forums.uncategorized;
    let g = groups.find((x) => x.name === name);
    if (!g) {
      g = { name, items: [] };
      groups.push(g);
    }
    g.items.push(f);
  }
  const totalTopics = forums.reduce((a, f) => a + f.topics, 0);

  return (
    <div className="pting-forums">
      {/* 左侧分区栏（按分类导航，锚点跳转） */}
      <aside className="pting-sidenav">
        <h2>{dict.forums2.allNodes}</h2>
        <nav aria-label={dict.forums.title}>
          {groups.map((g) => (
            <a key={g.name} href={`#cat-${encodeURIComponent(g.name)}`}>
              <span className="pting-sidenav__dot" aria-hidden="true">
                {g.name.slice(0, 1)}
              </span>
              {g.name}
            </a>
          ))}
        </nav>
      </aside>

      {/* 版块卡片流（按分类分组） */}
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
          <section
            key={g.name}
            id={`cat-${encodeURIComponent(g.name)}`}
            className="mt-6"
          >
            <h2 className="mb-3 flex items-center gap-2 font-display text-lg text-ink">
              <span
                className="inline-block h-1.5 w-1.5 rounded-full bg-sky"
                aria-hidden="true"
              />
              {g.name}
              <span className="text-xs font-normal text-sub">
                {g.items.length} {dict.forums2.topicsUnit}
              </span>
            </h2>
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
          </section>
        ))}
        {forums.length === 0 && (
          <p className="pting-card__empty">{dict.forums2.goBoard} →</p>
        )}
      </div>
    </div>
  );
}
