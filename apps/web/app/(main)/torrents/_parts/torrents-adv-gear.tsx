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
        if (adv instanceof HTMLDetailsElement) adv.open = !adv.open;
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
