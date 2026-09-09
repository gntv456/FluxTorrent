import Link from "next/link";
import { getPosts } from "@/lib/data";
import { getDict } from "@/i18n/server";
import { dateLocale, fmt } from "@/i18n/config";

export const dynamic = "force-dynamic";

export default async function TopicPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const { dict, locale } = await getDict();
  const posts = await getPosts(Number(id));

  return (
    <div className="flex flex-col gap-4">
      <Link href="/forums" className="text-sm text-sky">
        {dict.forums.backToForum}
      </Link>
      <ol className="flex flex-col gap-3">
        {posts.map((p, i) => (
          <li
            key={p.id}
            className="rounded-[var(--r-md)] border border-line bg-white p-4 shadow-[var(--shadow-card)]"
          >
            <div className="flex items-baseline justify-between">
              <span className="font-bold text-sky">
                {p.username ?? dict.torrent.anonymous}
              </span>
              <span className="num text-xs text-sub">
                {fmt(dict.forums.floor, { n: i + 1 })}
              </span>
            </div>
            <p className="mt-2 whitespace-pre-wrap text-sm">{p.body}</p>
            <p className="mt-2 text-[11px] text-sub">
              {new Date(p.created_at).toLocaleString(dateLocale(locale))}
            </p>
          </li>
        ))}
      </ol>
    </div>
  );
}
