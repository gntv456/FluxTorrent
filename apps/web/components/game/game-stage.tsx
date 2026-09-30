import type { ReactNode } from "react";

/** 玩法舞台外框：绘制底图 + 压暗罩 + 描金角，承载各页自己的交互件（canvas/网格/跑道）。
 *  纯展示组件（无 hooks、无 "use client"），供四个专注页复用；底图在 public/games/。
 *  `bg` 用于没有位图的新玩法（扭蛋/大转盘）：直接给一段 CSS 背景，不走 url()。 */
export function GameStage({
  art,
  bg,
  children,
  className = "",
}: {
  /** 位图底图路径（public/games/ 下）；与 `bg` 二选一 */
  art?: string;
  /** 直接 CSS 背景值（渐变等）；给了它就忽略 `art` */
  bg?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={`gs-stage ${className}`}>
      <div
        className="gs-art"
        style={
          bg ? { background: bg } : { backgroundImage: `url(${art ?? ""})` }
        }
      />
      <div className="gs-scrim" />
      <span className="gs-corner tl" />
      <span className="gs-corner tr" />
      <span className="gs-corner bl" />
      <span className="gs-corner br" />
      <div className="gs-inner">{children}</div>
    </div>
  );
}
