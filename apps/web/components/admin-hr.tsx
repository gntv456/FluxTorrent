"use client";

import { BTN_SM_GHOST, INPUT_CARD, INPUT_MD, INPUT_W32 } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 第八轮 P1-3：H&R 后台总览（好学站 user/hit-and-runs 口径）
 *  hr_snapshots 全量浏览 + 批量豁免（violated/open → pardoned） */

interface HrRow {
  user_id: number;
  username: string;
  torrent_id: number;
  torrent_name: string | null;
  uploaded: number;
  downloaded: number;
  required_seconds: number;
  seeded_seconds: number;
  deadline: string;
  status: "open" | "satisfied" | "violated" | "pardoned";
  pardoned_name: string | null;
  updated_at: string;
}

const STATUS: [string, string][] = [
  ["", "全部"],
  ["open", "考察中"],
  ["satisfied", "已达标"],
  ["violated", "已违规"],
  ["pardoned", "已豁免"],
];

function fmtHours(sec: number): string {
  return `${(sec / 3600).toFixed(1)}h`;
}

export function AdminHr() {
  const [rows, setRows] = useState<HrRow[]>([]);
  const [total, setTotal] = useState(0);
  const [uid, setUid] = useState("");
  const [status, setStatus] = useState("");
  const [page, setPage] = useState(1);
  const [note, setNote] = useState("");
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  };

  const load = useCallback(async () => {
    const p = new URLSearchParams({ page: String(page), per_page: "20" });
    if (uid.trim()) p.set("uid", uid.trim());
    if (status) p.set("status", status);
    try {
      const r = await api.get<{ rows: HrRow[]; total: number }>(
        `/api/v1/admin/hr/records?${p}`,
      );
      setRows(r.rows);
      setTotal(r.total);
      setSel(new Set());
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "加载失败");
    }
  }, [uid, status, page]);

  useEffect(() => {
    load();
  }, [load]);

  const key = (r: HrRow) => `${r.user_id}:${r.torrent_id}`;
  const toggle = (r: HrRow) => {
    setSel((prev) => {
      const n = new Set(prev);
      if (n.has(key(r))) n.delete(key(r));
      else n.add(key(r));
      return n;
    });
  };

  async function batchPardon() {
    if (sel.size === 0) return;
    if (!note.trim()) {
      flash("豁免必须填理由");
      return;
    }
    setBusy(true);
    try {
      const records = [...sel].map((k) => {
        const [user_id, torrent_id] = k.split(":").map(Number);
        return { user_id, torrent_id };
      });
      const r = await api.post<{ pardoned: number }>(
        "/api/v1/admin/hr/batch-pardon",
        { records, note },
      );
      flash(`已豁免 ${r.pardoned} 条`);
      setNote("");
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      <section className="flex flex-wrap items-end gap-2">
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
        <label className="flex flex-col gap-1 text-xs">
          状态
          <select
            value={status}
            onChange={(e) => {
              setStatus(e.target.value);
              setPage(1);
            }}
            className={INPUT_CARD}
          >
            {STATUS.map(([v, l]) => (
              <option key={v} value={v}>
                {l}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-1 flex-col gap-1 text-xs">
          批量豁免理由（必填，随 PM 通知）
          <input
            value={note}
            onChange={(e) => setNote(e.target.value)}
            placeholder="例：故障日补偿"
            className={INPUT_MD}
          />
        </label>
        <button
          disabled={busy || sel.size === 0}
          onClick={batchPardon}
          className="min-h-[40px] rounded-full bg-mint px-5 text-sm font-bold text-white disabled:opacity-50"
        >
          豁免所选（{sel.size}）
        </button>
      </section>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead w-10"></td>
            <td className="colhead">用户</td>
            <td className="colhead">种子</td>
            <td className="colhead">上传/下载</td>
            <td className="colhead">分享率</td>
            <td className="colhead">已做种/要求</td>
            <td className="colhead">还需做种</td>
            <td className="colhead">截止</td>
            <td className="colhead">状态</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr
              key={key(r)}
              className={r.status === "violated" ? "bg-coral/5" : ""}
            >
              <td>
                <input
                  type="checkbox"
                  checked={sel.has(key(r))}
                  onChange={() => toggle(r)}
                  disabled={r.status === "pardoned" || r.status === "satisfied"}
                />
              </td>
              <td>
                <a
                  href={`/admin/users/${r.user_id}`}
                  className="font-bold text-link"
                >
                  {r.username}
                </a>
              </td>
              <td className="max-w-[200px] truncate">
                <a href={`/torrent/${r.torrent_id}`} className="text-link">
                  {r.torrent_name ?? `#${r.torrent_id}`}
                </a>
              </td>
              <td className="num">
                {(r.uploaded / 1024 ** 3).toFixed(1)}G /{" "}
                {(r.downloaded / 1024 ** 3).toFixed(1)}G
              </td>
              <td className="num">
                {r.downloaded > 0
                  ? (r.uploaded / r.downloaded).toFixed(2)
                  : "∞"}
              </td>
              <td className="num">
                {fmtHours(r.seeded_seconds)} / {fmtHours(r.required_seconds)}
              </td>
              <td
                className={`num ${r.status === "violated" ? "text-danger" : ""}`}
              >
                {Math.max(0, r.required_seconds - r.seeded_seconds) > 0
                  ? fmtHours(r.required_seconds - r.seeded_seconds)
                  : "—"}
              </td>
              <td className="text-sub">
                {new Date(r.deadline).toLocaleDateString()}
              </td>
              <td>
                {r.status === "open" ? (
                  "考察中"
                ) : r.status === "satisfied" ? (
                  "已达标"
                ) : r.status === "violated" ? (
                  <span className="text-danger">已违规</span>
                ) : (
                  `已豁免${r.pardoned_name ? `(${r.pardoned_name})` : ""}`
                )}
              </td>
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td colSpan={9} className="py-6 text-center text-sub">
                暂无 H&R 记录
              </td>
            </tr>
          )}
        </tbody>
      </table>
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
            disabled={rows.length < 20}
            onClick={() => setPage(page + 1)}
            className={BTN_SM_GHOST}
          >
            下一页
          </button>
        </div>
      </div>
    </div>
  );
}
