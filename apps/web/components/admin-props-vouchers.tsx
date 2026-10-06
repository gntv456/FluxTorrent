"use client";

/**
 * 券的后台面板（发放面二轮）。
 *
 * 为什么单独一块：`POST /admin/user-vouchers/void` 早就存在，但界面上一直**零调用**——
 * 因为后台根本没有读口（只有用户侧 `/me/vouchers`）。看不到某人名下有几张券、
 * 哪张已核销、哪张是管理发放的，就没人敢决定「作废几张」。
 * 现在读与管在同一处，且作废只作用于 `source=admin` 的未核销券：
 * 用户花魔力买的券不在本端点范围（那是退款范畴）。
 */

import { useCallback, useEffect, useState } from "react";

import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

interface VoucherRow {
  id: number;
  user_id: number;
  username: string;
  kind: string;
  /** admin = 管理发放；shop = 用户自购（不可被本面板作废） */
  source: string;
  granted_at: string;
  expires_at: string;
  used_at: string | null;
  used_torrent_id: number | null;
  expired: boolean;
}

interface Props {
  /** 与背包面板共用同一个 UID 输入：一次圈定，两处资产一起看 */
  uid: string;
  setUid: (v: string) => void;
  flash: (m: string) => void;
}

const UID_CLS =
  "min-h-[32px] w-40 rounded-full border border-line px-3 text-xs";
const N_CLS =
  "min-h-[32px] w-16 rounded-full border border-line px-2 text-xs";

export function VoucherPanel({ uid, setUid, flash }: Props) {
  const { dict, locale } = useI18n();
  const at = dict.adminProps;
  const [rows, setRows] = useState<VoucherRow[]>([]);
  const [busy, setBusy] = useState(false);
  const [count, setCount] = useState("1");

  const load = useCallback(async () => {
    const u = uid.trim();
    if (!u) {
      setRows([]);
      return;
    }
    setBusy(true);
    try {
      const r = await api.get<VoucherRow[]>(
        `/api/v1/admin/user-vouchers?uid=${encodeURIComponent(u)}`,
      );
      setRows(r ?? []);
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.vLoadFail);
    } finally {
      setBusy(false);
    }
  }, [uid, at.vLoadFail, flash]);

  useEffect(() => {
    void load();
  }, [load]);

  /** 可作废 = 管理发放且未核销。自购券即使未用也不在内。 */
  const voidable = rows.filter(
    (r) => r.source === "admin" && r.used_at === null,
  );
  const stateOf = (r: VoucherRow) =>
    r.used_at ? at.vUsed : r.expired ? at.vExpired : at.vUsable;
  const fmtTime = (s: string) =>
    new Date(s).toLocaleString(dateLocale(locale));

  return (
    <section className="baozi-panel p-4">
      <div className="mb-2 flex flex-wrap items-end gap-2">
        <h3 className="text-sm font-bold">{at.vTitle}</h3>
        <input
          value={uid}
          onChange={(e) => setUid(e.target.value)}
          placeholder={at.qUid}
          className={UID_CLS}
        />
        <button
          className={UID_CLS}
          disabled={busy}
          onClick={() => void load()}
        >
          {at.vRefresh}
        </button>
        <span className="ml-auto text-xs text-sub">
          {at.vVoidable.replace("{n}", String(voidable.length))}
        </span>
        <input
          value={count}
          onChange={(e) => setCount(e.target.value)}
          className={N_CLS}
          inputMode="numeric"
          title={at.vVoidNHint}
        />
        <button
          className="min-h-[32px] rounded-full border border-red-300
            px-3 text-xs font-bold text-red-600 disabled:opacity-40"
          disabled={busy || voidable.length === 0}
          onClick={async () => {
            const n = Number(count);
            if (!Number.isInteger(n) || n < 1 || n > 50) {
              flash(at.vVoidBad);
              return;
            }
            if (
              !window.confirm(
                at.vVoidConfirm
                  .replace("{n}", String(n))
                  .replace("{u}", uid.trim()),
              )
            ) {
              return;
            }
            setBusy(true);
            try {
              const r = await api.post<{ voided: number }>(
                "/api/v1/admin/user-vouchers/void",
                { user_id: Number(uid.trim()), limit: n },
              );
              flash(at.vVoidDone.replace("{n}", String(r.voided)));
              await load();
            } catch (e) {
              flash(e instanceof ApiError ? e.message : at.vVoidFail);
            } finally {
              setBusy(false);
            }
          }}
        >
          {at.vVoid}
        </button>
      </div>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">{at.vThKind}</td>
            <td className="colhead">{at.vThSource}</td>
            <td className="colhead">{at.vThGranted}</td>
            <td className="colhead">{at.vThExpire}</td>
            <td className="colhead">{at.vThState}</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td>{r.kind === "neutral" ? at.vKindNeutral : at.vKindFree}</td>
              <td className="text-sub">{r.source}</td>
              <td className="text-sub">{fmtTime(r.granted_at)}</td>
              <td
                className={
                  r.expired && !r.used_at ? "text-red-600" : "text-sub"
                }
              >
                {fmtTime(r.expires_at)}
              </td>
              <td>{stateOf(r)}</td>
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td colSpan={5} className="py-4 text-center text-sub">
                {uid.trim() ? at.vEmpty : at.vNeedUid}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}
