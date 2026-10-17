"use client";

import Link from "next/link";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import { Badge } from "@/components/ui/badge";

/** 我的申诉记录（从 app/appeals/page.tsx 按域拆出，300 行门禁）。
 *  四列对齐样图：提交时间 / 申诉类型 / 状态 / 处理结果。 */

export interface AppealRow {
  id: number;
  ref_id: number | null;
  kind: string;
  body: string;
  status: string;
  result_note: string | null;
  created_at: string;
}

export function AppealRows({
  rows,
  kinds,
}: {
  /** null = 还没取到/取失败；[] = 确实没有记录 */
  rows: AppealRow[] | null;
  /** [码, 人话] 对照表，与提交表单共用同一份 */
  kinds: [string, string][];
}) {
  const { dict, locale } = useI18n();
  const t = dict.appeals;
  return (
    <section className="baozi-panel overflow-hidden">
      <div className="baozi-panel__head">
        <h2 className="font-display text-lg">{t.myRecords}</h2>
        <span className="text-xs text-sub">{t.sla}</span>
      </div>
      {rows === null && (
        <p className="py-2 px-4 text-sm text-sub">{t.loading}</p>
      )}
      {rows?.length === 0 && (
        <div className="px-4 py-3">
          <p className="text-sm text-sub">{t.empty}</p>
          <Link
            href="/ban-log"
            className="text-xs text-[var(--sky)] underline"
          >
            {t.banlogLink}
          </Link>
        </div>
      )}
      {rows && rows.length > 0 && (
        <div className="overflow-x-auto">
          {/* 样图这四列；申诉正文跟在类型下面一行，太长截断、title 给全文 */}
          <table className="nexus-table">
            <tbody>
              <tr>
                <td className="colhead w-36">{t.colTime}</td>
                <td className="colhead">{t.colKind}</td>
                <td className="colhead w-24">{t.colStatus}</td>
                <td className="colhead w-[30%]">{t.colResult}</td>
              </tr>
              {rows.map((r) => (
                <tr key={r.id}>
                  <td className="text-xs text-sub">
                    {new Date(r.created_at).toLocaleString(
                      dateLocale(locale),
                    )}
                  </td>
                  <td className="max-w-[320px]">
                    <span className="text-sm font-bold">
                      {kinds.find(([k]) => k === r.kind)?.[1] ?? r.kind}
                    </span>
                    <span
                      className="block truncate text-xs text-sub"
                      title={r.body}
                    >
                      {r.body}
                    </span>
                  </td>
                  <td>
                    {/* 三态而非「已处理」一把抓：受理与驳回对当事人是
                        两件事，合并后驳回的申诉看着像还在办 */}
                    <Badge
                      tone={
                        r.status === "open"
                          ? "warn"
                          : r.status === "accepted"
                            ? "success"
                            : "danger"
                      }
                      variant={r.status === "open" ? "soft" : "solid"}
                    >
                      {r.status === "open"
                        ? t.statusOpen
                        : r.status === "accepted"
                          ? t.statusAccepted
                          : t.statusRejected}
                    </Badge>
                  </td>
                  <td className="text-sm text-sub">
                    {r.result_note || t.resultPending}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
