"use client";

import { useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";
import { useIsCompact } from "@/lib/hooks/use-media";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import { toast } from "@/components/ui/toast";

interface HrRow {
  torrent_id: number;
  torrent_name: string;
  required_seconds: number;
  seeded_seconds: number;
  deadline: string;
  status: string;
}

/** 我的 H&R 表（/me/hr 口径：种子/要求/已做种/截止/状态 + 自助免罪） */
export function MyHrTable({ rows, locale }: { rows: HrRow[]; locale: string }) {
  const { dict, currency } = useI18n();
  const router = useRouter();
  const t = dict.myhr;
  const dl = locale === "zh-TW" ? "zh-TW" : locale === "en" ? "en-US" : "zh-CN";
  const [busyId, setBusyId] = useState<number | null>(null);
  const compact = useIsCompact();
  // A1 反馈协议：window.confirm → ConfirmDialog；结果行内 msg → toast
  const [askTid, setAskTid] = useState<number | null>(null);

  async function selfPardon(tid: number) {
    setBusyId(tid);
    try {
      await api.post("/api/v1/me/hr/pardon", { torrent_id: tid });
      toast.success(dict.myhr2.pardonOk);
      setAskTid(null);
      router.refresh();
    } catch (e) {
      toast.error(apiErrorMessage(dict, e), {
        requestId: e instanceof ApiError ? e.requestId : undefined,
      });
    } finally {
      setBusyId(null);
    }
  }

  return (
    <div className="flex flex-col gap-2">
      {/* M6.2 行卡片：compact 档表格降为行卡片（wide-table 横滚仅桌面保留） */}
      {compact && rows.length > 0 && (
        <div className="trow-list">
          {rows.map((r) => {
            const hours = (secs: number) => `${(secs / 3600).toFixed(1)} h`;
            return (
              <div className="hrcard" key={r.torrent_id}>
                <Link
                  href={`/torrent/${r.torrent_id}`}
                  className="hrcard__title"
                >
                  {r.torrent_name}
                </Link>
                <p className="hrcard__meta num">
                  <span>
                    {t.colRequired} {hours(r.required_seconds)}
                  </span>
                  <span>
                    {t.colSeeded} {hours(r.seeded_seconds)}
                  </span>
                  <span>
                    {t.colDeadline}{" "}
                    {new Date(r.deadline).toLocaleString(dl)}
                  </span>
                </p>
                <p className="hrcard__ft">
                  {r.status === "violated" ? (
                    <span className="fun-status fun-status--banned">
                      {t.stFlagged}
                    </span>
                  ) : r.status === "satisfied" ? (
                    <span className="fun-status fun-status--normal">
                      {t.stOk}
                    </span>
                  ) : r.status === "pardoned" ? (
                    <span className="fun-status fun-status--normal">
                      {dict.myhr2.stPardoned}
                    </span>
                  ) : (
                    <span className="fun-status fun-status--dull">
                      {t.stPending}
                    </span>
                  )}
                  {r.status === "violated" && (
                    <button
                      type="button"
                      disabled={busyId === r.torrent_id}
                      onClick={() => setAskTid(r.torrent_id)}
                      className="hrcard__btn"
                      title={dict.myhr2.pardonNote.replace(
                        "{magic}",
                        currency,
                      )}
                    >
                      {dict.myhr2.pardonBtn.replace("{magic}", currency)}
                    </button>
                  )}
                </p>
              </div>
            );
          })}
        </div>
      )}
      {!compact && (
        <div className="brand-wide-table-scroll">
          <table className="nexus-table">
            <tbody>
              <tr>
                <td className="colhead">{t.colTorrent}</td>
                <td className="colhead">{t.colRequired}</td>
                <td className="colhead">{t.colSeeded}</td>
                <td className="colhead">{t.colDeadline}</td>
                <td className="colhead">{t.colStatus}</td>
                <td className="colhead" />
              </tr>
              {rows.map((r) => {
                const hours = (secs: number) =>
                  `${(secs / 3600).toFixed(1)} h`;
                return (
                  <tr key={r.torrent_id}>
                    <td>
                      <Link
                        href={`/torrent/${r.torrent_id}`}
                        className="font-bold"
                      >
                        {r.torrent_name}
                      </Link>
                    </td>
                    <td className="num">{hours(r.required_seconds)}</td>
                    <td className="num">{hours(r.seeded_seconds)}</td>
                    <td className="nowrap">
                      {new Date(r.deadline).toLocaleString(dl)}
                    </td>
                    <td>
                      {r.status === "violated" ? (
                        <span className="fun-status fun-status--banned">
                          {t.stFlagged}
                        </span>
                      ) : r.status === "satisfied" ? (
                        <span className="fun-status fun-status--normal">
                          {t.stOk}
                        </span>
                      ) : r.status === "pardoned" ? (
                        <span className="fun-status fun-status--normal">
                          {dict.myhr2.stPardoned}
                        </span>
                      ) : (
                        <span className="fun-status fun-status--dull">
                          {t.stPending}
                        </span>
                      )}
                    </td>
                    <td>
                      {r.status === "violated" && (
                        <button
                          type="button"
                          disabled={busyId === r.torrent_id}
                          onClick={() => setAskTid(r.torrent_id)}
                          className="min-h-[28px] rounded-full border border-line px-3 text-[11px] font-bold text-sky-deep disabled:opacity-50"
                          title={dict.myhr2.pardonNote.replace(
                            "{magic}",
                            currency,
                          )}
                        >
                          {dict.myhr2.pardonBtn.replace(
                            "{magic}",
                            currency,
                          )}
                        </button>
                      )}
                    </td>
                  </tr>
                );
              })}
              {rows.length === 0 && (
                <tr>
                  <td colSpan={6} className="py-8 text-center text-sub">
                    {t.empty}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}
      {/* 移动端空态：卡片列表只在有数据时渲染，空数据时给**可见**占位。
          旧版把空态塞进 sr-only 表格——视觉隐藏等于谁都看不见（UX），
          且 .nexus-table 的 width:100% 覆盖 sr-only 的 1px、绝对定位表格
          把 body 滚动区撑宽 15px（390px 实测横滚，M5.6 扫描 2026-10-15）。 */}
      {compact && rows.length === 0 && (
        <p className="py-8 text-center text-sub">{t.empty}</p>
      )}
      {/* 自助免罪确认（花费魔力，二次确认防误触） */}
      <ConfirmDialog
        open={askTid !== null}
        onClose={() => setAskTid(null)}
        onConfirm={() => askTid !== null && void selfPardon(askTid)}
        tone="danger"
        title={dict.myhr2.pardonBtn.replace("{magic}", currency)}
        message={dict.myhr2.pardonConfirm.replace("{magic}", currency)}
      />
    </div>
  );
}
