"use client";

/** 高级筛选面板的受控 <details> 壳。
 *
 * 背景（2026-11-09 站长报「切资源库/教育子菜单高级筛选都开着」）：
 * 齿轮按钮/收起钮/summary 原生交互都是直改 DOM 的 open 属性，不触发
 * React 渲染——软导航（点菜单换 URL）后 prop 若无变化（false→false 无
 * diff），手开的面板跨导航粘住不关；反过来手关的也会粘住不弹。
 *
 * 口径：open prop（服务端 advAutoOpen 判定）是导航切换后的「应答态」。
 * 每次渲染后把 DOM 对齐到 prop：纯导航落地强制收起，带真筛选强制展开。
 * effect 不带依赖数组——本组件只在 RSC 更新（软导航换 URL）时随父级
 * 重渲染，齿轮/summary 直改 DOM 不触发渲染，手开手关的即时手感保留。
 */
import { type ReactNode, useEffect, useRef } from "react";

export function AdvDetails({
  open,
  children,
}: {
  open: boolean;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDetailsElement>(null);
  useEffect(() => {
    if (ref.current && ref.current.open !== open) ref.current.open = open;
  });
  return (
    <details
      className="tsb-adv tsb-adv--gear"
      id="tsb-adv"
      open={open}
      ref={ref}
    >
      {children}
    </details>
  );
}
