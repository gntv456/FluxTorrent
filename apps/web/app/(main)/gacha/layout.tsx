import type { ReactNode } from "react";

/* 抽卡页专用样式（2026-10-03 新建 layout）。
   arcade-gacha.css 是从 arcade-sweet.css 拆出的抽卡消费规则
   （.gtabs / .sw-card* / .sw-album* / .sw-stat* / .sw-synth*），
   拆出来是因为 arcade-sweet.css 原先挂在 globals.css 根 @import 链，
   全站 93 页都下载 68KB，但只服务 /games 与 /gacha。 */
import "@/app/styles/arcade-gacha.css";
/* 夜场皮肤（波·成熟化）：/gacha 与 /games 同一套深色赌场令牌 */
import "@/app/styles/arcade-night.css";

/** /gacha 路由布局：挂 data-arcade="sweet"（夜场令牌作用域）+ 路由级 CSS。 */
export default function GachaLayout({ children }: { children: ReactNode }) {
  return <div data-arcade="sweet">{children}</div>;
}
