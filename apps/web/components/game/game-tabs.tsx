"use client";

/**
 * 游戏页顶部的切换标签条（样图手机版结构：每个游戏是「主玩法 + 子页」
 * 的 tab 形态，不是把所有区块平铺在一页）。
 * 受控组件：tab 状态由页面持有（切 tab 时常要停动画/懒加载）。
 */
export function GameTabs({
  tabs,
  active,
  onChange,
}: {
  tabs: { key: string; label: string; icon?: string }[];
  active: string;
  onChange: (key: string) => void;
}) {
  return (
    <nav className="gtabs" aria-label="tab">
      {tabs.map((t) => (
        <button
          key={t.key}
          type="button"
          className={`gtab${active === t.key ? " on" : ""}`}
          aria-current={active === t.key ? "page" : undefined}
          onClick={() => onChange(t.key)}
        >
          {t.icon && (
            <span aria-hidden className="gtab-ic">
              {t.icon}
            </span>
          )}
          {t.label}
        </button>
      ))}
    </nav>
  );
}
