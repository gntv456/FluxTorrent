"use client";

/** 传统级联下拉菜单（2026-11-09 站长定调）：hover 展开、子级右侧飞出。
 *
 *  形态（参考经典纯 CSS 多级下拉）：
 *    - 一级条挂在顶栏 .mainmenu 上（main-menu.tsx 渲染）；
 *    - 悬停带子级的一级 → 正下方弹第一层 ul；悬停带子级的行 →
 *      右侧飞出下一层 ul，任意层级无限嵌套（树多深展多深）；
 *    - 展开全由 CSS :hover/:focus-within 驱动，无 JS 状态机——
 *      键盘 Tab 走 focus-within 同样能展开（可达性不降级）。
 *  数据还是 lib/nav-tree 的同一棵 NavNode 树；选中点亮口径不变
 *  （active 回调注入）。旧画布壳 NavColumns/NavScroll 保留给
 *  <lg 平板收敛口（compact 模式），桌面全面换本组件。
 */

import Link from "next/link";
import type { NavNode } from "@/lib/nav-tree";

type Props = {
  node: NavNode;
  /** 顶栏一级按钮（main-menu 渲染）；列内行自带样式 */
  top?: boolean;
  active: (href: string) => boolean;
  onNavigate: (href: string) => void;
};

/** 单行：叶子=链接直达；带子级=悬停右飞。空 href（分组名）只展开。 */
function DropRow({ node, active, onNavigate }: Props) {
  const hasKids = node.children.length > 0;
  const lit = active(node.href);
  const inner = (
    <>
      <span className="nd-label">{node.label}</span>
      {hasKids && <span className="nd-caret" aria-hidden>›</span>}
    </>
  );
  const cls = "nd-row" + (lit ? " is-lit" : "") + (hasKids ? "" : " nd-leaf");
  return (
    <li className={hasKids ? "nd-has" : undefined}>
      {hasKids || !node.href ? (
        <button type="button" className={cls}
          aria-haspopup={hasKids || undefined}>
          {inner}
        </button>
      ) : (
        <Link
          href={node.href}
          className={cls}
          data-active={lit ? "true" : undefined}
          onClick={() => onNavigate(node.href)}
        >
          {inner}
        </Link>
      )}
      {hasKids && (
        <NavDropList node={node} active={active} onNavigate={onNavigate} />
      )}
    </li>
  );
}

/** 一层 ul：一级层（top）正下弹，子层右侧飞。 */
export function NavDropList({
  node,
  active,
  onNavigate,
}: Omit<Props, "top"> & { node: NavNode }) {
  return (
    <ul className="nd-list">
      {node.children.map((c) => (
        <DropRow key={c.id} node={c} active={active} onNavigate={onNavigate} />
      ))}
    </ul>
  );
}
