"use client";

/** 高级筛选面板的「收起」按钮。
 *
 * 2026-10-14 从 `torrents-search-box.tsx` 抽出：该文件是**有意的 server
 * component**（数据注入型，见其文件头），却内联了 `onClick` —— RSC 直接抛
 * 「Event handlers cannot be passed to Client Component props」，整页
 * /torrents 落进错误边界（`next build` 查不出，只有真开页面才炸）。
 * 目标面板本体仍在服务端渲染的 `<details id="tsb-adv">` 里，用 id 定位。
 */
export function AdvCollapseButton({ label }: { label: string }) {
  return (
    <button
      type="button"
      className="tsb-adv__collapse"
      onClick={() => {
        const adv = document.getElementById("tsb-adv");
        if (adv instanceof HTMLDetailsElement) adv.open = false;
      }}
    >
      {label}
    </button>
  );
}
