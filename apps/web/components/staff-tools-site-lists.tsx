"use client";

/**
 * 站点域·只读列表子面板（从 components/staff-tools-site.tsx 按域拆出）：
 * 无法连接的用户（notconnectable）/ 上传者（uploaders）/ 客户端（allagents）
 * / 投票总览（polloverview）。数据加载留在父组件。
 */

import { useI18n } from "@/i18n/client";
import type { AgentRow, PollRow } from "./staff-tools-site-shared";

interface SiteListsProps {
  tab: "notconnect" | "uploaders" | "agents" | "polls";
  notConnectRows: {
    id: number;
    username: string;
    torrents: number;
    last_seen_at: string | null;
  }[];
  uploaderRows: {
    id: number;
    username: string;
    uploads: number;
    seeding: number;
    total_size: number;
  }[];
  agentRows: AgentRow[];
  pollRows: PollRow[];
}

export function StaffSiteLists({
  tab,
  notConnectRows,
  uploaderRows,
  agentRows,
  pollRows,
}: SiteListsProps) {
  const { dict } = useI18n();
  const t = dict.stafftools;
  return (
    <>
      {/* 无法连接的用户（notconnectable） */}
      {tab === "notconnect" && (
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead">ID</td>
              <td className="colhead">{t.mlUser}</td>
              <td className="colhead">{t.ncTorrents}</td>
              <td className="colhead">{t.ncLastSeen}</td>
            </tr>
            {notConnectRows.map((r) => (
              <tr key={r.id}>
                <td className="num">{r.id}</td>
                <td>{r.username}</td>
                <td className="num">{r.torrents}</td>
                <td className="text-xs text-sub">
                  {r.last_seen_at
                    ? new Date(r.last_seen_at).toLocaleString("zh-CN")
                    : "—"}
                </td>
              </tr>
            ))}
            {notConnectRows.length === 0 && (
              <tr>
                <td colSpan={4} className="py-6 text-center text-sub">
                  {t.ncEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      )}

      {/* 上传者（uploaders） */}
      {tab === "uploaders" && (
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead">ID</td>
              <td className="colhead">{t.mlUser}</td>
              <td className="colhead">{t.ulpUploads}</td>
              <td className="colhead">{t.stSeeding}</td>
              <td className="colhead">{t.ulpSize}</td>
            </tr>
            {uploaderRows.map((r) => (
              <tr key={r.id}>
                <td className="num">{r.id}</td>
                <td>{r.username}</td>
                <td className="num">{r.uploads}</td>
                <td className="num">{r.seeding}</td>
                <td className="num">
                  {(r.total_size / 1024 ** 3).toFixed(2)} GB
                </td>
              </tr>
            ))}
            {uploaderRows.length === 0 && (
              <tr>
                <td colSpan={5} className="py-6 text-center text-sub">
                  {t.ulpEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      )}

      {/* 全部客户端（allagents） */}
      {tab === "agents" && (
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead">{t.agAgent}</td>
              <td className="colhead">{t.agPeers}</td>
            </tr>
            {agentRows.map((r) => (
              <tr key={r.agent}>
                <td className="font-mono">{r.agent}</td>
                <td className="num">{r.peers}</td>
              </tr>
            ))}
            {agentRows.length === 0 && (
              <tr>
                <td colSpan={2} className="py-6 text-center text-sub">
                  {t.agEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      )}

      {/* 投票总览（polloverview） */}
      {tab === "polls" && (
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead">ID</td>
              <td className="colhead">{t.plQuestion}</td>
              <td className="colhead">{t.plVotes}</td>
              <td className="colhead">{t.plStatus}</td>
            </tr>
            {pollRows.map((p) => (
              <tr key={p.id}>
                <td className="num">{p.id}</td>
                <td>{p.question}</td>
                <td className="num">{p.votes}</td>
                <td>{p.closed ? t.plClosed : t.plOpen}</td>
              </tr>
            ))}
            {pollRows.length === 0 && (
              <tr>
                <td colSpan={4} className="py-6 text-center text-sub">
                  {t.plEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      )}
    </>
  );
}
