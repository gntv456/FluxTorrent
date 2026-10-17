import Link from "next/link";
import { FollowButton } from "@/components/forum-follow";
import type { Dict } from "@/i18n/server";
import type { MyFollows } from "@fluxtorrent/domain-types";

/** 关注流侧栏（样图集 § forum-feed 右列两块）：我关注的版块 / 我关注的作者。
 *
 *  每行带一个「取消关注」——这两张列表本身就来自 `/follows/mine`，
 *  所以关注态是已知的，给 FollowButton 传 `initial` 省掉逐行探测
 *  （一屏十几行就是十几次 status 往返）。 */
export function FollowSide({
  mine,
  dict,
}: {
  mine: MyFollows;
  dict: Dict;
}) {
  const t = dict.forums3;
  const groups: [string, "forum" | "user", { id: number; name: string }[]][] =
    [
      [t.sideBoards, "forum", mine.forums],
      [t.sideUsers, "user", mine.users],
    ];
  return (
    <aside className="ff-side" aria-label={t.sideTitle}>
      {groups.map(([title, kind, rows]) => (
        <section key={kind} className="side-card">
          <h2 className="side-card__title">
            {title}
            <span className="num ff-side__n">{rows.length}</span>
          </h2>
          {rows.length === 0 ? (
            <p className="side-empty">
              {kind === "forum" ? t.sideEmptyBoards : t.sideEmptyUsers}
            </p>
          ) : (
            <ul className="ff-side__list">
              {rows.map((r) => (
                <li key={`${kind}-${r.id}`} className="ff-side__row">
                  <Link
                    href={
                      kind === "forum"
                        ? `/forums/${r.id}`
                        : `/users/${r.id}`
                    }
                    className="ff-side__name"
                  >
                    {r.name}
                  </Link>
                  <FollowButton
                    targetType={kind}
                    targetId={r.id}
                    initial={{
                      following: true,
                      followers: 0,
                      is_self: false,
                    }}
                  />
                </li>
              ))}
            </ul>
          )}
        </section>
      ))}
    </aside>
  );
}
