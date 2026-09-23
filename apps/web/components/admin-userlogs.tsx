"use client";

import { BTN_SM_GHOST, CELL_CARD_SUB, INPUT_W32 } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";

/** 第八轮 P2-6：用户记录（好学站 UsernameChangeLog / UserModifyLog 口径）
 *  改名记录 + 资料修改记录两个子页签 */

interface RenameRow {
  id: number;
  uid: number;
  username: string;
  old_name: string;
  new_name: string;
  operator_name: string | null;
  created_at: string;
}

interface ModifyRow {
  id: number;
  uid: number;
  username: string;
  modifier_name: string | null;
  content: string;
  created_at: string;
}

export function AdminUserLogs() {
  const { dict, locale } = useI18n();
  const at = dict.adminUserlogs;
  const c = dict.common;
  const [sub, setSub] = useState<"rename" | "modify">("rename");
  const [uid, setUid] = useState("");
  const [page, setPage] = useState(1);
  const [renames, setRenames] = useState<RenameRow[]>([]);
  const [modifies, setModifies] = useState<ModifyRow[]>([]);
  const [total, setTotal] = useState(0);
  const [msg, setMsg] = useState<string | null>(null);

  const load = useCallback(async () => {
    const p = new URLSearchParams({ page: String(page), per_page: "20" });
    if (uid.trim()) p.set("uid", uid.trim());
    try {
      if (sub === "rename") {
        const r = await api.get<{ rows: RenameRow[]; total: number }>(
          `/api/v1/admin/rename-logs?${p}`,
        );
        setRenames(r.rows);
        setTotal(r.total);
      } else {
        const r = await api.get<{ rows: ModifyRow[]; total: number }>(
          `/api/v1/admin/modify-logs?${p}`,
        );
        setModifies(r.rows);
        setTotal(r.total);
      }
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : at.loadFail);
    }
  }, [sub, uid, page]);

  useEffect(() => {
    load();
  }, [load]);

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      <div className="flex flex-wrap items-end gap-2">
        <div className="flex gap-2">
          {(
            [
              ["rename", at.tabRename],
              ["modify", at.tabModify],
            ] as const
          ).map(([k, l]) => (
            <button
              key={k}
              onClick={() => {
                setSub(k);
                setPage(1);
              }}
              className={`min-h-[40px] rounded-full px-4 text-sm font-bold ${sub === k ? "bg-sky text-white" : CELL_CARD_SUB}`}
            >
              {l}
            </button>
          ))}
        </div>
        <label className="flex flex-col gap-1 text-xs">
          {at.fUid}
          <input
            value={uid}
            onChange={(e) => {
              setUid(e.target.value);
              setPage(1);
            }}
            placeholder={at.qAll}
            className={INPUT_W32}
          />
        </label>
      </div>

      {sub === "rename" ? (
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead">{at.thId}</td>
              <td className="colhead">{at.thUser}</td>
              <td className="colhead">{at.thOldName}</td>
              <td className="colhead">{at.thNewName}</td>
              <td className="colhead">{at.thOperator}</td>
              <td className="colhead">{at.thTime}</td>
            </tr>
          </thead>
          <tbody>
            {renames.map((r) => (
              <tr key={r.id}>
                <td className="num">{r.id}</td>
                <td>
                  <a
                    href={`/admin/users/${r.uid}`}
                    className="font-bold text-link"
                  >
                    {r.username}
                  </a>
                </td>
                <td>{r.old_name}</td>
                <td className="font-bold">{r.new_name}</td>
                <td>{r.operator_name ?? "—"}</td>
                <td className="text-sub">
                  {new Date(r.created_at).toLocaleString(dateLocale(locale))}
                </td>
              </tr>
            ))}
            {renames.length === 0 && (
              <tr>
                <td colSpan={6} className="py-6 text-center text-sub">
                  {at.renameEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      ) : (
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead">{at.thId}</td>
              <td className="colhead">{at.thUser}</td>
              <td className="colhead">{at.thContent}</td>
              <td className="colhead">{at.thOperator}</td>
              <td className="colhead">{at.thTime}</td>
            </tr>
          </thead>
          <tbody>
            {modifies.map((r) => (
              <tr key={r.id}>
                <td className="num">{r.id}</td>
                <td>
                  <a
                    href={`/admin/users/${r.uid}`}
                    className="font-bold text-link"
                  >
                    {r.username}
                  </a>
                </td>
                <td>{r.content}</td>
                <td>{r.modifier_name ?? "—"}</td>
                <td className="text-sub">
                  {new Date(r.created_at).toLocaleString(dateLocale(locale))}
                </td>
              </tr>
            ))}
            {modifies.length === 0 && (
              <tr>
                <td colSpan={5} className="py-6 text-center text-sub">
                  {at.modifyEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      )}
      <div className="flex items-center justify-between text-sm text-sub">
        <span>{fmt(c.totalItems, { n: total })}</span>
        <div className="flex gap-2">
          <button
            disabled={page <= 1}
            onClick={() => setPage(page - 1)}
            className={BTN_SM_GHOST}
          >
            {c.prevPage}
          </button>
          <span>{fmt(c.pageX, { n: page })}</span>
          <button
            disabled={false}
            onClick={() => setPage(page + 1)}
            className="min-h-[36px] rounded-full border border-line px-3"
          >
            {c.nextPage}
          </button>
        </div>
      </div>
    </div>
  );
}
