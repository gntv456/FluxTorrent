import Link from "next/link";
import { notFound } from "next/navigation";
import { getTopics } from "@/lib/data";
import { getDict } from "@/i18n/server";
import { fmt } from "@/i18n/config";

export const dynamic = "force-dynamic";

export default async function ForumPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const { dict } = await getDict();
  const forumId = Number(id);
  if (!Number.isFinite(forumId)) notFound();
  const topics = await getTopics(forumId);

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-baseline gap-2">
        <Link href="/forums" className="text-sm text-sky">
          {dict.forums.backToForums}
        </Link>
      </div>
      <h1 className="font-display text-2xl">
        {fmt(dict.forums.boardTitle, { id: forumId })}
      </h1>
      {topics.length === 0 ? (
        <p className="py-10 text-center text-sub">{dict.forums.noTopics}</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {topics.map((t) => (
            <li key={t.id}>
              <Link
                href={`/forums/topic/${t.id}`}
                className="flex items-center justify-between rounded-[var(--r-md)] border border-line bg-white p-3 shadow-[var(--shadow-card)] hover:-translate-y-0.5"
              >
                <div className="min-w-0">
                  <p className="truncate font-bold">{t.title}</p>
                  <p className="text-xs text-sub">
                    {t.username ?? "—"} ·{" "}
                    {fmt(dict.forums.views, { n: t.views })}
                  </p>
                </div>
                <span className="num shrink-0 text-sm text-sub">
                  {fmt(dict.forums.replies, { n: t.replies })}
                </span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
