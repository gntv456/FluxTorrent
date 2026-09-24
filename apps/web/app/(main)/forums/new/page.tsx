import Link from "next/link";
import { getForums } from "@/lib/data";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

/** 发主题选版块：只列 can_create（read+write+create 三档全过 或 版主）的版块 */
export default async function NewTopicPage() {
  const { dict } = await getDict();
  const forums = (await getForums()).filter((f) => f.can_create);

  return (
    <div className="flex flex-col gap-4">
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">New Topic</div>
          <h1 className="font-display text-2xl">{dict.forums.newTopic}</h1>
        </div>
        <div className="aside">
          <Link href="/forums" className="text-sm text-sky">
            {dict.forums.backToForums}
          </Link>
        </div>
      </div>
      {forums.length === 0 ? (
        <p className="py-10 text-center text-sub">
          暂无可发帖的版块（需通过版块的 读/回/发 三档门槛）
        </p>
      ) : (
        <ul className="flex flex-col gap-2">
          {forums.map((f) => (
            <li key={f.id}>
              <Link
                href={`/forums/${f.id}`}
                className="flex items-center justify-between rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4 hover:border-sky"
              >
                <span>
                  <span className="font-bold text-ink">{f.name}</span>
                  {f.descr && (
                    <span className="ml-2 text-sm text-sub">{f.descr}</span>
                  )}
                </span>
                <span className="text-sm text-sky">
                  {dict.forums.newTopic} →
                </span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
