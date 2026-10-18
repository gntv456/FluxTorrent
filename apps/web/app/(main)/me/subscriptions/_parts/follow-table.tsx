import Link from "next/link";
import type { FollowEntry } from "@fluxtorrent/domain-types";
import { FollowButton } from "@/components/forum-follow";
import { EmptyState } from "@/components/ui/empty-state";

/** 关注名单表（样图 § subscriptions 的「我关注的作者 / 我订阅的版块」）。
 *
 *  两档的行形状本来就一样：名字链去对象页 + 一个取关按钮。抽出来是为了
 *  路由页别再复制第二份表（那页当时正踩 300 行门禁），也顺手让「版块」档
 *  真的接上——`/follows/mine` 一直回 `forums`，此前站内没有任何出口。
 *
 *  名单本身就来自 `/follows/mine`，所以 `initial.following` 直接给 true，
 *  不逐行再打一次状态。 */
export function FollowTable({
  rows,
  hrefPrefix,
  colLabel,
  actionLabel,
  targetType,
  emptyIcon,
  emptyTitle,
  emptyDesc,
}: {
  rows: FollowEntry[];
  /** 详情前缀：作者 `/users/`、版块 `/forums/` */
  hrefPrefix: string;
  colLabel: string;
  actionLabel: string;
  targetType: "user" | "forum";
  emptyIcon: string;
  emptyTitle: string;
  emptyDesc: string;
}) {
  if (rows.length === 0) {
    return (
      <EmptyState
        icon={emptyIcon}
        title={emptyTitle}
        desc={emptyDesc}
      />
    );
  }
  return (
    <div className="baozi-panel overflow-x-auto">
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">{colLabel}</td>
            <td className="colhead w-32 text-right">{actionLabel}</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={`${targetType}-${r.id}`}>
              <td>
                <Link
                  href={`${hrefPrefix}${r.id}`}
                  className="font-bold text-ink"
                >
                  {r.name}
                </Link>
              </td>
              <td className="text-right">
                <FollowButton
                  targetType={targetType}
                  targetId={r.id}
                  initial={{
                    following: true,
                    followers: 0,
                    is_self: false,
                  }}
                />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
