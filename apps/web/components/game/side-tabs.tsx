"use client";

import { useState } from "react";
import { useI18n } from "@/i18n/client";
import { PANEL_LG } from "@/lib/ui-classes";

/** 侧栏分区标签（布局优化 2026-11-23）：游戏专注页的侧栏此前把
 *  奖池 / 图鉴 / 规则 / 战绩等板块竖着全堆在 280px 窄栏里（fishing
 *  侧栏实测 851px，比视口还高，整页被撑到 2 屏）。收成分区标签：
 *  一次只显示一块，桌面端侧栏钉到舞台高度、内部滚动，不再撑页。
 *
 *  - ≥lg：横向小标签条 + 单面板；侧栏高度由 GameShell 的 grid 对齐
 *    舞台列（align-self: stretch + min-height: 0），面板内部滚动
 *  - <lg（窄屏无侧栏，side 落到舞台下方）：保持平铺全显示——
 *    窄屏本来就是上下滚动，藏进标签反而多点一次
 *
 *  受控/非受控两用：不传 active 时内部自持，默认显示第一块。 */
export interface SideTabItem {
  key: string;
  label: string;
  content: React.ReactNode;
}

export function SideTabs({
  items,
  active: activeProp,
  onActiveChange,
}: {
  items: SideTabItem[];
  active?: string;
  onActiveChange?: (k: string) => void;
}) {
  const [inner, setInner] = useState(items[0]?.key ?? "");
  const { dict } = useI18n();
  const active = activeProp ?? inner;
  const cur = items.find((i) => i.key === active) ?? items[0];
  const pick = (k: string) => {
    setInner(k);
    onActiveChange?.(k);
  };
  return (
    <div className="gs-side-tabs">
      {/* 窄屏平铺：标签条隐藏，所有面板按序展开 */}
      <div className="gs-side-stack">
        {items.map((i) => (
          <section key={i.key} className={PANEL_LG}>
            {i.content}
          </section>
        ))}
      </div>
      {/* 桌面端：标签 + 单面板 */}
      <div className="gs-side-tabbed">
        <div
          role="tablist"
          aria-label={dict.review.ariaTabList}
          className="gs-side-tabbar"
        >
          {items.map((i) => (
            <button
              key={i.key}
              type="button"
              role="tab"
              aria-selected={cur?.key === i.key}
              onClick={() => pick(i.key)}
              className={`gs-side-tab${cur?.key === i.key ? " on" : ""}`}
            >
              {i.label}
            </button>
          ))}
        </div>
        <div
          role="tabpanel"
          className={`gs-side-panel ${PANEL_LG}`}
        >
          {cur?.content}
        </div>
      </div>
    </div>
  );
}
