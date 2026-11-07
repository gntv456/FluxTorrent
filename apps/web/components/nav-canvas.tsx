"use client";

/** 主菜单两套渲染壳（2026-11-07 导航重构阶段 1）。
 *  NavColumns：brand / brand-night，点击逐级追加横向列。
 *  NavScroll ：shanhaijing 两档，点击逐级向下展卷 + 册页宫格。
 *  两者共用 useNavPath 的 path 状态与同一棵 NavNode 树。
 */
import { useEffect, useMemo, useRef, useState } from "react";
import Link from "next/link";
import type { NavNode } from "@/lib/nav-tree";
import { nodeById, pathToActive, searchNodes } from "@/lib/nav-tree";

export type CanvasProps = {
  tree: NavNode[];
  open: boolean;
  /** 触发面板的一级节点 id；进入时展开到它的子级 */
  rootId: string | null;
  active: (href: string) => boolean;
  onClose: () => void;
  onNavigate: (href: string) => void;
};

const CN = ["壹", "贰", "叁", "肆", "伍", "陆", "柒", "捌"];
const cn = (i: number) => CN[i] ?? String(i + 1);
const COL_W = 232;

export function useNavPath(
  tree: NavNode[],
  open: boolean,
  rootId: string | null,
  active: (href: string) => boolean,
) {
  const [path, setPath] = useState<string[]>([]);
  const [query, setQuery] = useState("");
  const activeRef = useRef(active);
  activeRef.current = active;
  useEffect(() => {
    if (!open) return;
    const hit = pathToActive(tree, (n) => activeRef.current(n.href));
    setPath(hit.length ? hit : rootId ? [rootId] : []);
    setQuery("");
  }, [open, rootId, tree]);
  const crumbs = useMemo(
    () => path.map((id) => nodeById(tree, id)).filter(Boolean) as NavNode[],
    [path, tree],
  );
  const columns = useMemo(() => {
    const cols: NavNode[][] = [tree];
    for (const id of path) {
      cols.push(nodeById(tree, id)?.children ?? []);
    }
    return cols;
  }, [path, tree]);
  const results = useMemo(() => searchNodes(tree, query), [tree, query]);
  return { path, setPath, query, setQuery, crumbs, columns, results };
}

function Crumb({
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

function Results({
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

function Head({
  query,
  setQuery,
  onClose,
  mark,
}: {
  query: string;
  setQuery: (v: string) => void;
  onClose: () => void;
  mark: string;
}) {
  return (
    <div className="nvc__head">
      <span className="nvc__mark">{mark}</span>
      <input className="nvc__search" value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="搜索任意层级…" aria-label="搜索导航" />
      <button type="button" className="nvc__close"
        onClick={onClose} aria-label="关闭菜单">×</button>
    </div>
  );
}

function Row({
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

export function NavColumns(props: CanvasProps) {
  const s = useNavPath(props.tree, props.open, props.rootId, props.active);
  const winRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const w = winRef.current;
    if (w) w.scrollLeft = w.scrollWidth;
  }, [s.path.length, props.open]);
  if (!props.open) return null;
  const go = (k: number, n: NavNode) => {
    if (n.children.length) s.setPath(s.path.slice(0, k).concat(n.id));
    else if (n.href) props.onNavigate(n.href);
  };
  const scroll = (d: number) => {
    const w = winRef.current;
    if (w) w.scrollLeft += d * COL_W;
  };
  return (
    <div className="nvc nvc--columns" role="menu">
      <Head query={s.query} setQuery={s.setQuery}
        onClose={props.onClose} mark="导航" />
      <Crumb
        crumbs={s.crumbs}
        back={(i) => s.setPath(s.path.slice(0, i + 1))}
      />
      {s.query ? (
        <Results items={s.results} active={props.active}
          onNavigate={props.onNavigate} />
      ) : (
        <div className="nvc__body">
          <div className="nvc__colwin" ref={winRef}>
            {s.columns.map((list, k) => (
              <div className="nvc__col" key={k}>
                <p className="nvc__cap">
                  <span>L{k + 1}</span>
                  {k > 0 ? s.crumbs[k - 1]?.label : "一级导航"}
                  <b>{list.length}</b>
                </p>
                {list.map((n) => (
                  <Row key={n.id} node={n}
                    on={s.path[k] === n.id || props.active(n.href)}
                    onClick={() => go(k, n)} />
                ))}
              </div>
            ))}
          </div>
          <button type="button" className="nvc__nav nvc__nav--l"
            onClick={() => scroll(-1)} aria-label="回看更早层级">‹</button>
          <button type="button" className="nvc__nav nvc__nav--r"
            onClick={() => scroll(1)} aria-label="查看最新层级">›</button>
        </div>
      )}
      <p className="nvc__foot">点击 › 展开下一级 · Esc 关闭 · / 搜索</p>
    </div>
  );
}

export function NavScroll(props: CanvasProps) {
  const s = useNavPath(props.tree, props.open, props.rootId, props.active);
  const bodyRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const b = bodyRef.current;
    if (b) b.scrollTop = b.scrollHeight;
  }, [s.path.length, props.open]);
  if (!props.open) return null;
  const list = s.columns[s.columns.length - 1] ?? [];
  const go = (n: NavNode) => {
    if (n.children.length) s.setPath(s.path.concat(n.id));
    else if (n.href) props.onNavigate(n.href);
  };
  return (
    <div className="nvc nvc--scroll" role="menu">
      <Head query={s.query} setQuery={s.setQuery}
        onClose={props.onClose} mark="卷" />
      <Crumb
        crumbs={s.crumbs}
        back={(i) => s.setPath(s.path.slice(0, i + 1))}
      />
      {s.query ? (
        <Results items={s.results} active={props.active}
          onNavigate={props.onNavigate} />
      ) : (
        <div className="nvc__scrollbody" ref={bodyRef}>
          {s.crumbs.map((n, i) => (
            <button type="button" className="nvc__strip" key={n.id}
              onClick={() => s.setPath(s.path.slice(0, i + 1))}>
              <span className="nvc__seal">{cn(i)}</span>
              卷{cn(i)} · {n.label}
              <span className="nvc__strip-hint">↩ 回到此卷</span>
            </button>
          ))}
          <div className="nvc__vol">
            <p className="nvc__volhd">
              <span className="nvc__seal">{cn(s.crumbs.length)}</span>
              卷{cn(s.crumbs.length)} · {list.length} 册
            </p>
            <div className="nvc__tiles">
              {list.map((n, i) => (
                <button type="button" className="nvc__tile" key={n.id}
                  data-leaf={n.children.length === 0 ? "true" : undefined}
                  onClick={() => go(n)}>
                  <span className="nvc__tileno">
                    {String(i + 1).padStart(2, "0")}
                  </span>
                  <span className="nvc__tilename">{n.label}</span>
                  <span className="nvc__tilemeta">
                    {n.children.length ? `${n.children.length} 册子目` : "直达"}
                  </span>
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
      <p className="nvc__foot">点击册页展下一卷 · Esc 关闭 · / 搜索</p>
    </div>
  );
}
