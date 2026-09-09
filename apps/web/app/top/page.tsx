import { getTopUsers } from "@/lib/data";
import { formatBytes } from "@/lib/format";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

export default async function TopPage() {
  const { dict } = await getDict();
  const users = await getTopUsers();
  const medal = (rank: number) =>
    rank === 1 ? "🥇" : rank === 2 ? "🥈" : rank === 3 ? "🥉" : `${rank}`;

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.top.title}</h1>
      <div className="overflow-hidden rounded-[var(--r-lg)] border border-line bg-white shadow-[var(--shadow-card)]">
        <table className="w-full text-sm">
          <thead>
            <tr className="bg-sky-soft text-left text-ink">
              <th className="px-3 py-3">#</th>
              <th className="px-3 py-3">{dict.top.user}</th>
              <th className="px-3 py-3">{dict.top.class}</th>
              <th className="px-3 py-3 text-right">{dict.top.uploaded}</th>
              <th className="hidden px-3 py-3 text-right sm:table-cell">
                {dict.top.seedSize}
              </th>
            </tr>
          </thead>
          <tbody>
            {users.map((u, i) => (
              <tr key={u.username} className={i % 2 ? "bg-cloud/60" : ""}>
                <td className="num px-3 py-2.5">{medal(u.rank)}</td>
                <td className="px-3 py-2.5 font-bold">{u.username}</td>
                <td className="px-3 py-2.5 text-sub">{u.class_name}</td>
                <td className="num px-3 py-2.5 text-right text-mint">
                  {formatBytes(u.uploaded)}
                </td>
                <td className="num hidden px-3 py-2.5 text-right text-sub sm:table-cell">
                  {formatBytes(u.seed_size)}
                </td>
              </tr>
            ))}
            {users.length === 0 && (
              <tr>
                <td colSpan={5} className="px-4 py-10 text-center text-sub">
                  {dict.top.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
