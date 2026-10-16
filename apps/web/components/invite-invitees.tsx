"use client";

import { useState } from "react";
import { formatBytes as fmtBytes } from "@/lib/format";
import { Badge } from "@/components/ui/badge";

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
 *  ⚠️ 默认只显示最近 20 条：后端 `/invites/invitees` 是硬编码 `LIMIT 200`
 *  且**没有分页参数**（同文件的邀请记录倒是有分页的），重度用户会把整页
 *  顶爆——实测单账号 201 行、页高 10269px。这里先在前端收起来，
 *  后端补分页是后续项（>200 的历史目前会被静默截断）。
 */
const PREVIEW = 20;

export function InviteeList({
  invitees,
  t,
}: {
  invitees: Invitee[];
  t: Record<string, string>;
}) {
  const [all, setAll] = useState(false);
  const shown = all ? invitees : invitees.slice(0, PREVIEW);
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
            {shown.map((u) => (
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
      {invitees.length > PREVIEW && (
        <button
          type="button"
          onClick={() => setAll(!all)}
          className="self-start text-xs text-sky-deep hover:underline"
        >
          {all
            ? t.inviteeCollapse
            : `${t.inviteeShowAll}（${invitees.length}）`}
        </button>
      )}
    </section>
  );
}
