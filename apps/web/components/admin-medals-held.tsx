"use client";

/**
 * 勋章管理·持有浏览子面板（从 components/admin-medals.tsx 按域拆出）：
 * 全站持有记录（按 UID 过滤）与勋章回收（授予入口在用户详情页）。
 */

import { api, ApiError } from "@/lib/api-client";
import type { UserMedalRow } from "./admin-medals-shared";
import { useI18n } from "@/i18n/client";

interface HeldPanelProps {
  held: UserMedalRow[];
  heldUid: string;
  setHeldUid: (v: string) => void;
  busy: boolean;
  flash: (m: string) => void;
  load: () => Promise<void>;
}

/** UID 过滤输入框样式 */
const UID_FILTER_CLS =
  "min-h-[32px] w-40 rounded-full border border-line px-3 text-xs";

export function MedalHeldPanel({
  held,
  heldUid,
  setHeldUid,
  busy,
  flash,
  load,
}: HeldPanelProps) {
  const at = useI18n().dict.adminMedals;
  return (
    <section className="baozi-panel p-4">
      <div className="mb-2 flex items-end gap-2">
        <h3 className="text-sm font-bold">{at.heldTitle}</h3>
        <input
          value={heldUid}
          onChange={(e) => setHeldUid(e.target.value)}
          placeholder={at.qUid}
          className={UID_FILTER_CLS}
        />
      </div>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">{at.thUser}</td>
            <td className="colhead">{at.thMedal}</td>
            <td className="colhead">{at.thSource}</td>
            <td className="colhead">{at.thWearing}</td>
            <td className="colhead">{at.thExpiry}</td>
            <td className="colhead text-right">{at.thAction}</td>
          </tr>
        </thead>
        <tbody>
          {held.map((h) => (
            <tr key={`${h.user_id}-${h.medal_id}`}>
              <td>
                <a
                  href={`/admin/users/${h.user_id}`}
                  className="font-bold text-link"
                >
                  {h.username}
                </a>
              </td>
              <td>{h.medal_name}</td>
              <td>{h.source}</td>
              <td>{h.wearing ? at.wearing : "—"}</td>
              <td>
                {h.expires_at
                  ? `${h.expires_at.slice(0, 10)}${
                      h.expired ? ` · ${at.expiredMark}` : ""
                    }`
                  : at.forever}
              </td>
              <td className="text-right">
                <div className="flex justify-end gap-1">
                  <button
                    className="cmgmt-act"
                    disabled={busy}
                    title={at.redateTitle}
                    onClick={async () => {
                      const raw = window.prompt(at.redatePrompt);
                      if (raw === null) return;
                      const trimmed = raw.trim();
                      const days =
                        trimmed === "" ? null : Number(trimmed);
                      if (days !== null && !Number.isInteger(days)) {
                        flash(at.redateBad);
                        return;
                      }
                      try {
                        await api.post(
                          `/api/v1/admin/users/${h.user_id}` +
                            `/medal/${h.medal_id}/redate`,
                          { days },
                        );
                        flash(at.redated);
                        await load();
                      } catch (e) {
                        flash(
                          e instanceof ApiError
                            ? e.message
                            : at.redateFail,
                        );
                      }
                    }}
                  >
                    {at.redate}
                  </button>
                  <button
                    className="cmgmt-act cmgmt-act--danger"
                    disabled={busy}
                    onClick={async () => {
                      try {
                        await api.post("/api/v1/admin/user-medals/delete", {
                          user_id: h.user_id,
                          medal_id: h.medal_id,
                        });
                        flash(at.revoked);
                        await load();
                      } catch (e) {
                        flash(
                          e instanceof ApiError ? e.message : at.revokeFail,
                        );
                      }
                    }}
                  >
                    {at.revoke}
                  </button>
                </div>
              </td>
            </tr>
          ))}
          {held.length === 0 && (
            <tr>
              <td colSpan={6} className="py-4 text-center text-sub">
                {at.heldEmpty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}
