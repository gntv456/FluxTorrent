"use client";

import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { extractYear } from "@/lib/name-builder";

/** 命名向导折叠面板（0525，从 upload-form-files.tsx 拆出防行数门禁）：
 *  元数据建议名 + 维度选中值的实时预览与「应用」写回。拼装口径在
 *  lib/name-builder.ts（buildName），本件只管展示与写回。 */

/** 「更多选项」同款按钮样式（源件 MORE_BTN_CLS 的镜像，拆件后就近复制） */
const WIZ_BTN_CLS =
  "min-h-10 rounded-[var(--r-sm)] border border-[var(--compat-line)] " +
  "px-3 text-sm font-bold text-ink";

export function UploadNameWizard({
  open,
  setOpen,
  metaName,
  preview,
  onApply,
}: {
  open: boolean;
  setOpen: (v: boolean) => void;
  /** PT-Gen 回包的建议名（中文名+外文名+年份），可能为空 */
  metaName: string;
  /** 父层实时拼装的主标题预览串（buildName 产物） */
  preview: string;
  /** 应用回调：写回主标题输入框 */
  onApply: (v: string) => void;
}) {
  const { dict } = useI18n();
  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className={WIZ_BTN_CLS + " w-fit text-xs"}
      >
        {open ? dict.upload.wizardClose : dict.upload.wizardOpen}
      </button>
      {open && (
        <div
          className="flex flex-col gap-1 rounded-[var(--r-sm)]
            border border-line p-2"
        >
          <span className="text-xs text-sub">
            {metaName
              ? fmt(dict.upload.wizardMetaName, {
                  name: metaName,
                  year: extractYear(metaName),
                })
              : dict.upload.wizardNoMeta}
          </span>
          <span
            className="min-h-8 break-all rounded-[var(--r-sm)]
              bg-[var(--surface-card)] p-2 font-mono text-xs"
          >
            {preview || dict.upload.wizardEmpty}
          </span>
          <button
            type="button"
            disabled={!preview}
            onClick={() => onApply(preview)}
            className={WIZ_BTN_CLS + " w-fit text-xs disabled:opacity-50"}
          >
            {dict.upload.wizardApply}
          </button>
          <span className="text-xs text-sub">{dict.upload.wizardHint}</span>
        </div>
      )}
    </>
  );
}
