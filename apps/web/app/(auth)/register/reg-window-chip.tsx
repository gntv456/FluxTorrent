"use client";

import { fmt } from "@/i18n/config";

/** P-684 开放注册轮的窗口条（从 register/page.tsx 按域拆出，300 行门禁）。
 *  两种形态：
 *   - 生效中（open 且未 closed）：剩余额度 + 截止时间；
 *   - 预约未开始（not_started）：开放预告（此时仍按当前模式注册，
 *     邀请码照常可用——后端窗口闸会在未开始时拒免邀请码提交，
 *     前端 P-689 同口径按邀请制渲染主表单）。 */

export function RegWindowChip({
  soon,
  win,
  labels,
}: {
  soon: boolean;
  win: {
    label?: string;
    slots_left?: number;
    slots_total?: number | null;
    close_at?: string | null;
    open_at?: string | null;
    closed?: boolean;
  } | undefined;
  labels: {
    windowSoon: string;
    windowSlots: string;
    windowUntil: string;
    windowNoLimit: string;
  }; // 传 dict.register 命名空间（其为超集，Pick 结构字段逐字对齐）
}) {
  if (soon) {
    return (
      <div className="bz-regwindow-chip" data-testid="reg-window-soon">
        {fmt(labels.windowSoon, {
          label: win?.label ?? "",
          t: win?.open_at ? new Date(win.open_at).toLocaleString() : "",
        })}
      </div>
    );
  }
  if (!win || win.closed) return null;
  return (
    <div className="bz-regwindow-chip" data-testid="reg-window">
      {win.slots_left !== undefined && win.slots_left !== null ? (
        <>
          {fmt(labels.windowSlots, {
            left: win.slots_left,
            total: win.slots_total ?? win.slots_left,
          })}
          {win.close_at
            ? ` · ${fmt(labels.windowUntil, {
                t: new Date(win.close_at).toLocaleString(),
              })}`
            : ""}
        </>
      ) : (
        labels.windowNoLimit
      )}
    </div>
  );
}
