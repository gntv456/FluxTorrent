import type { ReactNode } from "react";

/* 抽卡页专用样式（2026-10-03 新建 layout）。
   arcade-gacha.css 是从 arcade-sweet.css 拆出的抽卡消费规则
   （.gtabs / .sw-card* / .sw-album* / .sw-stat* / .sw-synth*），
   拆出来是因为 arcade-sweet.css 原先挂在 globals.css 根 @import 链，
   全站 93 页都下载 68KB，但只服务 /games 与 /gacha。 */
import "@/app/styles/arcade-gacha.css";

/** /gacha 路由布局：目前只负责路由级 CSS 加载，不改 DOM 结构。 */
export default function GachaLayout({ children }: { children: ReactNode }) {
  return <>{children}</>;
}
