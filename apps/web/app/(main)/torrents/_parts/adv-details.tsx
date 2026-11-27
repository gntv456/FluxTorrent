"use client";

/**
 * 高级筛选面板的受控 <details> 壳。
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
 *
 * 懒面板（2026-11-27 首屏瘦身）：面板内容 386 个筛选 chip 占页面
 * payload ~150KB，而绝大多数访问不开面板。`lazy`（=服务端本次未渲染
 * body）时，summary 首次展开改为带 `adv=1` 硬导航——服务端按该信号
 * 渲染完整面板并自动展开；收起时把 adv 从 URL 摘掉（下一页回轻壳）。
 */
import { type ReactNode, useEffect, useRef } from "react";

export function AdvDetails({
  open,
  lazy,
  children,
}: {
  open: boolean;
  /** true = 服务端本次未下发面板 body（占位壳），首次展开需 adv=1 导航 */
  lazy?: boolean;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDetailsElement>(null);
  useEffect(() => {
    if (ref.current && ref.current.open !== open) ref.current.open = open;
  });

  /** 展开意图（summary 原生 toggle / 齿轮按钮共用）：lazy 壳 → 带 adv=1 整页取内容 */
  const expand = () => {
    if (!lazy) return;
    const u = new URL(window.location.href);
    u.searchParams.set("adv", "1");
    window.location.assign(u.toString());
  };

  /** 收起：把 adv 从 URL 摘掉（replaceState 不打断滚动位置），DOM 同步收 */
  const collapse = () => {
    const u = new URL(window.location.href);
    if (u.searchParams.has("adv")) {
      u.searchParams.delete("adv");
      window.history.replaceState(null, "", u.toString());
    }
    if (ref.current) ref.current.open = false;
  };

  return (
    <details
      className="tsb-adv tsb-adv--gear"
      id="tsb-adv"
      open={open}
      data-adv-lazy={lazy ? "1" : undefined}
      ref={ref}
      onToggle={(e) => {
        const nowOpen = (e.target as HTMLDetailsElement).open;
        if (nowOpen) expand();
        else collapse();
      }}
    >
      {children}
    </details>
  );
}
