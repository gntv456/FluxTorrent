"use client";

/** 传统级联下拉菜单（2026-11-09 站长定调）：hover 展开、子级右侧飞出。
 *
 *  形态（参考经典纯 CSS 多级下拉）：
 *    - 一级条挂在顶栏 .mainmenu 上（main-menu.tsx 渲染）；
 *    - 悬停带子级的一级 → 正下方弹第一层 ul；悬停带子级的行 →
 *      右侧飞出下一层 ul，任意层级无限嵌套（树多深展多深）；
 *    - 展开由 CSS :hover/:focus-within 驱动，JS 只同步 aria 语义与
 *      点击展开态；键盘 Tab 走 focus-within 同样能展开。
 *  数据还是 lib/nav-tree 的同一棵 NavNode 树；选中点亮口径不变
 *  （active 回调注入，父链按子树命中上溯）。
 */

import Link from "next/link";
import { useState } from "react";
import { subtreeActive, type NavNode } from "@/lib/nav-tree";

type Props = {
  node: NavNode;
  /** 一级面板是否右缘反手（最后一项挂「更多」，防 1024 出屏） */
  flip?: boolean;
  active: (href: string) => boolean;
  onNavigate: (href: string) => void;
};

/** 单行：叶子=链接直达；带子级=悬停右飞。空 href（分组名）只展开。 */
function DropRow({ node, active, onNavigate }: Props) {
  const hasKids = node.children.length > 0;
  const lit =
    active(node.href) || (hasKids && subtreeActive(node.children, active));
  const [open, setOpen] = useState(false);
  const inner = (
    <>
      <span className="nd-label">{node.label}</span>
      {hasKids && <span className="nd-caret" aria-hidden>›</span>}
    </>
  );
  const cls =
    "nd-row" + (lit ? " is-lit" : "") + (hasKids ? "" : " nd-leaf");
  return (
    <li
      className={hasKids ? "nd-has" : undefined}
      data-open={open ? "true" : undefined}
      onPointerEnter={() => hasKids && setOpen(true)}
      onPointerLeave={() => setOpen(false)}
      onFocusCapture={() => hasKids && setOpen(true)}
      onBlurCapture={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node)) setOpen(false);
      }}
    >
      {hasKids || !node.href ? (
        <button
          type="button"
          className={cls}
          aria-haspopup={hasKids || undefined}
          aria-expanded={hasKids ? open : undefined}
          // 空 href 且无子级时仅作分组标题，点击无动作
          onClick={hasKids ? () => setOpen((o) => !o) : undefined}
        >
          {inner}
        </button>
      ) : (
        <Link
          href={node.href}
          className={cls}
          aria-current={lit ? "page" : undefined}
          data-active={lit ? "true" : undefined}
          onClick={() => onNavigate(node.href)}
        >
          {inner}
        </Link>
      )}
      {hasKids && (
        <NavDropList
          node={node}
          active={active}
          onNavigate={onNavigate}
        />
      )}
    </li>
  );
}

/** 一层 ul：一级层正下弹，子层右侧飞。叶子面板自带内滚动。 */
export function NavDropList({
  node,
  active,
  onNavigate,
  flip,
}: Props) {
  const leafOnly = node.children.every((c) => c.children.length === 0);
  return (
    <ul
      className={
        "nd-list" +
        (flip ? " nd-list--flip" : "") +
        (leafOnly ? " nd-list--scroll" : "")
      }
    >
      {node.children.map((c) => (
        <DropRow
          key={c.id}
          node={c}
          active={active}
          onNavigate={onNavigate}
        />
      ))}
    </ul>
  );
}
