import { getTextbooks } from "@/lib/data";
import Link from "next/link";
import { getDict } from "@/i18n/server";
import { fmt } from "@/i18n/config";
import { TextbookLinkButton } from "@/components/textbook-link-button";

export const dynamic = "force-dynamic";

export default async function TextbooksPage() {
  const { dict } = await getDict();
  const books = await getTextbooks();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.textbooks.title}</h1>
        <span className="text-sm text-sub">{dict.textbooks.subtitle}</span>
      </div>
      {books.length === 0 ? (
        <div className="flex flex-col items-center gap-2 py-10 text-center">
          <span aria-hidden className="text-[80px] leading-none">
            📚
          </span>
          <p className="font-display text-lg">{dict.textbooks.empty}</p>
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
                <span className="sticker bg-sky text-white">
                  {fmt(dict.textbooks.edition, { name: b.edition })}
                </span>
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
                  {dict.textbooks.viewTorrent}
                </Link>
              ) : (
                <div className="flex items-center gap-2">
                  <p className="text-xs text-sub">{dict.textbooks.noTorrent}</p>
                  <TextbookLinkButton textbookId={b.id} />
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
