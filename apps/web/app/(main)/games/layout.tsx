import type { ReactNode } from "react";

/* 娱乐屋样式（2026-10-03 从 app/globals.css 根 @import 链移到这里）：
   arcade.css 是大厅游戏表面（纹章环/周常/票根册/星轨/货架/门禁），
   arcade-sweet.css 是「甜梦奇境 · 蓝调」皮肤覆盖层。
   此前挂在根 @import 链 → 全站 93 个页面都下载这 120KB 无关样式。
   Next.js 允许在 layout 里 import CSS，会自动按路由分包。 */
import "@/app/styles/arcade.css";
import "@/app/styles/arcade-sweet.css";
/* /gacha（抽卡）也消费 sw-* 皮肤类（.gtabs / .sw-card* / .sw-album*），
   arcade-gacha.css 是从 arcade-sweet.css 拆出的抽卡专用规则，
   同样走路由级加载。 */
import "@/app/styles/arcade-gacha.css";
/* arcade-fx.css：手感批（骰子滚动/刮格错峰/赢局星屑/连胜脉冲），
   独立成文件避免撑爆 arcade-sweet.css 的行数门禁（93% 满）。 */
import "@/app/styles/arcade-fx.css";
/* arcade-night.css：夜场赌场皮肤（波·成熟化）——令牌换血 + 金线面板 +
   发光交互，全部压在 arcade-fx 之后加载（覆盖 sweet 浅色遗留）。 */
import "@/app/styles/arcade-night.css";

/**
 * 娱乐屋路由布局：在 /games/* 下挂 data-arcade="sweet" 属性，
 * 让 arcade-sweet.css 的蓝调奇迹风作用域生效。
 * 不改任何业务组件，只换皮。
 */
export default function GamesLayout({ children }: { children: ReactNode }) {
  return <div data-arcade="sweet">{children}</div>;
}
