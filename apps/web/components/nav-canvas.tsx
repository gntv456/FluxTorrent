"use client";

/** 主菜单两套渲染壳（2026-11-07 导航重构阶段 1）。
 *  NavColumns：brand / brand-night，点击逐级追加横向列。
 *  NavScroll ：shanhaijing 两档，点击逐级向下展卷 + 册页宫格。
 *  两者共用 useNavPath 的 path 状态与同一棵 NavNode 树。
 *  2026-11-08 症状修复：面板列从 crumbs 子级起（rootId 进入第一列
 *  直达二级，不再重复一级平铺）；底部状态条给当前层「进入」直进口
 *  （一级层也直达）——一级菜单的页面入口由此恢复。共享子件在
 *  nav-canvas-parts.tsx。
 */
import { useEffect, useMemo, useRef, useState } from "react";
import type { NavNode } from "@/lib/nav-tree";
import {
  nodeById,
  nodesAt,
  pathToActive,
  searchNodes,
} from "@/lib/nav-tree";
import { Crumb, Head, Results, Row, Status } from "./nav-canvas-parts";

const COL_W = 232;

/** 面板开启时「/」聚焦搜索（foot 提示兑现，2026-11-08 体验批）。
 *  输入框已在焦点时不劫持（正常打字）。 */
function useSlashFocus(open: boolean) {
  const searchRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "/" || e.defaultPrevented) return;
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA")) return;
      e.preventDefault();
      searchRef.current?.focus();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open]);
  return searchRef;
}

export type CanvasProps = {
  tree: NavNode[];
  open: boolean;
  /** 触发面板的一级节点 id；进入时展开到它的子级 */
  rootId: string | null;
  active: (href: string) => boolean;
  onClose: () => void;
  onNavigate: (href: string) => void;
  /** 状态条按钮文案（i18n：nav.goCurrent） */
  goLabel: string;
};

const CN = ["壹", "贰", "叁", "肆", "伍", "陆", "柒", "捌"];
const cn = (i: number) => CN[i] ?? String(i + 1);

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
    // rootId 进入且高亮命中不在其子树（如在别的频道开本面板）：
    // 回退 [rootId]，避免 columns 全空
    const init =
      rootId && hit[0] !== rootId
        ? [rootId]
        : hit.length
          ? hit
          : rootId
            ? [rootId]
            : [];
    setPath(init);
    setQuery("");
  }, [open, rootId, tree]);
  const crumbs = useMemo(
    () => path.map((id) => nodeById(tree, id)).filter(Boolean) as NavNode[],
    [path, tree],
  );
  /** 列从 crumbs 的子级起（点「资源库 ▾」进来第一列就是它的二级，
   *  不再重复一级平铺）；仅「更多 ▾」无 path 时才从一级列起。 */
  const columns = useMemo(() => {
    const cols: NavNode[][] = [];
    for (const id of path) {
      const kids = nodeById(tree, id)?.children ?? [];
      if (kids.length) cols.push(kids);
    }
    if (!path.length) cols.unshift(tree);
    return cols;
  }, [path, tree]);
  /** path 链上节点（含一级）：状态条「进入当前层」的取数源 */
  const chain = useMemo(() => nodesAt(tree, path), [path, tree]);
  const results = useMemo(() => searchNodes(tree, query), [tree, query]);
  return { path, setPath, query, setQuery, crumbs, columns, chain, results };
}

export function NavColumns(props: CanvasProps) {
  const s = useNavPath(props.tree, props.open, props.rootId, props.active);
  const searchRef = useSlashFocus(props.open);
  const winRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const w = winRef.current;
    if (w) w.scrollLeft = w.scrollWidth;
  }, [s.path.length, props.open]);
  if (!props.open) return null;
  /** 列 k 的行点击后 path 截到该列父链再追加；rootCol（一级列起）
   *  与 rootId 进入（二级列起）的 chain 对位差一，分层处理。 */
  const go = (k: number, n: NavNode) => {
    if (n.children.length) {
      s.setPath(s.chain.slice(0, k + 1).map((c) => c.id).concat(n.id));
    } else if (n.href) props.onNavigate(n.href);
  };
  const scroll = (d: number) => {
    const w = winRef.current;
    if (w) w.scrollLeft += d * COL_W;
  };
  const rootCol = s.columns[0] === props.tree;
  return (
    <div className="nvc nvc--columns" role="menu">
      <Head query={s.query} setQuery={s.setQuery}
        onClose={props.onClose} mark="导航" inputRef={searchRef} />
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
            {s.columns.map((list, k) => {
              /** 列 k 的父节点：rootCol 头列=null（列即一级），
               *  其余取 chain[k]（rootId 进入时 chain[0] 是一级本身）。 */
              const parent = rootCol
                ? k === 0 ? null : s.chain[k - 1]
                : s.chain[k];
              /** 层级号：rootCol 列 k=Lk+1（头列=一级）；
               *  rootId 进入第一列已是二级，显示 Lk+2。 */
              const lv = rootCol ? k + 1 : k + 2;
              return (
                <div className="nvc__col" key={k}>
                  <p className="nvc__cap">
                    <span>L{lv}</span>
                    {parent ? parent.label : "一级导航"}
                    <b>{list.length}</b>
                  </p>
                  {list.map((n) => (
                    <Row key={n.id} node={n}
                      on={s.chain[k + (rootCol ? 0 : 1)]?.id === n.id ||
                        props.active(n.href)}
                      onClick={() => go(k, n)} />
                  ))}
                </div>
              );
            })}
          </div>
          <button type="button" className="nvc__nav nvc__nav--l"
            onClick={() => scroll(-1)} aria-label="回看更早层级">‹</button>
          <button type="button" className="nvc__nav nvc__nav--r"
            onClick={() => scroll(1)} aria-label="查看最新层级">›</button>
        </div>
      )}
      <Status chain={s.chain} onNavigate={props.onNavigate}
        goLabel={props.goLabel} />
      <p className="nvc__foot">点击 › 展开下一级 · Esc 关闭 · / 搜索</p>
    </div>
  );
}

export function NavScroll(props: CanvasProps) {
  const s = useNavPath(props.tree, props.open, props.rootId, props.active);
  const searchRef = useSlashFocus(props.open);
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
        onClose={props.onClose} mark="卷" inputRef={searchRef} />
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
      <Status chain={s.chain} onNavigate={props.onNavigate}
        goLabel={props.goLabel} />
      <p className="nvc__foot">点击册页展下一卷 · Esc 关闭 · / 搜索</p>
    </div>
  );
}
