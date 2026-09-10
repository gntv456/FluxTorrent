import Link from "next/link";
import { getForums } from "@/lib/data";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

export default async function ForumsPage() {
  const { dict } = await getDict();
  const forums = await getForums();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.forums.title}</h1>
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">{dict.forums.board}</td>
            <td className="colhead w-24 text-right">{dict.forums.topics}</td>
            <td className="colhead hidden w-24 text-right sm:table-cell">
              {dict.forums.posts}
            </td>
          </tr>
        </thead>
        <tbody>
          {forums.map((f) => (
            <tr key={f.id}>
              <td>
                <Link
                  href={`/forums/${f.id}`}
                  className="font-bold text-ink hover:text-sky"
                >
                  {f.name}
                </Link>
                {f.descr && <p className="text-xs text-sub">{f.descr}</p>}
              </td>
              <td className="num text-right">{f.topics}</td>
              <td className="num hidden text-right sm:table-cell">{f.posts}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
