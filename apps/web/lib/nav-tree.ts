/** 主菜单导航统一树（2026-11-07 导航重构阶段 1）。
 *
 *  把 main-menu.tsx 历史上三套形状（一级 sub / 四级 children /
 *  「更多」groups）归一成一棵 NavNode 树，供两套渲染壳共用：
 *    - nav-columns.tsx：brand / brand-night 的横向逐级列
 *    - nav-scroll.tsx ：shanhaijing 两档的纵向卷轴
 *  两套只换渲染壳，数据与 path 状态完全同源。
 *
 *  选中态口径沿用 2026-11-03 定稿的「键值全等」：带参条目只有
 *  URL 参数集合与菜单项完全一致才点亮；裸路径条目在任何带参
 *  条目命中时让位。见 tests/libmenu-nav-advanced.test.tsx。
 */
import type { NavGroup, NavItem } from "./nav-menu";

export type NavNode = {
  id: string;
  label: string;
  href: string;
  desc?: string;
  icon?: string;
  children: NavNode[];
};

const IGNORE_QUERY = new Set([
  "view", "limit", "cursor", "dir", "sort", "group",
]);

const text = (v: unknown): string =>
  v === null || v === undefined ? "" : String(v);

/** URL 是否精确携带 href 的 query（忽略展示态/翻页参数）。 */
export function queryActive(href: string, sp: URLSearchParams): boolean {
  const q = href.split("?")[1];
  if (!q) return false;
  const want = new URLSearchParams(q);
  const cur = new URLSearchParams();
  for (const k of sp.keys()) {
    if (!IGNORE_QUERY.has(k)) cur.set(k, sp.get(k) ?? "");
  }
  if (cur.size !== want.size) return false;
  for (const [k, v] of want) {
    if (cur.get(k) !== v) return false;
  }
  return true;
}

/** 所有叶子扁平化，用于算「带参条目已认领的路径」。 */
export function leavesOf(nodes: NavNode[]): NavNode[] {
  const out: NavNode[] = [];
  const walk = (list: NavNode[]) => {
    for (const n of list) {
      out.push(n);
      if (n.children.length) walk(n.children);
    }
  };
  walk(nodes);
  return out;
}

export function claimedPaths(
  leaves: NavNode[],
  sp: URLSearchParams,
): Set<string> {
  return new Set(
    leaves
      .filter((n) => queryActive(n.href, sp))
      .map((n) => n.href.split("?")[0]),
  );
}

export function isActive(
  href: string,
  pathname: string,
  sp: URLSearchParams,
  claimed: Set<string>,
): boolean {
  // 空 href 是分组节点（「更多」/ 学段组），不参与 URL 点亮
  if (!href) return false;
  const [path, query] = href.split("?");
  const hit = path === "/"
    ? pathname === "/"
    : pathname === path || pathname.startsWith(`${path}/`);
  if (!hit) return false;
  if (query) return queryActive(href, sp);
  return !claimed.has(path);
}

/** 一级项：sub（资源库/教育）与 children（教育四级）都归到 children。 */
function fromItem(n: NavItem, id: string): NavNode {
  const children: NavNode[] = [];
  (n.sub ?? []).forEach((g, gi) => {
    const gid = `${id}s${gi}`;
    children.push({
      id: gid,
      label: g.label,
      href: g.href,
      desc: g.label,
      children: (g.items ?? []).map((leaf, li) =>
        fromLeaf(leaf, `${gid}l${li}`)),
    });
  });
  (n.children ?? []).forEach((c, ci) => {
    children.push(fromLeaf(c, `${id}c${ci}`));
  });
  return {
    id,
    label: n.label,
    href: n.href,
    icon: undefined,
    children,
  };
}

function fromLeaf(n: NavItem, id: string): NavNode {
  return {
    id,
    label: n.label,
    href: n.href,
    children: (n.children ?? []).map((c, ci) =>
      fromLeaf(c, `${id}c${ci}`)),
  };
}

/** 「更多 ▾」的分组域：分组名作二级节点，组内条目作三级。 */
function fromGroups(groups: NavGroup[]): NavNode[] {
  return groups.map((g, gi) => ({
    id: `m${gi}`,
    label: g.group,
    href: "",
    desc: g.group,
    children: g.items.map((it, ii) => ({
      id: `m${gi}i${ii}`,
      label: it.label,
      href: it.href,
      children: (it.children ?? []).map((c, ci) =>
        fromLeaf(c, `m${gi}i${ii}c${ci}`)),
    })),
  }));
}

/** 构建统一树。moreLabel 为空时不生成「更多」节点。 */
export function buildNavTree(
  primary: NavItem[],
  groups: NavGroup[],
  moreLabel?: string,
): NavNode[] {
  const roots = primary.map((n, i) => fromItem(n, `p${i}`));
  if (moreLabel && groups.length) {
    roots.push({
      id: "more",
      label: moreLabel.replace(/\s*▾\s*$/, ""),
      href: "",
      desc: moreLabel,
      children: fromGroups(groups),
    });
  }
  return roots;
}

/** 从 URL 反推初始 path：展开到当前选中叶子所在层。
 *  首屏就高亮「你在这里」，同时让既有 data-active 回归用例成立。 */
export function pathToActive(
  tree: NavNode[],
  test: (n: NavNode) => boolean,
): string[] {
  let best: string[] = [];
  const walk = (list: NavNode[], trail: string[]) => {
    for (const n of list) {
      if (test(n) && trail.length >= best.length) best = trail.slice();
      if (n.children.length) walk(n.children, trail.concat(n.id));
    }
  };
  walk(tree, []);
  return best;
}

export function nodeById(tree: NavNode[], id: string): NavNode | null {
  for (const n of tree) {
    if (n.id === id) return n;
    const hit = nodeById(n.children, id);
    if (hit) return hit;
  }
  return null;
}

/** 跨层级搜索：返回命中节点及其完整路径标签。 */
export function searchNodes(
  tree: NavNode[],
  q: string,
  limit = 40,
): { node: NavNode; path: string }[] {
  const needle = text(q).trim().toLowerCase();
  if (!needle) return [];
  const out: { node: NavNode; path: string }[] = [];
  const walk = (list: NavNode[], trail: string[]) => {
    for (const n of list) {
      const path = trail.concat(n.label);
      if (path.join(" › ").toLowerCase().includes(needle) ||
        n.label.toLowerCase().includes(needle)) {
        out.push({ node: n, path: path.join(" › ") });
      }
      if (out.length < limit && n.children.length) walk(n.children, path);
    }
  };
  walk(tree, []);
  return out.slice(0, limit);
}
