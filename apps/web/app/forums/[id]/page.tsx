import Link from "next/link";
import { notFound } from "next/navigation";
import { getTopics } from "@/lib/data";

export const dynamic = "force-dynamic";

export default async function ForumPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const forumId = Number(id);
  if (!Number.isFinite(forumId)) notFound();
  const topics = await getTopics(forumId);

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-baseline gap-2">
        <Link href="/forums" className="text-sm text-sky">
          ← 论坛
        </Link>
      </div>
      <h1 className="font-display text-2xl">版块 #{forumId}</h1>
      {topics.length === 0 ? (
        <p className="py-10 text-center text-sub">还没有主题，来发第一帖吧</p>
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
                    {t.username ?? "—"} · 浏览 {t.views}
                  </p>
                </div>
                <span className="num shrink-0 text-sm text-sub">
                  回复 {t.replies}
                </span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
