"use client";

import { BTN_SM_GHOST, CELL_CARD_SUB, INPUT_W32 } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

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
      setMsg(e instanceof ApiError ? e.message : "加载失败");
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
              ["rename", "改名记录"],
              ["modify", "修改记录"],
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
          用户 UID
          <input
            value={uid}
            onChange={(e) => {
              setUid(e.target.value);
              setPage(1);
            }}
            placeholder="留空看全部"
            className={INPUT_W32}
          />
        </label>
      </div>

      {sub === "rename" ? (
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead">ID</td>
              <td className="colhead">用户</td>
              <td className="colhead">旧用户名</td>
              <td className="colhead">新用户名</td>
              <td className="colhead">操作者</td>
              <td className="colhead">时间</td>
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
                  {new Date(r.created_at).toLocaleString()}
                </td>
              </tr>
            ))}
            {renames.length === 0 && (
              <tr>
                <td colSpan={6} className="py-6 text-center text-sub">
                  暂无改名记录
                </td>
              </tr>
            )}
          </tbody>
        </table>
      ) : (
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead">ID</td>
              <td className="colhead">用户</td>
              <td className="colhead">修改内容</td>
              <td className="colhead">操作者</td>
              <td className="colhead">时间</td>
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
                  {new Date(r.created_at).toLocaleString()}
                </td>
              </tr>
            ))}
            {modifies.length === 0 && (
              <tr>
                <td colSpan={5} className="py-6 text-center text-sub">
                  暂无修改记录
                </td>
              </tr>
            )}
          </tbody>
        </table>
      )}
      <div className="flex items-center justify-between text-sm text-sub">
        <span>共 {total} 条</span>
        <div className="flex gap-2">
          <button
            disabled={page <= 1}
            onClick={() => setPage(page - 1)}
            className={BTN_SM_GHOST}
          >
            上一页
          </button>
          <span>第 {page} 页</span>
          <button
            disabled={false}
            onClick={() => setPage(page + 1)}
            className="min-h-[36px] rounded-full border border-line px-3"
          >
            下一页
          </button>
        </div>
      </div>
    </div>
  );
}
