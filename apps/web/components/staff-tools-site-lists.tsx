"use client";

/**
 * 站点域·只读列表子面板（从 components/staff-tools-site.tsx 按域拆出）：
 * 无法连接的用户（notconnectable）/ 上传者（uploaders）/ 客户端（allagents）
 * / 投票总览（polloverview）。数据加载留在父组件。
 */

import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import type { AgentRow, ConnDistRow, PollRow } from "./staff-tools-site-shared";

/** connectable 四档的中文标签与提示（0310）。
 *  -2 与 0 必须分开显示：前者是「查不出来」，后者是「查出来了，不可信」。 */
const CONN_STATE_META: Record<number, { label: string; hint: string }> = {
  [-1]: { label: "未测", hint: "本轮未被抽中探测，不作判定" },
  [-2]: {
    label: "无法验证",
    hint: "端口可连但不响应明文 BT 协议：多为仅加密连接客户端 / MSE-PE / peer 白名单 / CGNAT。通用站不应据此惩罚",
  },
  0: {
    label: "不可信",
    hint: "实锤：端口不通，或 piece SHA-1 与 info.pieces 不符（伪造数据）",
  },
  1: { label: "可信", hint: "BT 握手 + bitfield（+ 抽样 piece 哈希）全部通过" },
};

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
  /** 做种结论四档分布（0310）：-2 无法验证 / 0 不可信 / -1 未测 / 1 可信 */
  connDist?: ConnDistRow[];
}

export function StaffSiteLists({
  tab,
  notConnectRows,
  uploaderRows,
  agentRows,
  pollRows,
  connDist,
}: SiteListsProps) {
  const { dict, locale } = useI18n();
  const t = dict.stafftools;
  return (
    <>
      {/* 无法连接的用户（notconnectable） */}
      {tab === "notconnect" && (
        <>
        {/* 四档分布（0310）：先把「无法验证」与「不可信」摆在一起，再决定
            要不要收紧口径。-2 占比高 = 站内多加密客户端/白名单用户，
            此时处罚他们等于自伤做种供给。 */}
        {connDist && connDist.length > 0 && (
          <div className="mb-3 flex flex-wrap gap-2 text-xs">
            {connDist.map((d) => {
              const meta = CONN_STATE_META[d.state];
              if (!meta) return null;
              return (
                <span
                  key={d.state}
                  className="rounded border border-line px-2 py-1"
                  title={meta.hint}
                >
                  <span className="font-bold">{meta.label}</span>
                  <span className="ml-2 num">{d.n}</span>
                  <span className="ml-1 text-sub">({d.users} 人)</span>
                </span>
              );
            })}
          </div>
        )}
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
                    ? new Date(r.last_seen_at).toLocaleString(
                        dateLocale(locale),
                      )
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
        </>
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
