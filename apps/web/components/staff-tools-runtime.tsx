"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import {
  PAGE_BTN_CLS,
  type RuntimeLogPage,
} from "./staff-tools-sys-shared";

/** 运行日志面板（0218 G6）：api/worker 的 WARN+ tracing 事件（runtime_logs）。
 *  本页此前读 audit_log —— 那是「谁做了什么」的操作审计（视图在 ?tool=audit），
 *  站点报错时站长在这里查不到任何运行痕迹。现两级 tracing 层落库，见
 *  apps/api/src/runtime_log.rs 与 apps/worker/src/runtime_log.rs。 */

/** 级别筛选档位（空串 = 全部） */
const LEVELS: string[] = ["", "WARN", "ERROR"];

/** 级别配色：ERROR 红、WARN 黄、其余灰 */
function levelCls(level: string): string {
  if (level === "ERROR") return "font-bold text-danger";
  if (level === "WARN") return "font-bold text-warning";
  return "text-sub";
}

/** 档位按钮：选中态用品牌底色，未选中复用分页描边口径 */
function segCls(on: boolean): string {
  return on
    ? "min-h-[36px] rounded-full bg-sky-soft px-4 text-xs font-bold " +
        "text-ink"
    : PAGE_BTN_CLS;
}

export function RuntimeLogPanel() {
  const { dict, locale } = useI18n();
  const t = dict.stafftools;
  const [page, setPage] = useState(1);
  const [q, setQ] = useState("");
  const [level, setLevel] = useState("");
  const [instance, setInstance] = useState("");
  const [data, setData] = useState<RuntimeLogPage | null>(null);

  useEffect(() => {
    const query =
      `page=${page}&q=${encodeURIComponent(q)}` +
      `&level=${encodeURIComponent(level)}` +
      `&instance=${encodeURIComponent(instance)}`;
    api
      .get<RuntimeLogPage>(`/api/v1/admin/syslog?${query}`)
      .then(setData)
      .catch(() => setData(null));
  }, [page, q, level, instance]);

  const countOf = (lv: string) =>
    (data?.counts_24h ?? []).find(([k]) => k === lv)?.[1] ?? 0;

  // 多实例下拉（0224 G30）：后端只列近 7 天出现过的实例；单实例部署列表为
  // 空（instance=''）→ 整块筛选器隐藏，不占位
  const hasInstances = (data?.instances ?? []).length > 0;

  return (
    <>
      <section className="baozi-panel flex flex-col gap-3 p-4">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs text-sub">{t.rlLevel}</span>
          {LEVELS.map((lv) => (
            <button
              key={lv || "all"}
              type="button"
              className={segCls(level === lv)}
              onClick={() => {
                setLevel(lv);
                setPage(1);
              }}
            >
              {lv || t.rlAll}
              {lv !== "" && (
                <span className="num ml-1">{countOf(lv)}</span>
              )}
            </button>
          ))}
          {hasInstances && (
            <span className="ml-2 flex items-center gap-1">
              <span className="text-xs text-sub">{t.rlInstance}</span>
              <select
                className={
                  "min-h-[36px] rounded-full border border-line " +
                  "bg-surface px-3 text-xs"
                }
                value={instance}
                onChange={(e) => {
                  setInstance(e.target.value);
                  setPage(1);
                }}
              >
                <option value="">{t.rlAll}</option>
                {(data?.instances ?? []).map(([name]) => (
                  <option key={name} value={name}>
                    {name}
                  </option>
                ))}
              </select>
            </span>
          )}
        </div>
        <div className="cmgmt-form">
          <label>
            {t.rlSearch}
            <input
              value={q}
              onChange={(e) => {
                setQ(e.target.value);
                setPage(1);
              }}
              placeholder="panic / timeout / announce"
            />
          </label>
        </div>
        <p className="text-xs text-sub">{t.rlHint}</p>
      </section>

      <div className="baozi-wide-table-scroll">
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead w-40">{t.rlTime}</td>
              <td className="colhead w-20">{t.rlLevel}</td>
              <td className="colhead w-24">{t.rlSrc}</td>
              {hasInstances && (
                <td className="colhead w-24">{t.rlInstance}</td>
              )}
              <td className="colhead w-48">{t.rlTarget}</td>
              <td className="colhead">{t.rlMsg}</td>
            </tr>
            {data?.items.map((r) => (
              <tr key={r.id}>
                <td className="text-xs text-sub">
                  {new Date(r.ts).toLocaleString(dateLocale(locale))}
                </td>
                <td className={`text-xs ${levelCls(r.level)}`}>
                  {r.level}
                </td>
                <td className="font-mono text-xs">{r.source}</td>
                {hasInstances && (
                  <td className="font-mono text-xs">{r.instance || "—"}</td>
                )}
                <td className="font-mono text-xs break-all">
                  {r.target || "—"}
                </td>
                <td className="text-xs break-all">
                  {r.message}
                  {r.repeat > 1 && (
                    <span className="num ml-1 text-sub">
                      ×{r.repeat}
                    </span>
                  )}
                </td>
              </tr>
            ))}
            {(!data || data.items.length === 0) && (
              <tr>
                <td
                  colSpan={hasInstances ? 6 : 5}
                  className="py-6 text-center text-sub"
                >
                  {t.rlEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      {data && data.pages > 1 && (
        <div className="flex items-center justify-between">
          <button
            className={PAGE_BTN_CLS}
            disabled={page <= 1}
            onClick={() => setPage((p) => p - 1)}
          >
            {dict.common.prevPage}
          </button>
          <span className="text-xs text-sub">
            {data.page} / {data.pages}（{data.total}）
          </span>
          <button
            className={PAGE_BTN_CLS}
            disabled={page >= data.pages}
            onClick={() => setPage((p) => p + 1)}
          >
            {dict.common.nextPage}
          </button>
        </div>
      )}
    </>
  );
}
