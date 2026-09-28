/** 段级回落式新语言的深合并（E10）。
 *
 * 新语言字典是 DeepPartial<Dict>：翻到哪段哪段生效，未翻译的段/键
 * 自动回落 zh-CN。合并是纯数据深合并（数组/字符串叶子直接覆盖），
 * 不处理 React 元素（字典里没有）。
 */

import type { Dict } from "./zh-CN";
import type { DeepPartial } from "./ja";

/** 深合并：base 的结构与 partial 的覆盖（partial 优先，缺省回落 base） */
export function deepMergeDict(
  base: Dict,
  partial: DeepPartial<Dict>,
): Dict {
  return mergeNode(base, partial) as Dict;
}

function mergeNode<T>(base: T, patch: unknown): T {
  // 叶子（字符串/数字/布尔/null）或数组：直接用覆盖值；未定义回落 base
  if (patch === undefined) return base;
  if (
    typeof base !== "object" ||
    base === null ||
    Array.isArray(base) ||
    typeof patch !== "object" ||
    patch === null ||
    Array.isArray(patch)
  ) {
    return patch as T;
  }
  const out: Record<string, unknown> = { ...(base as Record<string, unknown>) };
  for (const [k, v] of Object.entries(patch as Record<string, unknown>)) {
    const bv = (base as Record<string, unknown>)[k];
    out[k] = bv === undefined ? v : mergeNode(bv, v);
  }
  return out as T;
}
