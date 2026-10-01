import type { ReactNode } from "react";

/**
 * 娱乐屋路由布局：在 /games/* 下挂 data-arcade="sweet" 属性，
 * 让 arcade-sweet.css 的蓝调奇迹风作用域生效。
 * 不改任何业务组件，只换皮。
 */
export default function GamesLayout({ children }: { children: ReactNode }) {
  return <div data-arcade="sweet">{children}</div>;
}
