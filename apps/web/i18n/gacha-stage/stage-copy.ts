import type { StageCopy } from "./stage-copy-types";
import { zhCN } from "./stage-copy-zh-cn";
import { zhTW } from "./stage-copy-zh-tw";
import { en } from "./stage-copy-en";
import { ja } from "./stage-copy-ja";

export type { StageCopy } from "./stage-copy-types";

export const STAGE_COPY: Record<string, StageCopy> = {
  "zh-CN": zhCN,
  "zh-TW": zhTW,
  en,
  ja,
};

export function stageCopy(locale: string): StageCopy {
  return STAGE_COPY[locale] ?? zhCN;
}

/** {name} 级占位替换 */
export function fill(
  tpl: string,
  vars: Record<string, string | number>,
): string {
  return tpl.replace(/\{(\w+)\}/g, (m, k: string) =>
    vars[k] === undefined ? m : String(vars[k]),
  );
}
