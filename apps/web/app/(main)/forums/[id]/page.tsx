import Link from "next/link";
import { notFound } from "next/navigation";
import { getTopics, getForumTags, getBoards } from "@/lib/data";
import { TopicComposer } from "@/components/forum-composer";
import { TagChip } from "@/components/forum-bits";
import { ForumTopicList } from "@/components/forum-topic-list";
import { FollowButton } from "@/components/forum-follow";
import { Pager } from "@/components/ui/pager";
import { getDict } from "@/i18n/server";
import { fmt } from "@/i18n/config";
import { requireModule } from "@/components/module-gate";
import "@/app/styles/forums.css";

export const dynamic = "force-dynamic";

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
  const { dict, locale } = await getDict();
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
  // 类型筛选（对齐样图 § tplForumBoard 的筛选行）：白名单 sticky/digest，
  // 其余（含爬虫乱造的）一律回落全量，不拿 400 打断浏览。
  const type =
    typeRaw === "sticky" || typeRaw === "digest" ? typeRaw : undefined;
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

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-baseline gap-2">
        <Link href="/forums" className="text-sm text-sky">
          {dict.forums.backToForums}
        </Link>
      </div>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h1 className="font-display text-2xl">
          {data.forum_name ?? fmt(dict.forums.boardTitle, { id: forumId })}
        </h1>
        <div className="flex items-center gap-2">
          <FollowButton targetType="forum" targetId={forumId} showCount />
          {/* 排序切换：SSR 链接，不引入客户端状态 */}
          <div className={
            "inline-flex overflow-hidden rounded-full border border-line"
          }>
            <Link
              href={`/forums/${forumId}${qs({ sort: "new" })}`}
              aria-current={sort === "new" ? "true" : undefined}
              className={`px-3 py-1 text-xs font-bold transition ${
                sort === "new"
                  ? "bg-[var(--sky-soft)] text-sky"
                  : "text-sub hover:text-sky"
              }`}
            >
              {dict.forums.sortNew}
            </Link>
            <Link
              href={`/forums/${forumId}${qs({ sort: "hot" })}`}
              aria-current={sort === "hot" ? "true" : undefined}
              className={`px-3 py-1 text-xs font-bold transition ${
                sort === "hot"
                  ? "bg-[var(--sky-soft)] text-sky"
                  : "text-sub hover:text-sky"
              }`}
            >
              {dict.forums.sortHot}
            </Link>
          </div>
          {/* 类型筛选：样图那一行有「公告/置顶/精华」三档，本站
              topic_type 只有 bounty/poll/lottery，没有公告类型 ⇒ 只做
              置顶/精华两档（有列可依），不为装饰徽章凭空加字段。 */}
          <div
            className={
              "inline-flex overflow-hidden rounded-full border border-line"
            }
          >
            {(
              [
                [undefined, dict.forums.filterAll],
                ["sticky", dict.forums.filterSticky],
                ["digest", dict.forums.filterDigest],
              ] as [string | undefined, string][]
            ).map(([val, label]) => (
              <Link
                key={val ?? "all"}
                href={`/forums/${forumId}${qs({ type: val })}`}
                aria-current={type === val ? "true" : undefined}
                className={`px-3 py-1 text-xs font-bold transition ${
                  type === val
                    ? "bg-[var(--sky-soft)] text-sky"
                    : "text-sub hover:text-sky"
                }`}
              >
                {label}
              </Link>
            ))}
          </div>
          {data.can_create && <TopicComposer forumId={forumId} />}
        </div>
      </div>
      {/* 版块规则条（0392）：只有该版块设了规则才渲染——空框比没有更吵。
          正文用 pre-line 保留版主写的换行，故这里不能再套一层格式化。 */}
      {rules && (
        <div className="fb-rules">
          <span className="fb-rules__t">{dict.forums3.rulesTitle}</span>
          {rules}
        </div>
      )}
      {/* 标签筛选（0123）：SSR 链接切换，选中项加 outline 高亮；「全部」清除筛选 */}
      {tagDict.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          <Link
            href={`/forums/${forumId}${qs({ tag: undefined })}`}
            aria-current={!tag ? "true" : undefined}
            className={`rounded-full border px-2.5 py-0.5 text-[11px] \
font-bold ${
              !tag
                ? "border-sky bg-[var(--sky-soft)] text-sky"
                : "border-line text-sub hover:text-sky"
            }`}
          >
            {dict.forums.tagAll}
          </Link>
          {tagDict.map((t) =>
            tag === t.id ? (
              <Link
                key={t.id}
                href={`/forums/${forumId}${qs({ tag: undefined })}`}
                aria-current="true"
                className={
                  "rounded-full outline outline-2 outline-offset-1 " +
                  "outline-sky"
                }
                title={dict.forums.tagClear}
              >
                <TagChip tag={t} />
              </Link>
            ) : (
              <Link
                key={t.id}
                href={`/forums/${forumId}${qs({ tag: String(t.id) })}`}
                className={
                  "rounded-full opacity-70 transition hover:opacity-100"
                }
              >
                <TagChip tag={t} />
              </Link>
            ),
          )}
        </div>
      )}
      {activeTag && topics.length === 0 && (
        <p className="py-6 text-center text-sm text-sub">
          {dict.forums.tagNoHit}
        </p>
      )}
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
  );
}
