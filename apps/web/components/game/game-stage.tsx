import type { ReactNode } from "react";

/** 玩法舞台外框：绘制底图 + 压暗罩 + 描金角，承载各页自己的交互件（canvas/网格/跑道）。
 *  纯展示组件（无 hooks、无 "use client"），供四个专注页复用；底图在 public/games/。 */
export function GameStage({
  art,
  children,
  className = "",
}: {
  art: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={`gs-stage ${className}`}>
      <div className="gs-art" style={{ backgroundImage: `url(${art})` }} />
      <div className="gs-scrim" />
      <span className="gs-corner tl" />
      <span className="gs-corner tr" />
      <span className="gs-corner bl" />
      <span className="gs-corner br" />
      <div className="gs-inner">{children}</div>
    </div>
  );
}
