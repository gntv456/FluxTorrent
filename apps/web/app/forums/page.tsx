import Link from "next/link";
import { getForums } from "@/lib/data";

export const dynamic = "force-dynamic";

export default async function ForumsPage() {
  const forums = await getForums();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">论坛</h1>
      <div className="overflow-hidden rounded-[var(--r-lg)] border border-line bg-white shadow-[var(--shadow-card)]">
        <table className="w-full text-sm">
          <thead>
            <tr className="bg-sky-soft text-left text-ink">
              <th className="px-4 py-3">版块</th>
              <th className="px-4 py-3 text-right">主题</th>
              <th className="hidden px-4 py-3 text-right sm:table-cell">帖子</th>
            </tr>
          </thead>
          <tbody>
            {forums.map((f, i) => (
              <tr key={f.id} className={i % 2 ? "bg-cloud/60" : ""}>
                <td className="px-4 py-3">
                  <Link
                    href={`/forums/${f.id}`}
                    className="font-bold text-ink hover:text-sky"
                  >
                    {f.name}
                  </Link>
                  {f.descr && (
                    <p className="text-xs text-sub">{f.descr}</p>
                  )}
                </td>
                <td className="num px-4 py-3 text-right">{f.topics}</td>
                <td className="num hidden px-4 py-3 text-right sm:table-cell">
                  {f.posts}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
