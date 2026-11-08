"use client";

/** 导航画布共享子件（2026-11-08 自 nav-canvas.tsx 外移，行数门禁拆件）：
 *  面包头 / 面包屑 / 搜索结果 / 状态条 / 列行——两套渲染壳共用。 */
import Link from "next/link";
import type { NavNode } from "@/lib/nav-tree";
import type { searchNodes } from "@/lib/nav-tree";

export function Crumb({
  crumbs,
  back,
}: {
  crumbs: NavNode[];
  back: (i: number) => void;
}) {
  return (
    <div className="nvc__crumb">
      <button type="button" onClick={() => back(-1)}>全站</button>
      {crumbs.map((n, i) => (
        <span key={n.id} className="nvc__crumb-seg">
          <span aria-hidden>›</span>
          <button type="button" data-end={i === crumbs.length - 1}
            onClick={() => back(i)}>{n.label}</button>
        </span>
      ))}
      <span className="nvc__crumb-hint">点击任一段回退</span>
    </div>
  );
}

export function Results({
  items,
  active,
  onNavigate,
}: {
  items: ReturnType<typeof searchNodes>;
  active: (href: string) => boolean;
  onNavigate: (href: string) => void;
}) {
  return (
    <div className="nvc__results">
      {items.length === 0 && <p className="nvc__empty">没有匹配项</p>}
      {items.map(({ node, path }) => (
        <Link key={node.id} href={node.href || "/"}
          className="nvc__res"
          data-active={active(node.href) ? "true" : undefined}
          onClick={() => onNavigate(node.href)}>
          <span className="nvc__res-name">{node.label}</span>
          <span className="nvc__res-path">{path}</span>
        </Link>
      ))}
    </div>
  );
}

export function Head({
  query,
  setQuery,
  onClose,
  mark,
  inputRef,
}: {
  query: string;
  setQuery: (v: string) => void;
  onClose: () => void;
  mark: string;
  /** 「/」快捷键聚焦目标（useSlashFocus 提供） */
  inputRef?: React.Ref<HTMLInputElement>;
}) {
  return (
    <div className="nvc__head">
      <span className="nvc__mark">{mark}</span>
      <input ref={inputRef} className="nvc__search" value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="搜索任意层级…" aria-label="搜索导航" />
      <button type="button" className="nvc__close"
        onClick={onClose} aria-label="关闭菜单">×</button>
    </div>
  );
}

/** 底部状态条（站长口径 2026-11-08）：当前展开层可「进入」——
 *  一级层也给直达口（症状①），末段与按钮同源取 chain 末节点。 */
export function Status({
  chain,
  onNavigate,
  goLabel,
}: {
  chain: NavNode[];
  onNavigate: (href: string) => void;
  goLabel: string;
}) {
  const last = chain.length ? chain[chain.length - 1] : null;
  if (!last || !last.href) return null;
  return (
    <div className="nvc__status">
      <span className="nvc__status-ic" aria-hidden>›</span>
      <span className="nvc__status-txt">
        <b>{last.label}</b>
        <i>{chain.map((n) => n.label).join(" › ")}</i>
      </span>
      <button type="button" className="nvc__status-go"
        onClick={() => onNavigate(last.href)}>
        {goLabel}
      </button>
    </div>
  );
}

export function Row({
  node,
  on,
  onClick,
}: {
  node: NavNode;
  on: boolean;
  onClick: () => void;
}) {
  const leaf = node.children.length === 0;
  return (
    <button type="button" className="nvc__row"
      data-active={on ? "true" : undefined}
      data-leaf={leaf ? "true" : undefined}
      onClick={onClick}>
      <span className="nvc__row-label">{node.label}</span>
      <span aria-hidden>{leaf ? "✓" : "›"}</span>
    </button>
  );
}
