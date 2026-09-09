import { EmptyTorrents, TorrentRow } from "@/components/torrent";
import { paged } from "@/lib/api-client";
import type { TorrentListItem } from "@fluxtorrent/domain-types";

export const dynamic = "force-dynamic";

const CATEGORIES = [
  { id: undefined as number | undefined, label: "全部" },
  { id: 1, label: "学前教育" },
  { id: 2, label: "小学" },
  { id: 3, label: "初中" },
  { id: 4, label: "职高" },
  { id: 5, label: "高中" },
  { id: 6, label: "教育影音" },
  { id: 7, label: "纪录片" },
];

const MEDIA = [
  { id: undefined as number | undefined, label: "全部媒介" },
  { id: 1, label: "视频" },
  { id: 2, label: "音频" },
  { id: 3, label: "书籍" },
  { id: 4, label: "文档" },
  { id: 5, label: "笔记" },
  { id: 6, label: "课件" },
  { id: 7, label: "软件" },
  { id: 8, label: "图片" },
];

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
  const page = await paged<TorrentListItem>("/api/v1/torrents", {
    limit: 20,
    category_id: sp.category_id ? Number(sp.category_id) : undefined,
    medium_id: sp.medium_id ? Number(sp.medium_id) : undefined,
    official: sp.official ? sp.official === "1" : undefined,
    include_dead: sp.include_dead === "1",
    search: sp.search,
    cursor: sp.cursor,
  });

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">资源库</h1>
        <span className="num text-sm text-sub">
          共 {page.total_estimate} 个种子
        </span>
      </div>

      {/* 筛选 Chip（移动端横向滚动，§7.3）—— 增量参数互不覆盖 */}
      <div className="flex gap-2 overflow-x-auto pb-1">
        {CATEGORIES.map((c) => (
          <a
            key={c.label}
            href={withParam(sp, "category_id", c.id ? String(c.id) : undefined)}
            aria-current={(sp.category_id ? Number(sp.category_id) : undefined) === c.id ? "true" : undefined}
            className={`min-h-[44px] flex shrink-0 items-center rounded-full px-3 text-sm ${
              (sp.category_id ? Number(sp.category_id) : undefined) === c.id
                ? "bg-sky-deep text-white"
                : "bg-white text-ink border border-line"
            }`}
          >
            {c.label}
          </a>
        ))}
      </div>
      <div className="flex gap-2 overflow-x-auto pb-1">
        {MEDIA.map((m) => (
          <a
            key={m.label}
            href={withParam(sp, "medium_id", m.id ? String(m.id) : undefined)}
            aria-current={(sp.medium_id ? Number(sp.medium_id) : undefined) === m.id ? "true" : undefined}
            className={`min-h-[44px] flex shrink-0 items-center rounded-full px-3 text-sm ${
              (sp.medium_id ? Number(sp.medium_id) : undefined) === m.id
                ? "bg-sky-deep text-white"
                : "bg-white text-ink border border-line"
            }`}
          >
            {m.label}
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
          下一页
        </a>
      )}
    </div>
  );
}
