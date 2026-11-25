"use client";

/** 固定抽签运营面板·领取审核半场（0520 拆件守 300 行门禁）。
 *  主面板（活动/创建/开奖/履约）在 admin-fixed-lottery.tsx。 */

import { BTN_SM_GHOST, BTN_SM_SKY, PANEL_MD } from "@/lib/ui-classes";
import { SkeletonBlock } from "@/components/ui/skeleton";
import type { useI18n } from "@/i18n/client";
import { api } from "@/lib/api-client";

type ClaimRow = {
  winnerId: number;
  username: string;
  prizeName: string;
  submission: Record<string, unknown>;
};

/** props.act 由主面板传入（同一错误/成功提示通道）。 */
export function AdminLotteryClaims({
  t,
  claims,
  act,
}: {
  t: ReturnType<typeof useI18n>["dict"]["adminLottery"];
  claims: ClaimRow[] | null;
  act: (fn: () => Promise<unknown>, ok: string) => Promise<void>;
}) {
  return (
      <section className={PANEL_MD}>
        <h2 className="mb-2 font-bold">{t.claims}</h2>
        {claims === null ? (
          <SkeletonBlock height={60} />
        ) : claims.length === 0 ? (
          <p className="text-sm opacity-60">{t.empty}</p>
        ) : (
          <table className="w-full text-sm">
            <tbody>
              {claims.map((c) => (
                <tr key={c.winnerId} className="border-t border-line/40">
                  <td className="py-1.5 pr-2">{c.username}</td>
                  <td className="py-1.5 pr-2">{c.prizeName}</td>
                  <td className="py-1.5 pr-2 opacity-60">
                    {JSON.stringify(c.submission)}
                  </td>
                  <td className="py-1.5">
                    <div className="flex gap-1">
                      <button className={BTN_SM_SKY}
                        onClick={() => void act(
                          () => api.post(
                            `/api/v1/admin/lottery/claims/${c.winnerId}/review`,
                            { approve: true }
                          ),
                          "ok"
                        )}>
                        {t.approve}
                      </button>
                      <button className={BTN_SM_GHOST}
                        onClick={() => {
                          const reason = window.prompt(t.rejectReason);
                          if (reason?.trim()) {
                            void act(
                              () => api.post(
                                `/api/v1/admin/lottery/claims/` +
                                  `${c.winnerId}/review`,
                                { approve: false, reason }
                              ),
                              "ok"
                            );
                          }
                        }}>
                        {t.reject}
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
  );
}
