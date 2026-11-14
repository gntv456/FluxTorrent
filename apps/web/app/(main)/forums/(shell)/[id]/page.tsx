import Link from "next/link";
import { notFound } from "next/navigation";
import { getTopics, getForumTags, getBoards } from "@/lib/data";
import { TopicComposer } from "@/components/forum-composer";
import { ForumTopicList } from "@/components/forum-topic-list";
import { Pager } from "@/components/ui/pager";
import { ForumSearch } from "@/components/forum-search";
import { BoardHead, BoardTagRow } from "./_parts/board-head";
import { getDict } from "@/i18n/server";
import { fmt } from "@/i18n/config";

export const dynamic = "force-dynamic";

/** 版块页中栏：标题/筛选/规则条/话题列表/分页。
 *
 *  左右两翼、手机工具条与模块守卫都在 (shell)/layout.tsx；本页只
 *  负责中栏取数与渲染——左栏版块导航点一下时，Next 只替换这一段。 */
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
  const { id } = await params;
  const { sort: sortRaw, tag: tagRaw, page: pageRaw, type: typeRaw } =
    await searchParams;
  const { dict } = await getDict();
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
  // notice/sticky/digest，其余（含爬虫乱造的）一律回落全量。
  const type =
    typeRaw === "notice" || typeRaw === "sticky" || typeRaw === "digest"
      ? typeRaw
      : undefined;
  const [data, tagDict] = await Promise.all([
    getTopics(forumId, sort, tag, page, type),
    getForumTags(),
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
  const searchBox = (
    <ForumSearch
      placeholder={dict.forums2.searchPh}
      button={dict.forums2.searchBtn}
    />
  );

  return (
    <>
      <h1 className="sr-only">
        {data.forum_name ?? fmt(dict.forums.boardTitle, { id: forumId })}
      </h1>
      <header className="pting-board__head">
        <span className="fh-head__search">{searchBox}</span>
        <span className="fh-head__new">
          {data.can_create ? <TopicComposer forumId={forumId} /> : null}
        </span>
      </header>

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
    </>
  );
}
