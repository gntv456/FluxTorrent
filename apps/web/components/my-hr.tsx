"use client";

import { useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { hrRemain } from "@/lib/hr-remain";
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
  /** 下载完成时刻（/me/hr 新口径带的列；历史数据可能为 null） */
  downloaded_at?: string | null;
}

/** 剩余期限文案：`hrRemain` 算天数，这里只管插进 dict 模板。
 *  deadline 解析不出来 → 「—」，不猜。 */
function remainLabel(
  t: { daysLeft: string; overdue: string },
  deadline: string,
  now: number,
): string {
  const r = hrRemain(deadline, now);
  if (r === null) return "—";
  return fmt(r.overdue ? t.overdue : t.daysLeft, { n: r.days });
}

/** 我的 H&R 表（/me/hr 口径：种子/要求/已做种/截止/状态 + 自助免罪）。
 *  分页与状态筛选由 RSC 用 URL 驱动，本页只渲染当页 rows；`status`
 *  用来把空态分成「暂无记录」与「这一类没有记录」两句话。 */
export function MyHrTable({
  rows,
  locale,
  status,
  now,
}: {
  rows: HrRow[];
  locale: string;
  status: string;
  now: number;
}) {
  const { dict, currency } = useI18n();
  const router = useRouter();
  const t = dict.myhr;
  const dl = locale === "zh-TW" ? "zh-TW" : locale === "en" ? "en-US" : "zh-CN";
  const [busyId, setBusyId] = useState<number | null>(null);
  const compact = useIsCompact();
  // A1 反馈协议：window.confirm → ConfirmDialog；结果行内 msg → toast
  const [askTid, setAskTid] = useState<number | null>(null);
  // 空态两句话：没筛选时「暂无记录」，筛了却零行是「这一类没有记录」
  const emptyText = status ? t.emptyFiltered : t.empty;
  const hours = (secs: number) => `${(secs / 3600).toFixed(1)} h`;

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
                  <span>
                    {t.colDownloaded}{" "}
                    {r.downloaded_at
                      ? new Date(r.downloaded_at).toLocaleDateString(dl)
                      : "—"}
                  </span>
                  <span>
                    {t.colRemaining} {remainLabel(t, r.deadline, now)}
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
                  {/* 申诉走 /appeals（POST /appeals 收 kind='hr'）：那一页
                      读不到 query，所以只递入口、不带 ref_id。 */}
                  <Link href="/appeals" className="hrcard__btn">
                    {t.appeal}
                  </Link>
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
                <td className="colhead">{t.colDownloaded}</td>
                <td className="colhead">{t.colRemaining}</td>
                <td className="colhead">{t.colStatus}</td>
                <td className="colhead" />
              </tr>
              {rows.map((r) => {
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
                    <td className="nowrap num text-xs text-sub">
                      {r.downloaded_at
                        ? new Date(r.downloaded_at).toLocaleDateString(dl)
                        : "—"}
                    </td>
                    <td className="nowrap num text-xs">
                      {remainLabel(t, r.deadline, now)}
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
                          className="min-h-[28px] rounded-full border border-line px-3 text-[11px] font-bold text-[var(--sky-deep)] disabled:opacity-50"
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
                      <Link
                        href="/appeals"
                        className="ml-2 text-xs font-bold"
                      >
                        {t.appeal}
                      </Link>
                    </td>
                  </tr>
                );
              })}
              {rows.length === 0 && (
                <tr>
                  <td colSpan={8} className="py-8 text-center text-sub">
                    {emptyText}
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
        <p className="py-8 text-center text-sub">{emptyText}</p>
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
