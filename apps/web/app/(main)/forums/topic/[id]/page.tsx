import Link from "next/link";
import { notFound } from "next/navigation";
import { getPosts } from "@/lib/data";
import { ReplyBox } from "@/components/forum-composer";
import { getDict } from "@/i18n/server";
import { dateLocale, fmt } from "@/i18n/config";

export const dynamic = "force-dynamic";

export default async function TopicPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const topicId = Number(id);
  const { dict, locale } = await getDict();
  if (!Number.isFinite(topicId)) notFound();
  const detail = await getPosts(topicId);
  if (!detail || detail.posts.length === 0) notFound();

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-baseline gap-2">
        <Link href="/forums" className="text-sm text-sky">
          {dict.forums.backToForums}
        </Link>
        {detail.forum_name && (
          <span className="text-sm text-sub">» {detail.forum_name}</span>
        )}
      </div>
      <h1 className="font-display text-2xl">{detail.title}</h1>
      <table className="nexus-table">
        <tbody>
          {detail.posts.map((p, i) => (
            <tr key={p.id} className="align-top">
              <td className="w-36 border-r border-line bg-[rgba(255,232,197,0.45)] p-3">
                <span className="font-bold text-sky">
                  {p.username ?? dict.torrent.anonymous}
                </span>
                <p className="mt-1 text-[11px] text-sub">
                  {fmt(dict.forums.floor, { n: i + 1 })}
                </p>
              </td>
              <td className="p-3">
                <p className="whitespace-pre-wrap text-sm">{p.body}</p>
                <p className="mt-2 text-[11px] text-sub">
                  {new Date(p.created_at).toLocaleString(dateLocale(locale))}
                </p>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <ReplyBox topicId={topicId} />
    </div>
  );
}
