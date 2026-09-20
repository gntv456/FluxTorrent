/**
 * 用户公开主页展示小件（从 app/(main)/users/[id]/page.tsx 按域拆出）：
 * Card 卡片（标题 + 内容块，full 横跨整行，slot 首屏定席）、
 * Row 资料行（标签定宽 + 虚线分隔）。无 hooks：server component。
 */

import type { ReactNode } from "react";

/** 个人中心首屏的网格定席（见 globals.css .up-card--slot-*）：
 *  tl = 左列第一行，bl = 左列第二行，r2 = 右列横跨两行 */
const SLOT_CLASS = {
  tl: "up-card--slot-tl",
  bl: "up-card--slot-bl",
  r2: "up-card--slot-r2",
} as const;

/** 卡片：标题（图标 + 文案）+ 内容块；full 横跨整行，slot 指定首屏定席 */
export function Card({
  title,
  icon,
  full,
  slot,
  children,
}: {
  title: string;
  icon?: string;
  full?: boolean;
  slot?: keyof typeof SLOT_CLASS;
  children: ReactNode;
}) {
  const cls = ["up-card", full ? "up-card--full" : "", slot ? SLOT_CLASS[slot] : ""]
    .filter(Boolean)
    .join(" ");
  return (
    <section className={cls}>
      <h2 className="up-card__title">
        {icon ? <i aria-hidden>{icon}</i> : null}
        <span>{title}</span>
      </h2>
      <div className="up-card__body">{children}</div>
    </section>
  );
}

/** 资料行：标签定宽 + 虚线分隔（替代原两列表格的一整块） */
export function Row({ label, icon, children }: { label: string; icon?: string; children: ReactNode }) {
  return (
    <div className="up-row">
      <div className="up-row__label">
        {icon ? <i aria-hidden>{icon}</i> : null}
        <span>{label}</span>
      </div>
      <div className="up-row__value">{children}</div>
    </div>
  );
}
