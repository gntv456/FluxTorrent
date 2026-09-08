import { getTextbooks } from "@/lib/data";
import Link from "next/link";

export const dynamic = "force-dynamic";

export default async function TextbooksPage() {
  const books = await getTextbooks();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">课本中心</h1>
        <span className="text-sm text-sub">按科目/版本/年级找教材资源</span>
      </div>
      {books.length === 0 ? (
        <div className="flex flex-col items-center gap-2 py-10 text-center">
          <span aria-hidden className="text-[80px] leading-none">
            📚
          </span>
          <p className="font-display text-lg">课本库还在筹备中</p>
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {books.map((b) => (
            <div
              key={b.id}
              className="flex flex-col gap-1 rounded-[var(--r-md)] border border-line bg-white p-4 shadow-[var(--shadow-card)]"
            >
              <div className="flex items-center justify-between">
                <h2 className="font-bold">{b.subject}</h2>
                <span className="sticker bg-sky text-white">{b.edition}版</span>
              </div>
              <p className="text-sm text-sub">
                {b.grade}
                {b.volume ? ` · ${b.volume}` : ""}
              </p>
              {b.torrent_id ? (
                <Link
                  href={`/torrent/${b.torrent_id}`}
                  className="text-sm font-bold text-sky"
                >
                  查看资源 →
                </Link>
              ) : (
                <p className="text-xs text-sub">暂无关联资源</p>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
