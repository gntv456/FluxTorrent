"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { InviteItem } from "@/lib/data";

/** 邀请管理（包子站 invite.php 同款）：我的邀请码 + 生成新邀请 */
export function InviteManager({
  empty,
  issueLabel,
  needClass,
}: {
  empty: string;
  issueLabel: string;
  needClass: string;
}) {
  const { dict } = useI18n();
  const [invites, setInvites] = useState<InviteItem[] | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function refresh() {
    try {
      setInvites(await api.get<InviteItem[]>("/api/v1/invites"));
    } catch {
      setInvites([]);
    }
  }
  useEffect(() => {
    refresh();
  }, []);

  async function issue() {
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.post<{ code: string }>("/api/v1/invites", {});
      setMsg(r.code);
      refresh();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function copy(code: string) {
    await navigator.clipboard.writeText(code);
    setMsg(code);
  }

  if (invites === null) return null;
  return (
    <div className="flex flex-col gap-3">
      <button
        type="button"
        disabled={busy}
        onClick={issue}
        className="self-start min-h-[44px] rounded-full bg-sky-deep px-5 text-sm text-white disabled:opacity-50"
      >
        {issueLabel}
      </button>
      {msg && (
        <p role="alert" className="text-sm text-sky-deep">
          {msg}
        </p>
      )}
      {invites.length === 0 ? (
        <p className="py-6 text-center text-sub">{empty}</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {invites.map((i) => (
            <li
              key={i.id}
              className="flex flex-wrap items-center justify-between gap-2 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]"
            >
              <button
                type="button"
                onClick={() => copy(i.code)}
                className="num font-mono text-sm text-sky-deep hover:underline"
                title={i.code}
              >
                {i.code.slice(0, 8)}••••••••
              </button>
              <span className="text-xs text-sub">
                {i.used_by ? `→ ${i.used_by}` : ""}
              </span>
              <span
                className={`sticker ${
                  i.status === 0 ? "bg-sun text-ink" : i.status === 1 ? "bg-mint/30 text-ink" : "bg-cloud text-sub"
                }`}
              >
                {i.status === 0
                  ? dict.invites.unused
                  : i.status === 1
                    ? dict.invites.used
                    : dict.invites.expired}
              </span>
            </li>
          ))}
        </ul>
      )}
      <p className="text-xs text-sub">{needClass}</p>
    </div>
  );
}
