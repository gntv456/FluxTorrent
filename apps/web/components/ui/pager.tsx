"use client";

import type { ReactNode } from "react";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { BTN_SM_GHOST } from "@/lib/ui-classes";

/** 分页条原语（S2 模板化 · 2026-10-16）
 *
 *  统一 admin-* 各面板里近乎逐字重复的翻页条：同一段
 *  「共 N 条 + 上一页 + 页码 + 下一页」flex 容器，彼此只在按钮类串、
 *  页码格式（`第 N 页` vs `N / M`）与「下一页禁用条件」上微异。
 *  调用方只表达「第几页 / 共几页 / 翻到哪」，容器排版与文案归原语
 *  （DS-PLAN-01 §10 页面模板）。
 *
 *  文案走 dict.common（prevPage / nextPage / totalItems / pageX），
 *  按钮样式复用 ui-classes 的 BTN_SM_GHOST —— 本组不新增 CSS。
 *  「下一页禁用」由调用方传入的 pages 判定；部分老实现用
 *  「本页行数 < perPage」推断，等价但更易错，这里改为显式传总页数。
 *
 *  用法：
 *    <Pager page={page} pages={pages} total={total} onStep={setPage} />
 *    <Pager layout="compact" posLabel={`${page} / ${pages}`} … />
 */
export function Pager({
  page,
  pages,
  total,
  onStep,
  layout = "split",
  posLabel,
}: {
  /** 当前页（1 起） */
  page: number;
  /** 总页数；page >= pages 时「下一页」禁用 */
  pages: number;
  /** 总条数（split 档左上角「共 N 条」）；不传则留空 */
  total?: number;
  onStep: (page: number) => void;
  /** split=左「共 N 条」右导航（后台表格默认）；compact=整体右对齐、不显总数 */
  layout?: "split" | "compact";
  /** 页码文案覆盖（默认 dict.common.pageX「第 N 页」）。各 admin 段若有
   *  自有口径（pageInfo「第 N 页 / 共 M 条」、`N / M`、`N / M（T）`）
   *  从自己的 dict 传入，不必为迁就原语改 i18n。 */
  posLabel?: ReactNode;
}) {
  const c = useI18n().dict.common;
  return (
    <div
      className={
        "ui-pager flex items-center text-xs text-sub " +
        (layout === "compact" ? "justify-end" : "justify-between")
      }
    >
      {layout === "split" && (
        <span>
          {total === undefined ? "" : fmt(c.totalItems, { n: total })}
        </span>
      )}
      <div className="flex items-center gap-2">
        <button
          type="button"
          disabled={page <= 1}
          onClick={() => onStep(page - 1)}
          className={BTN_SM_GHOST}
        >
          {c.prevPage}
        </button>
        <span>{posLabel ?? fmt(c.pageX, { n: page })}</span>
        <button
          type="button"
          disabled={page >= pages}
          onClick={() => onStep(page + 1)}
          className={BTN_SM_GHOST}
        >
          {c.nextPage}
        </button>
      </div>
    </div>
  );
}
