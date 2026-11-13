/**
 * 版块页中栏头部（2026-11-13 三栏改版从 page 拆出，300 行门禁）：
 * 返回链接 + 版块名 + 关注按钮 + 排序/类型筛选胶囊组。
 * 全 SSR 链接切换（不引客户端状态），qs 基由调用侧传入保持
 * sort/tag/type 互不丢失。
 */

import Link from "next/link";
import { FollowButton } from "@/components/forum-follow";
import { TagChip, type TagChipData } from "@/components/forum-bits";
import type { Dict } from "@/i18n/zh-CN";
import { fmt } from "@/i18n/config";

export function BoardHead({
  forumId,
  forumName,
  sort,
  type,
  qs,
  dict,
}: {
  forumId: number;
  forumName?: string | null;
  sort: string;
  type: string | undefined;
  /** 链接 query 基：见 page.tsx 的 qs()（换 sort/type 保留其余参数） */
  qs: (over: Record<string, string | undefined>) => string;
  dict: Dict;
}) {
  const pill =
    "inline-flex overflow-hidden rounded-full border border-line";
  const pillItem = (on: boolean | undefined) =>
    `px-3 py-1 text-xs font-bold transition ${
      on
        ? "bg-[var(--sky-soft)] text-[var(--sky)]"
        : "text-sub hover:text-[var(--sky)]"
    }`;
  return (
    <div className="flex flex-col gap-2">
      <Link href="/forums" className="text-sm text-[var(--sky)]">
        {dict.forums.backToForums}
      </Link>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <b className="font-display text-2xl">
          {forumName ?? fmt(dict.forums.boardTitle, { id: forumId })}
        </b>
        <div className="flex items-center gap-2">
          <FollowButton targetType="forum" targetId={forumId} showCount />
          {/* 排序切换（0116）：SSR 链接，不引入客户端状态 */}
          <div className={pill}>
            <Link
              href={`/forums/${forumId}${qs({ sort: "new" })}`}
              aria-current={sort === "new" ? "true" : undefined}
              className={pillItem(sort === "new")}
            >
              {dict.forums.sortNew}
            </Link>
            <Link
              href={`/forums/${forumId}${qs({ sort: "hot" })}`}
              aria-current={sort === "hot" ? "true" : undefined}
              className={pillItem(sort === "hot")}
            >
              {dict.forums.sortHot}
            </Link>
          </div>
          {/* 类型筛选：样图那一行有「公告/置顶/精华」三档，本站
              topic_type 只有 bounty/poll/lottery，没有公告类型 ⇒ 只做
              置顶/精华两档（有列可依），不为装饰徽章凭空加字段。 */}
          <div className={pill}>
            {(
              [
                [undefined, dict.forums.filterAll],
                ["notice", dict.forums.filterNotice],
                ["sticky", dict.forums.filterSticky],
                ["digest", dict.forums.filterDigest],
              ] as [string | undefined, string][]
            ).map(([val, label]) => (
              <Link
                key={val ?? "all"}
                href={`/forums/${forumId}${qs({ type: val })}`}
                aria-current={type === val ? "true" : undefined}
                className={pillItem(type === val)}
              >
                {label}
              </Link>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}


/** 标签筛选行（0123，2026-11-13 随三栏改版从 page 拆出）：
 *  SSR 链接切换，选中项 outline 高亮 + 「全部」清除筛选。 */
export function BoardTagRow({
  forumId,
  tagDict,
  tag,
  qs,
  dict,
  noHit,
}: {
  forumId: number;
  tagDict: TagChipData[];
  tag: number | undefined;
  qs: (over: Record<string, string | undefined>) => string;
  dict: Dict;
  /** 选中标签且无命中时的空态文案（调用侧：topics.length === 0） */
  noHit?: boolean;
}) {
  if (tagDict.length === 0) return null;
  return (
    <>
      {noHit && (
        <p className="py-6 text-center text-sm text-sub">
          {dict.forums.tagNoHit}
        </p>
      )}
      <div className="flex flex-wrap items-center gap-1.5">
      <Link
        href={`/forums/${forumId}${qs({ tag: undefined })}`}
        aria-current={!tag ? "true" : undefined}
        className={`rounded-full border px-2.5 py-0.5 text-xs font-bold ${
          !tag
            ? "border-[var(--sky)] bg-[var(--sky-soft)] text-[var(--sky)]"
            : "border-line text-sub hover:text-[var(--sky)]"
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
            className="rounded-full opacity-70 transition hover:opacity-100"
          >
            <TagChip tag={t} />
          </Link>
        ),
      )}
      </div>
    </>
  );
}
