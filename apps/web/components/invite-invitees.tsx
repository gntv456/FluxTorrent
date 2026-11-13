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
  per,
  onPage,
  emptyTitle,
}: {
  invitees: Invitee[];
  t: Record<string, string>;
  total: number;
  page: number;
  /** 页大小（桌面 50 / 窄屏 10） */
  per: number;
  onPage: (p: number) => void;
  /** 空列表时面板仍在，只把表体换成这句话 */
  emptyTitle?: string;
  emptyDesc?: string;
}) {
  const pages = Math.ceil(total / per);
  return (
    // 2026-10-17 对齐样图集 § tplInvite（panelFlat）：与邀请码表同样包一层
    // 既有 .brand-panel 原语（原来是裸 section + h2），标题走
    // t.inviteesTitle。Pager 留在面板内底部。
    <section className="brand-panel">
      <div className="brand-panel__head">
        <h2>{t.inviteesTitle}</h2>
      </div>
      {invitees.length === 0 && (
        <p className="p-4 text-center text-sm text-sub">{emptyTitle ?? "—"}</p>
      )}
      {invitees.length > 0 && (
        <>
        {/* 手机：卡片（桌面那张五列表格在 390px 屏上要横滑才看得全） */}
        <ul className="flex flex-col gap-2 p-2 sm:hidden">
          {invitees.map((u) => (
            <li
              key={u.id}
              className={
                "rounded-[var(--r-md)] border border-[var(--compat-line)] " +
                "bg-[var(--compat-paper)] p-3"
              }
            >
              <div className="flex items-center gap-2">
                <a
                  href={`/users/${u.id}`}
                  className="text-[13px] font-bold text-sky-deep"
                >
                  {u.username}
                </a>
                {u.status >= 2 && (
                  <Badge tone="neutral">{t.invBanned}</Badge>
                )}
                <span className="ml-auto text-xs text-sub">
                  {u.class_name ?? "—"}
                </span>
              </div>
              <p className="mt-1 text-xs text-sub">
                {t.invUploaded} {fmtBytes(u.uploaded)}
                {" · "}
                {t.invDownloaded} {fmtBytes(u.downloaded)}
                {" · "}
                {new Date(u.created_at).toLocaleDateString()}
              </p>
            </li>
          ))}
        </ul>
        {/* 桌面：五列表格 */}
        <div className="brand-wide-table-scroll hidden sm:block">
          <table className="nexus-table">
            <tbody>
              <tr>
                <th scope="col" className="colhead">{t.invUser}</th>
                <th scope="col" className="colhead">{t.invLevel}</th>
                <th scope="col" className="colhead">{t.invUploaded}</th>
                <th scope="col" className="colhead">{t.invDownloaded}</th>
                <th scope="col" className="colhead">{t.invJoined}</th>
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
        </>
      )}
      {pages > 1 && (
        <div className="p-3 pt-0">
          <Pager
            layout="compact"
            page={page}
            pages={pages}
            posLabel={`${page} / ${pages}`}
            onStep={(p) => void onPage(p)}
          />
        </div>
      )}
    </section>
  );
}
