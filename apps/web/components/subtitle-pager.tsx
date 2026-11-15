"use client";

/** 字幕区分页条（A7：count 同谓词，翻页不重叠）。
 *  从 subtitle-board.tsx 拆出（300 行门禁）。 */

export function SubtitlePager({
  page,
  pages,
  total,
  onPage,
}: {
  page: number;
  pages: number;
  total: number;
  onPage: (p: number) => void;
}) {
  const window: number[] = [];
  const lo = Math.max(1, page - 2);
  const hi = Math.min(pages, lo + 4);
  for (let i = lo; i <= hi; i++) window.push(i);
  return (
    <nav className="mt-2 flex items-center justify-center gap-2 text-sm">
      <button
        type="button"
        className="btn2"
        disabled={page <= 1}
        onClick={() => onPage(page - 1)}
      >
        ‹
      </button>
      {window.map((p) => (
        <button
          key={p}
          type="button"
          className={p === page ? "btn" : "btn2"}
          onClick={() => onPage(p)}
        >
          {p}
        </button>
      ))}
      <button
        type="button"
        className="btn2"
        disabled={page >= pages}
        onClick={() => onPage(page + 1)}
      >
        ›
      </button>
      <span className="text-sub">
        {total} · {page}/{pages}
      </span>
    </nav>
  );
}
