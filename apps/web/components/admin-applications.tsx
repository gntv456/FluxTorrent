"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 申请制入站后台队列（0227）：待审优先排序，通过/拒绝可附备注。
 *  通过后端自动生成绑定邮箱的邀请码并邮件通知申请人。 */

interface ApplicationRow {
  id: number;
  username: string;
  email: string;
  proof_links: string;
  reason: string;
  status: number; // 0 待审 1 通过 2 拒绝
}

const STATUS_LABEL: Record<number, string> = {
  0: "待审",
  1: "已通过",
  2: "已拒绝",
};

export function AdminApplications() {
  const [rows, setRows] = useState<ApplicationRow[] | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [notes, setNotes] = useState<Record<number, string>>({});
  const [busy, setBusy] = useState<number | null>(null);

  const load = useCallback(async () => {
    try {
      const r = await api.get<{ items: ApplicationRow[] }>(
        "/api/v1/admin/applications",
      );
      setRows(r.items ?? []);
    } catch {
      setRows([]);
    }
  }, []);
  useEffect(() => {
    void load();
  }, [load]);

  async function decide(id: number, action: "approve" | "reject") {
    setBusy(id);
    setMsg(null);
    try {
      const r = await api.post<{ invite_code: string | null }>(
        "/api/v1/admin/applications/decide",
        { id, action, note: notes[id] ?? "" },
      );
      setMsg(
        action === "approve"
          ? `已通过，邀请码 ${r.invite_code ?? "?"}（已邮件通知）`
          : "已拒绝（已邮件通知）",
      );
      await load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : String(e));
    } finally {
      setBusy(null);
    }
  }

  if (!rows) return <p className="py-8 text-center text-sub">…</p>;

  return (
    <section className="space-y-3">
      <h2 className="font-display text-lg">入站申请审核</h2>
      {msg && <p className="text-sm">{msg}</p>}
      {rows.length === 0 && (
        <p className="py-6 text-center text-sub">暂无申请</p>
      )}
      {rows.map((a) => (
        <div key={a.id} className="baozi-panel space-y-2 p-4">
          <div className="flex flex-wrap items-center gap-2 text-sm">
            <b>{a.username}</b>
            <span className="text-sub">{a.email}</span>
            <span
              className={`rounded-full px-2 py-0.5 text-xs ${
                a.status === 0
                  ? "bg-warn/20"
                  : a.status === 1
                    ? "bg-mint/20"
                    : "bg-danger/20"
              }`}
            >
              {STATUS_LABEL[a.status]}
            </span>
          </div>
          <p className="text-xs text-sub">
            佐证：
            {a.proof_links.split(/\n|,|\s{2,}/).filter(Boolean).join(" ｜ ")}
          </p>
          <p className="text-xs">{a.reason}</p>
          {a.status === 0 && (
            <div className="flex flex-wrap items-center gap-2">
              <input
                value={notes[a.id] ?? ""}
                onChange={(e) =>
                  setNotes((p) => ({ ...p, [a.id]: e.target.value }))
                }
                placeholder="备注（拒绝理由会随邮件发出，可留空）"
                className="min-w-0 flex-1 rounded border border-line
                  px-2 py-1.5 text-xs"
              />
              <button
                type="button"
                disabled={busy === a.id}
                onClick={() => void decide(a.id, "approve")}
                className="min-h-[32px] rounded-full bg-sky-deep px-3 text-xs
                  font-bold text-white disabled:opacity-50"
              >
                通过（发邀请码）
              </button>
              <button
                type="button"
                disabled={busy === a.id}
                onClick={() => void decide(a.id, "reject")}
                className="min-h-[32px] rounded-full bg-coral px-3 text-xs
                  font-bold text-white disabled:opacity-50"
              >
                拒绝
              </button>
            </div>
          )}
        </div>
      ))}
    </section>
  );
}
