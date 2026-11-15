"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import {
  ConfirmPair,
  RedeemForm,
  StartButton,
} from "@/components/voucher-redeem-forms";

type VoucherKind =
  | "free"
  | "neutral"
  | "temp_invite"
  | "rename_card"
  | (string & {});

type Voucher = {
  id: number;
  kind: VoucherKind;
  source: string;
  granted_at: string;
  expires_at: string;
  used_torrent_id: number | null;
  used_at: string | null;
};

/** temp_invite 核销响应（0211 H2）：直接返回邀请码，无需绑种子 */
type InviteRedeem = {
  voucher_id: number;
  invite_id: number;
  code: string;
  expires_at: string;
};

/** rename_card 核销响应（me_rename）：改名成功返回新旧名 */
type RenameRedeem = {
  voucher_id: number;
  old_name: string;
  new_name: string;
};

const KIND_LABEL: Record<string, (d: VouchersDict) => string> = {
  free: (d) => d.freeKind,
  neutral: (d) => d.neutralKind,
  temp_invite: (d) => d.tempInviteKind,
  rename_card: (d) => d.renameCardKind,
};

const KIND_BADGE: Record<string, string> = {
  free: "FL",
  neutral: "NL",
  temp_invite: "IV",
  rename_card: "NM",
};

type VouchersDict = ReturnType<typeof useI18n>["dict"]["vouchers"];

/** 我的券面板（0073 免费券/中性券 + 0211 分券种口径）：
 *  free/neutral 绑种子；temp_invite 核销即得邀请码；rename_card
 *  在此输入新用户名核销（POST /me/rename，2026-11-15 起）。 */
export function VoucherPanel() {
  const { dict, locale } = useI18n();
  const [vouchers, setVouchers] = useState<Voucher[]>([]);
  const [using, setUsing] = useState<number | null>(null);
  const [torrentId, setTorrentId] = useState("");
  const [renameTo, setRenameTo] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const rows = await api.get<Voucher[]>("/api/v1/me/vouchers");
      setVouchers(rows);
    } catch (err) {
      setError(apiErrorMessage(dict, err));
    }
  }, [dict]);

  useEffect(() => {
    void load();
  }, [load]);

  const open = vouchers.filter(
    (v) => !v.used_at && new Date(v.expires_at) > new Date(),
  );
  const history = vouchers.filter((v) => !open.includes(v));
  const kindLabel = (v: Voucher) =>
    (KIND_LABEL[v.kind] ?? ((d: VouchersDict) => v.kind))(dict.vouchers);

  async function use(id: number) {
    const target = open.find((v) => v.id === id);
    if (!target) return;
    // temp_invite：无需种子 ID，核销即得邀请码
    if (target.kind === "temp_invite") {
      setError(null);
      try {
        const r = await api.post<InviteRedeem>(
          "/api/v1/me/vouchers/use",
          { voucher_id: id },
        );
        setMessage(
          dict.vouchers.inviteRedeemed
            .replace("{code}", r.code)
            .replace(
              "{date}",
              new Date(r.expires_at).toLocaleDateString(dateLocale(locale)),
            ),
        );
        setUsing(null);
        await load();
      } catch (err) {
        setError(apiErrorMessage(dict, err));
      }
      return;
    }
    // rename_card：输入新用户名核销（POST /me/rename 自动抵扣最早到期的一张）
    if (target.kind === "rename_card") {
      const name = renameTo.trim();
      if (name.length < 3 || name.length > 24) {
        setError(dict.vouchers.renameNameLength);
        return;
      }
      setError(null);
      try {
        const r = await api.post<RenameRedeem>("/api/v1/me/rename", {
          new_name: name,
        });
        setMessage(
          dict.vouchers.renameDone
            .replace("{old}", r.old_name)
            .replace("{new}", r.new_name),
        );
        setUsing(null);
        setRenameTo("");
        await load();
      } catch (err) {
        setError(apiErrorMessage(dict, err));
      }
      return;
    }
    const tid = Number(torrentId);
    if (!tid || tid <= 0) {
      setError(dict.vouchers.needTorrentId);
      return;
    }
    setError(null);
    try {
      await api.post("/api/v1/me/vouchers/use", {
        voucher_id: id,
        torrent_id: tid,
      });
      setMessage(dict.vouchers.used.replace("{id}", String(tid)));
      setUsing(null);
      setTorrentId("");
      await load();
    } catch (err) {
      setError(apiErrorMessage(dict, err));
    }
  }

  return (
    <section className="brand-panel">
      <div className="brand-panel__head">
        <h2>{dict.vouchers.title}</h2>
        <span className="text-xs text-sub">{dict.shop.packNote}</span>
      </div>
      <div className="flex flex-col gap-3 p-3">
        {message && (
          <p className="text-xs text-[var(--success)]">{message}</p>
        )}
        {error && <p className="text-xs text-[var(--danger-text)]">{error}</p>}
        {open.length === 0 && (
          <p className="text-sm text-sub">{dict.vouchers.empty}</p>
        )}
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {open.map((v) => (
            <div key={v.id} className="vticket">
              <span className="n">{KIND_BADGE[v.kind] ?? "??"}</span>
              <span className="l">
                {kindLabel(v)}
                <small>
                  {dict.vouchers.expires}{" "}
                  {new Date(v.expires_at).toLocaleDateString(
                    dateLocale(locale),
                  )}
                </small>
              </span>
              <div className="acts">
                {v.used_torrent_id ? (
                  <span className="text-xs text-sub">
                    {dict.vouchers.used.replace(
                      "{id}", String(v.used_torrent_id),
                    )}
                  </span>
                ) : v.kind === "rename_card" ? (
                  using === v.id ? (
                    <RedeemForm
                      placeholder={dict.vouchers.renamePrompt}
                      value={renameTo}
                      maxLength={24}
                      onChange={setRenameTo}
                      onConfirm={() => void use(v.id)}
                      onCancel={() => setUsing(null)}
                    />
                  ) : (
                    <StartButton
                      label={dict.vouchers.renameUse}
                      onStart={() => {
                        setUsing(v.id);
                        setMessage(null);
                        setError(null);
                      }}
                    />
                  )
                ) : using === v.id ? (
                  v.kind === "temp_invite" ? (
                    <div className="flex flex-col gap-2">
                      <p className="text-xs text-sub">
                        {dict.vouchers.inviteConfirm}
                      </p>
                      <ConfirmPair
                        onConfirm={() => void use(v.id)}
                        onCancel={() => setUsing(null)}
                      />
                    </div>
                  ) : (
                    <RedeemForm
                      placeholder={dict.vouchers.torrentIdPlaceholder}
                      value={torrentId}
                      inputMode="numeric"
                      onChange={setTorrentId}
                      onConfirm={() => void use(v.id)}
                      onCancel={() => setUsing(null)}
                    />
                  )
                ) : (
                  <StartButton
                    label={dict.vouchers.use}
                    onStart={() => {
                      setUsing(v.id);
                      setMessage(null);
                      setError(null);
                    }}
                  />
                )}
              </div>
            </div>
          ))}
        </div>
        {history.length > 0 && (
          <details className="rounded border border-line p-3">
            <summary className="cursor-pointer text-sm text-sub">
              {dict.vouchers.history}
            </summary>
            <ul className="mt-2 flex flex-col gap-1 text-xs text-sub">
            {history.slice(0, 30).map((v) => (
              <li key={v.id}>
                #{v.id} {kindLabel(v)}
                {v.used_torrent_id ? ` → #${v.used_torrent_id}` : ""}{" "}
                {v.used_at
                  ? `(${new Date(v.used_at).toLocaleDateString(
                      dateLocale(locale),
                    )})`
                  : ""}
              </li>
            ))}
          </ul>
        </details>
        )}
      </div>
    </section>
  );
}
