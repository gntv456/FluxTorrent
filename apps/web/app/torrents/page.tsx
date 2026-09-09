import { EmptyTorrents, TorrentRow } from "@/components/torrent";
import { paged } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { fmt } from "@/i18n/config";
import type { TorrentListItem } from "@fluxtorrent/domain-types";

export const dynamic = "force-dynamic";

/** 在现有参数上增量修改，保留其余筛选（修复翻页丢参数） */
function withParam(
  sp: Record<string, string | undefined>,
  key: string,
  value: string | undefined,
): string {
  const qs = new URLSearchParams();
  for (const [k, v] of Object.entries(sp)) {
    if (v !== undefined && v !== "" && k !== key) qs.set(k, v);
  }
  if (value !== undefined && value !== "") qs.set(key, value);
  const s = qs.toString();
  return s ? `/torrents?${s}` : "/torrents";
}

export default async function TorrentsPage({
  searchParams,
}: {
  searchParams: Promise<Record<string, string | undefined>>;
}) {
  const sp = await searchParams;
  const { dict } = await getDict();
  // 半旧会话（cookie 无 token）或后端抖动时降级为空列表，页面骨架仍可用
  const page = await paged<TorrentListItem>("/api/v1/torrents", {
    limit: 20,
    category_id: sp.category_id ? Number(sp.category_id) : undefined,
    medium_id: sp.medium_id ? Number(sp.medium_id) : undefined,
    grade_id: sp.grade_id ? Number(sp.grade_id) : undefined,
    official: sp.official ? sp.official === "1" : undefined,
    include_dead: sp.include_dead === "1",
    search: sp.search,
    sort: sp.sort,
    cursor: sp.cursor,
  }).catch(() => ({
    items: [] as TorrentListItem[],
    next_cursor: null,
    total_estimate: 0,
  }));

  // 字典分类/媒介数组按下标对齐：index 0 = 全部，1..n = 对应 id
  const categories = dict.torrents.categories.map((label, i) => ({
    id: i === 0 ? undefined : i,
    label,
  }));
  const media = dict.torrents.media.map((label, i) => ({
    id: i === 0 ? undefined : i,
    label,
  }));
  const currentCategory = sp.category_id ? Number(sp.category_id) : undefined;
  const currentMedium = sp.medium_id ? Number(sp.medium_id) : undefined;
  const currentGrade = sp.grade_id !== undefined && sp.grade_id !== "" ? Number(sp.grade_id) : undefined;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.torrents.title}</h1>
        <span className="num text-sm text-sub">
          {fmt(dict.torrents.total, { n: page.total_estimate })}
        </span>
      </div>

      {/* 全文搜索（标题/简介/文件名，后端 pg_trgm）——GET 表单保留其余筛选 */}
      <form action="/torrents" method="get" className="flex gap-2">
        <input
          type="search"
          name="search"
          defaultValue={sp.search}
          placeholder={dict.torrents.searchPlaceholder}
          aria-label={dict.torrents.searchAction}
          className="min-h-[44px] flex-1 rounded-full border border-line bg-white px-4 text-sm outline-none focus:border-sky"
        />
        {sp.category_id && (
          <input type="hidden" name="category_id" value={sp.category_id} />
        )}
        {sp.medium_id && (
          <input type="hidden" name="medium_id" value={sp.medium_id} />
        )}
        {sp.grade_id && <input type="hidden" name="grade_id" value={sp.grade_id} />}
        {sp.official && <input type="hidden" name="official" value={sp.official} />}
        {sp.include_dead && (
          <input type="hidden" name="include_dead" value="1" />
        )}
        {sp.sort && <input type="hidden" name="sort" value={sp.sort} />}
        <button
          type="submit"
          className="min-h-[44px] rounded-full bg-sky-deep px-5 text-sm font-bold text-white active:scale-[0.97]"
        >
          {dict.torrents.searchAction}
        </button>
      </form>
      {sp.search && (
        <a
          href={withParam(sp, "search", undefined)}
          className="self-start text-sm text-sky"
        >
          ✕ {dict.torrents.clearSearch}「{sp.search}」
        </a>
      )}

      {/* 筛选 Chip（移动端横向滚动，§7.3）—— 增量参数互不覆盖 */}
      <div className="flex gap-2 overflow-x-auto pb-1">
        {categories.map((c) => (
          <a
            key={c.label}
            href={withParam(sp, "category_id", c.id ? String(c.id) : undefined)}
            aria-current={currentCategory === c.id ? "true" : undefined}
            className={`min-h-[44px] flex shrink-0 items-center rounded-full px-3 text-sm ${
              currentCategory === c.id
                ? "bg-sky-deep text-white"
                : "bg-white text-ink border border-line"
            }`}
          >
            {c.label}
          </a>
        ))}
      </div>
      <div className="flex gap-2 overflow-x-auto pb-1">
        {media.map((m) => (
          <a
            key={m.label}
            href={withParam(sp, "medium_id", m.id ? String(m.id) : undefined)}
            aria-current={currentMedium === m.id ? "true" : undefined}
            className={`min-h-[44px] flex shrink-0 items-center rounded-full px-3 text-sm ${
              currentMedium === m.id
                ? "bg-sky-deep text-white"
                : "bg-white text-ink border border-line"
            }`}
          >
            {m.label}
          </a>
        ))}
      </div>
      {/* 学段筛选（grades id 0-12 与字典下标 i=id+1 对齐） */}
      <div className="flex gap-2 overflow-x-auto pb-1">
        {dict.torrents.grades.map((label, gid) => (
          <a
            key={label}
            href={withParam(
              sp,
              "grade_id",
              gid === 0 ? undefined : String(gid - 1),
            )}
            aria-current={
              currentGrade === (gid === 0 ? undefined : gid - 1)
                ? "true"
                : undefined
            }
            className={`min-h-[44px] flex shrink-0 items-center rounded-full px-3 text-sm ${
              currentGrade === (gid === 0 ? undefined : gid - 1)
                ? "bg-sky-deep text-white"
                : "bg-white text-ink border border-line"
            }`}
          >
            {label}
          </a>
        ))}
      </div>

      {/* 排序（旧站 torrents.php 口径）：默认时间 · 做种 · 体积 · 完成 */}
      <div className="flex gap-2 overflow-x-auto pb-1">
        {(
          [
            [undefined, dict.torrents.sortCreated],
            ["seeders", dict.torrents.sortSeeders],
            ["size", dict.torrents.sortSize],
            ["completed", dict.torrents.sortCompleted],
          ] as [string | undefined, string][]
        ).map(([value, label]) => (
          <a
            key={label}
            href={withParam(sp, "sort", value)}
            aria-current={(sp.sort ?? undefined) === value ? "true" : undefined}
            className={`min-h-[44px] flex shrink-0 items-center rounded-full px-3 text-sm ${
              (sp.sort ?? undefined) === value
                ? "bg-sky-deep text-white"
                : "bg-white text-ink border border-line"
            }`}
          >
            {label}
          </a>
        ))}
      </div>

      {page.items.length === 0 ? (
        <EmptyTorrents />
      ) : (
        <div className="flex flex-col gap-2">
          {page.items.map((t) => (
            <TorrentRow key={t.id} t={t} />
          ))}
        </div>
      )}

      {page.next_cursor && (
        <a
          href={withParam(sp, "cursor", page.next_cursor)}
          className="mx-auto min-h-[44px] flex items-center rounded-full border border-line bg-white px-6 text-sm text-sky-deep"
        >
          {dict.common.nextPage}
        </a>
      )}
    </div>
  );
}
