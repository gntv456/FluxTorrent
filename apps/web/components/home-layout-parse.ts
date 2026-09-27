/** 首页排版解析（四审 L6 单源化）——**纯函数，无 "use client"**。
 *
 *  为什么单独抽这层：解析函数原先与 interface 一起放在 `home-layout.tsx`，
 *  但该文件顶层写了 `"use client"`，导致首页 RSC（server component）里调用
 *  `parseHomeLayout` 时报「Attempted to call a client function from the server」。
 *  解析逻辑本身纯计算、不依赖 React/浏览器，抽成服务端可用的共享模块即可
 *  让 RSC 与客户端组件共用同一份实现（单源化初衷不变）。
 *
 *  键、推荐占宽、默认排版**全部来自后端 `/home.home_sections`**（唯一清单在
 *  `apps/api/src/http/home_layout.rs`）；这里不再自带任何副本——此前 Rust 一份白名单、
 *  TS 三份（键 / 默认排版 / 推荐占宽），漂移默性表现为「后台存得进、首页不认」，
 *  且 `latest` 因前端 switch 没有分支而长期不吃排序与占宽。
 *  键集与前端渲染器表的对应由 `scripts/home_sections_guard.mjs` 比对把关。 */

export interface HomeLayoutItem {
  key: string;
  /** 0 = 用清单里的推荐占宽 */
  span: number;
}

export interface HomeSectionMeta {
  key: string;
  /** 1/2/3 = 1/3、2/3、整行 */
  span: number;
  in_default: boolean;
}

/** 未配置时的默认排版：直接取清单顺序（含 latest） */
export function defaultLayout(
  sections: HomeSectionMeta[],
): HomeLayoutItem[] {
  return sections
    .filter((s) => s.in_default)
    .map((s) => ({ key: s.key, span: 0 }));
}

/** 解析后端 home_layout。规则（单源化后）：
 *  - 空/非法 JSON/空数组 → 默认排版
 *  - 未知键、重复键 → **跳过该条**（旧行为是整份回退默认，等于站长配的全部作废）
 *  - span 只认 0..3，其余按 0（= 推荐档） */
export function parseHomeLayout(
  raw: string | undefined | null,
  sections: HomeSectionMeta[],
): HomeLayoutItem[] {
  // 清单为空＝混布场景（新前端撞上未下发 home_sections 的旧 api）：
  // 此时信任后端已强校验过的键，而不是把首页渲染成空白
  const known =
    sections.length > 0 ? new Set(sections.map((s) => s.key)) : null;
  if (!raw || !raw.trim()) return defaultLayout(sections);
  let arr: unknown;
  try {
    arr = JSON.parse(raw);
  } catch {
    return defaultLayout(sections);
  }
  if (!Array.isArray(arr) || arr.length === 0)
    return defaultLayout(sections);
  const items: HomeLayoutItem[] = [];
  const seen = new Set<string>();
  for (const it of arr as { key?: unknown; span?: unknown }[]) {
    const key = typeof it?.key === "string" ? it.key : "";
    if (!key || (known && !known.has(key)) || seen.has(key)) continue;
    seen.add(key);
    const span = typeof it.span === "number" && [0, 1, 2, 3].includes(it.span)
      ? it.span
      : 0;
    items.push({ key, span });
  }
  return items.length > 0 ? items : defaultLayout(sections);
}

/** 板块实际宽度档：显式 span 优先，0 = 清单推荐档；认不出的键落到整行 */
export function effectiveSpan(
  item: HomeLayoutItem,
  sections: HomeSectionMeta[],
): number {
  if (item.span > 0) return item.span;
  return sections.find((s) => s.key === item.key)?.span || 3;
}
