"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { InviteItem } from "@/lib/data";

/** 邀请管理（包子站 invite.php 同款）：我的邀请码 + 生成新邀请 */
export function InviteManager({
  empty,
  issueLabel,
  redeemLabel,
  redeemNote,
  replayedText,
  needClass,
}: {
  empty: string;
  issueLabel: string;
  redeemLabel: string;
  redeemNote: string;
  replayedText: string;
  needClass: string;
}) {
  const { dict } = useI18n();
  const [invites, setInvites] = useState<InviteItem[] | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  /** 魔力商店「邀请名额」现价（kind=invite）；未开放兑换时为 null，隐藏兑换入口 */
  const [price, setPrice] = useState<number | null>(null);

  async function refresh() {
    try {
      setInvites(await api.get<InviteItem[]>("/api/v1/invites"));
    } catch {
      setInvites([]);
    }
  }
  useEffect(() => {
    refresh();
    api
      .get<{ id: number; kind: string; price: number }[]>("/api/v1/shop/items")
      .then((items) => {
        const invite = items.find((i) => i.kind === "invite");
        setPrice(invite ? invite.price : null);
      })
      .catch(() => setPrice(null));
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

  async function redeem() {
    setBusy(true);
    setMsg(null);
    try {
      // 幂等键：每次点击生成新 UUID（与 /shop/buy 同口径，防网络重试双扣款）
      const r = await api.post<{ code: string; replayed?: boolean }>(
        "/api/v1/invites/redeem",
        { idempotency_key: crypto.randomUUID() },
      );
      setMsg(r.code ?? replayedText);
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
      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          disabled={busy}
          onClick={issue}
          className="min-h-[44px] rounded-full bg-sky-deep px-5 text-sm text-white disabled:opacity-50"
        >
          {issueLabel}
        </button>
        {price !== null && (
          <button
            type="button"
            disabled={busy}
            onClick={redeem}
            className="min-h-[44px] rounded-full border border-[var(--baozi-line)] px-5 text-sm text-sky-deep hover:border-[var(--baozi-orange)] hover:text-[var(--baozi-orange)] disabled:opacity-50"
          >
            {redeemLabel}（{price.toLocaleString()} {dict.common.spark}）
          </button>
        )}
      </div>
      {price !== null && <p className="text-xs text-sub">{redeemNote}</p>}
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
