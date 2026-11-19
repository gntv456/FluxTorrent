/** 日本語辞書（全 188 セグメント完全翻訳）
 *
 *  以前は「高露出セグメントのみ翻訳 + 残りは簡体字中国語へフォールバック」
 *  という DeepPartial 方式だったが、全セグメントの翻訳が完了したため、
 *  8 つのパートファイルをここで合成する。型は従来どおり DeepPartial<Dict>。
 *  カバレッジ検証: node scripts/ja_coverage.mjs
 */

import type { Dict } from "./zh-CN";
import { jaPart1 } from "./ja-part-1";
import { jaPart2 } from "./ja-part-2";
import { jaPart3 } from "./ja-part-3";
import { jaPart4 } from "./ja-part-4";
import { jaPart5 } from "./ja-part-5";
import { jaPart6 } from "./ja-part-6";
import { jaPart7 } from "./ja-part-7";
import { jaPart8 } from "./ja-part-8";

export const ja: DeepPartial<Dict> = {
  ...jaPart1,
  ...jaPart2,
  ...jaPart3,
  ...jaPart4,
  ...jaPart5,
  ...jaPart6,
  ...jaPart7,
  ...jaPart8,
};

/** 深い部分型（段落フォールバック互換のため残す） */
export type DeepPartial<T> = {
  [K in keyof T]?: T[K] extends object ? DeepPartial<T[K]> : T[K];
};
