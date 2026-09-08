import Link from "next/link";
import { getPosts } from "@/lib/data";

export const dynamic = "force-dynamic";

export default async function TopicPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const posts = await getPosts(Number(id));

  return (
    <div className="flex flex-col gap-4">
      <Link href="/forums" className="text-sm text-sky">
        ← 返回论坛
      </Link>
      <ol className="flex flex-col gap-3">
        {posts.map((p, i) => (
          <li
            key={p.id}
            className="rounded-[var(--r-md)] border border-line bg-white p-4 shadow-[var(--shadow-card)]"
          >
            <div className="flex items-baseline justify-between">
              <span className="font-bold text-sky">{p.username ?? "匿名"}</span>
              <span className="num text-xs text-sub">#{i + 1} 楼</span>
            </div>
            <p className="mt-2 whitespace-pre-wrap text-sm">{p.body}</p>
            <p className="mt-2 text-[11px] text-sub">
              {new Date(p.created_at).toLocaleString("zh-CN")}
            </p>
          </li>
        ))}
      </ol>
    </div>
  );
}
