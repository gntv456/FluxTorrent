"use client";

import { formatBytes as fmtBytes } from "@/lib/format";
import { Badge } from "@/components/ui/badge";
import { Pager } from "@/components/ui/pager";

/** GET /invites/invitees 的一条被邀请人（0204 P0，NP invitee 口径） */
export interface Invitee {
  id: number;
  username: string;
  status: number;
  class_name: string | null;
  uploaded: number;
  downloaded: number;
  created_at: string;
}

/** 被邀请人列表（2026-10-16 自 invite-manager.tsx 按域外移，300 行门禁）。
 *
 *  ⚠️ 这张表曾经两度出问题：
 *  1. 后端是硬编码 `LIMIT 200` 且无分页参数——邀请超 200 人的账号看不到
 *     更早的记录（静默截断）；现已补分页，per 固定 50，与邀请记录同口径。
 *  2. 前端无条件全渲染——单账号 201 行、页高 10269px，把整页顶爆。
 *     现在改由 Pager 分页，一页 50 行；两张表的页码各自独立，不联动。
 */
export function InviteeList({
  invitees,
  t,
  total,
  page,
  onPage,
}: {
  invitees: Invitee[];
  t: Record<string, string>;
  total: number;
  page: number;
  onPage: (p: number) => void;
}) {
  const pages = Math.ceil(total / 50);
  return (
    <section className="flex flex-col gap-2">
      <h2 className="text-base font-bold">{t.inviteesTitle}</h2>
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead">{t.invUser}</td>
              <td className="colhead">{t.invLevel}</td>
              <td className="colhead">{t.invUploaded}</td>
              <td className="colhead">{t.invDownloaded}</td>
              <td className="colhead">{t.invJoined}</td>
            </tr>
            {invitees.map((u) => (
              <tr key={u.id}>
                <td>
                  <a
                    href={`/users/${u.id}`}
                    className="text-sky-deep hover:underline"
                  >
                    {u.username}
                  </a>
                  {u.status >= 2 && (
                    <Badge tone="neutral" className="ml-2">
                      {t.invBanned}
                    </Badge>
                  )}
                </td>
                <td className="text-xs text-sub">{u.class_name ?? "—"}</td>
                <td className="num text-xs">{fmtBytes(u.uploaded)}</td>
                <td className="num text-xs">{fmtBytes(u.downloaded)}</td>
                <td className="num text-xs text-sub">
                  {new Date(u.created_at).toLocaleDateString()}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {pages > 1 && (
        <Pager
          layout="compact"
          page={page}
          pages={pages}
          posLabel={`${page} / ${pages}`}
          onStep={(p) => void onPage(p)}
        />
      )}
    </section>
  );
}
