"use client";

/** 高级搜索齿轮按钮（「给我搜」右侧）：点击开合同表单内的 <details#tsb-adv>。
 *  极小 client 组件——搜索盒本体保持 server component（0147 拆出）。 */

export function AdvGear({
  label,
  badge = 0,
}: {
  label: string;
  badge?: number;
}) {
  return (
    <button
      type="button"
      className="tsb-adv-btn"
      title={label}
      aria-label={label}
      aria-controls="tsb-adv"
      onClick={() => {
        const adv = document.getElementById("tsb-adv");
        if (!(adv instanceof HTMLDetailsElement)) return;
        // 懒面板（2026-11-27）：占位壳（data-adv-lazy）首次展开 → adv=1
        // 整页取内容（React onToggle 对 gear 的程序化 open 变更不保证
        // 触发，实测漏发——齿轮自己承担展开导航，不依赖事件）。
        if (!adv.open && adv.dataset.advLazy === "1") {
          const u = new URL(window.location.href);
          u.searchParams.set("adv", "1");
          window.location.assign(u.toString());
          return;
        }
        adv.open = !adv.open;
      }}
    >
      <span className="tsb-adv-btn__icon" aria-hidden="true">
        ⚙
      </span>
      {badge > 0 && (
        <span className="tsb-adv-btn__badge num">{badge}</span>
      )}
    </button>
  );
}
