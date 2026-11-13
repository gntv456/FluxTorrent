import Link from "next/link";
import { notFound } from "next/navigation";
import {
  getTopics,
  getForumTags,
  getBoards,
  getForumIndex,
  getForumSide,
  getForumUserCard,
} from "@/lib/data";
import { hasSessionToken } from "@/lib/session-token";
import { TopicComposer } from "@/components/forum-composer";
import { ForumTopicList } from "@/components/forum-topic-list";
import { Pager } from "@/components/ui/pager";
import { ForumSearch } from "@/components/forum-search";
import { ForumSideColumn } from "@/components/forum-home-aside";
import {
  ForumLeftColumn,
  type BoardGroup,
} from "@/components/forum-left-column";
import { ForumMobileBar } from "@/components/forum-mobile-bar";
import { BoardHead, BoardTagRow } from "./_parts/board-head";
import { getDict } from "@/i18n/server";
import { fmt } from "@/i18n/config";
import { requireModule } from "@/components/module-gate";
/* 两份页面级样式都要：forums.css = 话题列表/规则条等版块页原有表面；
   forums-home.css = 三栏栅格与两翼（2026-11-13 版块页对齐首页三栏）。 */
import "@/app/styles/forums.css";
import "@/app/styles/forums-home.css";

export const dynamic = "force-dynamic";

/** 版块页：三栏（2026-11-13 对齐论坛首页）——左列用户卡 + 版块导航
 *  （当前版块高亮）、中栏 = 该版块的话题列表（标题/筛选/规则条/分页，
 *  原单栏内容原样收编）、右列统计 / 热帖 / 最新回复。
 *  手机端两翼收进「版块/更多」弹窗（ForumMobileBar 同款四按钮）。 */
export default async function ForumPage({
  params,
  searchParams,
}: {
  params: Promise<{ id: string }>;
  searchParams: Promise<{
    sort?: string;
    tag?: string;
    page?: string;
    type?: string;
  }>;
}) {
  const gate = await requireModule("forums");
  if (gate) return gate;
  const { id } = await params;
  const { sort: sortRaw, tag: tagRaw, page: pageRaw, type: typeRaw } =
    await searchParams;
  const { dict, locale, currency } = await getDict();
  const forumId = Number(id);
  if (!Number.isFinite(forumId)) notFound();
  // 排序白名单（0116）：hot=热度衰减 / 缺省 new=最新
  const sort = sortRaw === "hot" ? "hot" : "new";
  // 标签筛选（0123）：tag=tag_dict id；非法值回落全量
  const tagNum = Number(tagRaw);
  const tag = Number.isFinite(tagNum) && tagNum > 0 ? tagNum : undefined;
  // 页码（2026-10-17）：非法/越界一律回落第 1 页，不拿 404 打断浏览
  const pageNum = Number(pageRaw);
  const page =
    Number.isFinite(pageNum) && pageNum > 1 ? Math.floor(pageNum) : 1;
  // 类型筛选（对齐样图 § tplForumBoard 的筛选行）：白名单
  // notice/sticky/digest，其余（含爬虫乱造的）一律回落全量，不拿 400 打断浏览。
  const type =
    typeRaw === "notice" || typeRaw === "sticky" || typeRaw === "digest"
      ? typeRaw
      : undefined;
  // 两翼数据与版块数据并发：左右列只对登录者取（游客打了也是 null）
  const signed = await hasSessionToken();
  const [data, tagDict, index, me, side] = await Promise.all([
    getTopics(forumId, sort, tag, page, type),
    getForumTags(),
    getForumIndex(),
    signed ? getForumUserCard() : Promise.resolve(null),
    signed ? getForumSide() : Promise.resolve(null),
  ]);
  // 版块不存在或无 minclassread 门槛权限（Forbidden）→ 404 口径
  if (!data) notFound();
  // 版主「移动到」下拉的数据源：仅 can_mod 时才拉。
  // 用 /forums/boards（单条 SQL）而非 /forums（逐版块 forum_access，1+N 次查询）。
  const allForums = data.can_mod ? await getBoards() : [];
  const topics = data.topics;
  // 版块规则（0392）：NULL 与纯空白都算「未设置」⇒ 不渲染规则条
  const rules = (data.forum_rules ?? "").trim();
  const activeTag = tagDict.find((t) => t.id === tag);
  // 左栏分组（与论坛首页同构：分区骨架 + 未分组尾巴）
  const groups: BoardGroup[] = index.categories.map((c) => ({
    key: `cat-${c.id}`,
    name: c.name,
    hidden: !c.visible,
    items: index.forums.filter((f) => f.category_id === c.id),
  }));
  const orphans = index.forums.filter(
    (f) =>
      f.category_id == null ||
      !index.categories.some((c) => c.id === f.category_id),
  );
  if (orphans.length > 0) {
    groups.push({
      key: "cat-none",
      name: dict.forums.uncategorized,
      hidden: false,
      items: orphans,
    });
  }
  const totalTopics = index.forums.reduce((a, f) => a + f.topics, 0);
  // 筛选/排序链接共享的 query 基（保持 tag 与 sort 互不丢失）
  const qs = (over: Record<string, string | undefined>) => {
    const sp = new URLSearchParams();
    const merged = {
      sort,
      tag: tag ? String(tag) : undefined,
      type,
      ...over,
    };
    for (const [k, v] of Object.entries(merged)) if (v) sp.set(k, v);
    const s = sp.toString();
    return s ? `?${s}` : "";
  };
  // 翻页条（2026-10-17）：本出口此前写死 LIMIT 50 且没有翻页参数——超 50 帖
  // 的版块第 51 帖起永远打不开，界面还长得像「就这么多了」。现在按信封里的
  // total 画条；换页保留 sort/tag，而换 sort/tag 自然回到第 1 页。
  const per = data.per ?? 50;
  const total = data.total ?? topics.length;
  const pages = Math.max(1, Math.ceil(total / per));
  const pageHrefs = Array.from({ length: pages }, (_, i) =>
    `/forums/${forumId}${i + 1 > 1 ? qs({ page: String(i + 1) }) : ""}`,
  );
  const sideState = !signed ? "guest" : side ? "ok" : "fail";
  const leftColumn = (
    <ForumLeftColumn
      me={me}
      dict={dict}
      currency={currency}
      groups={groups}
      totalTopics={totalTopics}
      activeId={forumId}
    />
  );
  const searchBox = (
    <ForumSearch
      placeholder={dict.forums2.searchPh}
      button={dict.forums2.searchBtn}
    />
  );
  // 手机端四档筛选（与首页同组白名单）
  const filterOpts: [string | undefined, string, string][] = [
    [undefined, dict.forums.filterAll, "bg-sky"],
    ["notice", dict.forums.filterNotice, "bg-coral"],
    ["sticky", dict.forums.filterSticky, "bg-indigo"],
    ["digest", dict.forums.filterDigest, "bg-sun"],
  ];
  const filterLabel = (filterOpts.find(([v]) => v === type) ??
    filterOpts[0])[1];

  return (
    <div className="pting-forums">
      {/* 左列：用户卡 + 版块导航（当前版块高亮；≤768 收进「版块」弹窗） */}
      <div className="fh-left">{leftColumn}</div>

      {/* 中栏：版块内容（原单栏页的全部内容收编于此） */}
      <div className="pting-board">
        <h1 className="sr-only">
          {data.forum_name ?? fmt(dict.forums.boardTitle, { id: forumId })}
        </h1>
        <header className="pting-board__head">
          <span className="fh-head__search">{searchBox}</span>
          <span className="fh-head__new">
            {data.can_create ? <TopicComposer forumId={forumId} /> : null}
          </span>
        </header>

        {/* 手机端（≤768）四按钮一排：筛选 / 版块 / 更多 / 搜帖 */}
        <ForumMobileBar
          filterLabel={filterLabel}
          labels={{
            nav: dict.forums2.allNodes,
            more: dict.forums3.mbarMore,
            search: dict.forums2.searchBtn,
          }}
          titles={{
            filter: dict.forums3.mbarFilter,
            nav: dict.forums2.allNodes,
            more: dict.forums3.mbarMore,
            search: dict.forums2.searchBtn,
          }}
          panels={{
            filter: (
              <div className="fh-pick">
                {filterOpts.map(([v, label, dot]) => (
                  <Link
                    key={v ?? "all"}
                    href={
                      v
                        ? `/forums/${forumId}${qs({ type: v })}`
                        : `/forums/${forumId}${qs({ type: undefined })}`
                    }
                    className={
                      "fh-pick__row" + (type === v ? " on" : undefined)
                    }
                    aria-current={type === v ? "true" : undefined}
                  >
                    <span
                      className={"fh-pick__dot " + dot}
                      aria-hidden="true"
                    />
                    {label}
                  </Link>
                ))}
              </div>
            ),
            nav: leftColumn,
            more: (
              <>
                <div className="fh-macts">
                  <Link href="/forums/feed" className="fh-macts__a">
                    {dict.forums.feedTitle}
                  </Link>
                  {data.can_create ? <TopicComposer forumId={forumId} /> : null}
                </div>
                <ForumSideColumn
                  side={side}
                  state={sideState}
                  dict={dict}
                />
              </>
            ),
            search: searchBox,
          }}
        />

        <BoardHead
          forumId={forumId}
          forumName={data.forum_name}
          sort={sort}
          type={type}
          qs={qs}
          dict={dict}
        />
        {/* 版块规则条（0392）：只有该版块设了规则才渲染——空框比没有更吵。
            正文用 pre-line 保留版主写的换行，故这里不能再套一层格式化。 */}
        {rules && (
          <div className="fb-rules">
            <span className="fb-rules__t">{dict.forums3.rulesTitle}</span>
            {rules}
          </div>
        )}
        <BoardTagRow
          forumId={forumId}
          tagDict={tagDict}
          tag={tag}
          qs={qs}
          dict={dict}
          noHit={!!activeTag && topics.length === 0}
        />
        {topics.length === 0 ? (
          <p className="py-10 text-center text-sub">{dict.forums.noTopics}</p>
        ) : (
          <ForumTopicList
            topics={topics}
            canMod={data.can_mod}
            forums={allForums}
          />
        )}
        {pages > 1 && (
          <Pager page={page} pages={pages} total={total} hrefs={pageHrefs} />
        )}
      </div>

      {/* 右列：论坛统计 / 今日热帖 / 最新回复（与首页同款） */}
      <ForumSideColumn side={side} state={sideState} dict={dict} />
    </div>
  );
}
