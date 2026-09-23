"use client";

import {
  approvalList,
  promoLabels,
  fmtBytes,
  type AdminTorrentRow,
} from "./admin-torrents-shared";
import { useI18n } from "@/i18n/client";

/** 种子批量工作台·列表表格（从 admin-torrents-list.tsx 按域拆出）。 */
export function TorrentTable(props: {
  data: { page: number; rows: AdminTorrentRow[] } | null;
  sel: Set<number>;
  setSel: React.Dispatch<React.SetStateAction<Set<number>>>;
  decide: (id: number, approve: boolean) => void;
  hr?: boolean;
}) {
  const { data, sel, setSel, decide } = props;
  const { dict } = useI18n();
  const at = dict.adminTorrents;
  const APPROVAL = approvalList(at.approval);
  const PROMO_LABEL = promoLabels(at.promo);
  return (
    <table className="nexus-table">
      <thead>
        <tr>
          <td className="colhead w-10"></td>
          <td className="colhead">ID</td>
          <td className="colhead">{at.fName}</td>
          <td className="colhead">{at.fOwner}</td>
          <td className="colhead">{at.thSize}</td>
          <td className="colhead">{at.thSeeders}</td>
          <td className="colhead">{at.fStatus}</td>
          <td className="colhead">{at.fPos}</td>
          <td className="colhead">{at.fPromo}</td>
          <td className="colhead">{at.fPick}</td>
          <td className="colhead">H&R</td>
          <td className="colhead">{at.thAction}</td>
        </tr>
      </thead>
      <tbody>
        {data?.rows.map((t) => (
          <tr key={t.id}>
            <td>
              <input
                type="checkbox"
                checked={sel.has(t.id)}
                onChange={(e) =>
                  setSel((prev) => {
                    const n = new Set(prev);
                    if (e.target.checked) n.add(t.id);
                    else n.delete(t.id);
                    return n;
                  })
                }
              />
            </td>
            <td>{t.id}</td>
            <td className="max-w-[240px] truncate">
              <a className="font-bold text-link" href={`/torrents?id=${t.id}`}>
                {t.name}
              </a>
              {t.deny_reason && (
                <span className="ml-1 text-xs text-danger">
                  {t.deny_reason}
                </span>
              )}
            </td>
            <td className="text-xs">{t.owner_name ?? "—"}</td>
            <td className="text-xs">{fmtBytes(t.size)}</td>
            <td>
              {t.seeders} / {t.leechers}
            </td>
            <td>{APPROVAL[t.approval_status] ?? t.approval_status}</td>
            <td>
              {t.pos_state === 1 ? (
                <span className="text-sky">
                  {at.fPos}
                  {t.pos_state_until
                    ? `·${new Date(t.pos_state_until).toLocaleDateString()}`
                    : ""}
                </span>
              ) : (
                "—"
              )}
            </td>
            <td>
              {t.promotion ? (
                <span className="text-mint">
                  {PROMO_LABEL[t.promotion] ?? t.promotion}
                  {t.promotion_ends_at
                    ? `·${new Date(t.promotion_ends_at).toLocaleDateString()}`
                    : ""}
                </span>
              ) : (
                "—"
              )}
            </td>
            <td>
              {t.pick_type === 1 ? (
                <span className="text-danger">{at.optPickRec}</span>
              ) : t.pick_type === 2 ? (
                <span className="text-sun">{at.optPickClassic}</span>
              ) : (
                "—"
              )}
            </td>
            <td>{t.hr ? <span className="text-danger">H&R</span> : "—"}</td>
            <td>
              {t.approval_status === 0 && (
                <>
                  <button
                    className="cmgmt-act"
                    onClick={() => decide(t.id, true)}
                  >
                    {APPROVAL[1]}
                  </button>
                  <button
                    className="cmgmt-act cmgmt-act--danger"
                    onClick={() => decide(t.id, false)}
                  >
                    {APPROVAL[2]}
                  </button>
                </>
              )}
            </td>
          </tr>
        ))}
        {data?.rows.length === 0 && (
          <tr>
            <td colSpan={12} className="py-6 text-center text-sub">
              {at.tableEmpty}
            </td>
          </tr>
        )}
      </tbody>
    </table>
  );
}
