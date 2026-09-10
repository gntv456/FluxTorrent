import Link from "next/link";
import { getTopUsers } from "@/lib/data";
import { formatBytes } from "@/lib/format";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

/** 排行榜（NexusPHP 经典表格：colhead 渐变表头 + rowfollow 行 + 前三名奖牌） */
export default async function TopPage() {
  const { dict } = await getDict();
  const users = await getTopUsers();
  const medal = (rank: number) =>
    rank === 1 ? "🥇" : rank === 2 ? "🥈" : rank === 3 ? "🥉" : `${rank}`;

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.top.title}</h1>
      <table className="nexus-table">
        <thead>
          <tr>
            <th className="w-14">#</th>
            <th>{dict.top.user}</th>
            <th>{dict.top.class}</th>
            <th className="text-right">{dict.top.uploaded}</th>
            <th className="text-right">{dict.top.downloaded}</th>
            <th className="hidden text-right sm:table-cell">{dict.top.seedSize}</th>
          </tr>
        </thead>
        <tbody>
          {users.map((u) => (
            <tr key={u.username}>
              <td className="num">{medal(u.rank)}</td>
              <td className="font-bold text-sky">{u.username}</td>
              <td className="text-sub">{u.class_name}</td>
              <td className="num text-right">{formatBytes(u.uploaded)}</td>
              <td className="num text-right text-sub">{formatBytes(u.downloaded)}</td>
              <td className="num hidden text-right text-sub sm:table-cell">
                {formatBytes(u.seed_size)}
              </td>
            </tr>
          ))}
          {users.length === 0 && (
            <tr>
              <td colSpan={6} className="py-10 text-center text-sub">
                {dict.top.empty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
      <p className="text-xs text-sub">
        <Link href="/" className="text-sky">
          {dict.nav.home}
        </Link>{" "}
        » {dict.top.title}
      </p>
    </div>
  );
}
