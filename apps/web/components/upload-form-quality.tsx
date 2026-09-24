"use client";

import { INPUT_BAOZI } from "@/lib/ui-classes";

import { useI18n } from "@/i18n/client";
import { FormRow, fieldCls } from "@/components/upload-form-parts";
import type {
  ProfileCat,
  SectionDictRow,
  SectionKindMeta,
} from "@/components/upload-form";

/** 发布表单·分类/质量/标签/推荐块（从 upload-form.tsx 按域拆出，300 行门禁）：
 *  分类下拉、质量维度（section_kinds 数据驱动）、标签多选（NP tags 口径）、
 *  推荐位（NP 挑选口径：置顶位置/截止 + 推荐影片）。 */

export function UploadQualityBlock({
  profileCats,
  categoryId,
  setCategoryId,
  secKinds,
  secDict,
  secVals,
  setSecVals,
  tagDict,
  tagGroups,
  tagSel,
  setTagSel,
  posState,
  setPosState,
  posUntil,
  setPosUntil,
  pickType,
  setPickType,
}: {
  profileCats: ProfileCat[] | null;
  categoryId: number;
  setCategoryId: (v: number) => void;
  secKinds: SectionKindMeta[];
  secDict: Record<string, SectionDictRow[]>;
  secVals: Record<string, string>;
  setSecVals: React.Dispatch<React.SetStateAction<Record<string, string>>>;
  tagDict: { id: number; name: string; kind: string }[];
  /** 平行数组（0160 P2）：与 tagDict 下标对齐，'attribute' | 'content'；缺省全 attribute */
  tagGroups?: string[];
  tagSel: number[];
  setTagSel: React.Dispatch<React.SetStateAction<number[]>>;
  posState: number;
  setPosState: (v: number) => void;
  posUntil: string;
  setPosUntil: (v: string) => void;
  pickType: number;
  setPickType: (v: number) => void;
}) {
  const { dict } = useI18n();

  // 质量维度渲染清单（0087）：九维全部由 section_kinds/section_dict 驱动，
  // 统一写 sections JSON（media/grades/editions 也走 torrent_sections）
  const kindDefs = secKinds
    .map((k) => ({
      kind: k.kind,
      label: k.label,
      opts: (secDict[k.kind] ?? []).map((d) => ({ v: d.id, label: d.name })),
    }))
    .filter((k) => k.opts.length > 0);
  const kindVal = (kind: string) => secVals[kind] ?? "";
  const setKindVal = (kind: string, v: string) => {
    setSecVals((prev) => ({ ...prev, [kind]: v }));
  };

  return (
    <>
      <FormRow label={dict.upload.category}>
        <select
          value={categoryId}
          onChange={(e) => setCategoryId(Number(e.target.value))}
          className={fieldCls}
        >
          {(profileCats ?? []).map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
        </select>
      </FormRow>
      {kindDefs.length > 0 && (
        <FormRow label={dict.upload.quality}>
          <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
            {kindDefs.map(({ kind, label, opts }) => (
              <label key={kind} className="flex items-center gap-1 text-sm">
                <span className="whitespace-nowrap text-sub">{label}：</span>
                <select
                  value={kindVal(kind)}
                  onChange={(e) => setKindVal(kind, e.target.value)}
                  className={INPUT_BAOZI}
                >
                  <option value="">{dict.upload.gradeNone}</option>
                  {opts.map((o) => (
                    <option key={o.v} value={o.v}>
                      {o.label}
                    </option>
                  ))}
                </select>
              </label>
            ))}
          </div>
        </FormRow>
      )}
      {tagDict.filter((t) => t.kind !== "official").length > 0 && (
        <FormRow label={dict.upload.tags}>
          <div className="flex flex-col gap-1">
            {/* 0160 P2：按组分区（属性类 / 内容类），无组信息时退化为平铺 */}
            {(["attribute", "content"] as const)
              .filter((g) =>
                tagDict.some(
                  (t, i) =>
                    t.kind !== "official" &&
                    (tagGroups?.[i] ?? "attribute") === g,
                ),
              )
              .map((g) => (
                <div
                  key={g}
                  className="flex flex-wrap items-center gap-x-4 gap-y-2"
                >
                  {(tagGroups ?? []).some((x) => x === "content") && (
                    <span className="text-xs font-bold text-sub">
                      {g === "attribute"
                        ? (dict.upload.tagGroupAttr)
                        : (dict.upload.tagGroupContent)}
                      ：
                    </span>
                  )}
                  {tagDict
                    .filter(
                      (t, i) =>
                        t.kind !== "official" &&
                        (tagGroups?.[i] ?? "attribute") === g,
                    )
                    .map((t) => {
                      const on = tagSel.includes(t.id);
                      return (
                        <label
                          key={t.id}
                          className="flex cursor-pointer items-center gap-1.5 text-sm"
                        >
                          <input
                            type="checkbox"
                            checked={on}
                            onChange={() =>
                              setTagSel((prev) =>
                                on
                                  ? prev.filter((x) => x !== t.id)
                                  : [...prev, t.id],
                              )
                            }
                            className="h-4 w-4 accent-[var(--baozi-orange)]"
                          />
                          {t.name}
                        </label>
                      );
                    })}
                </div>
              ))}
            <span className="text-xs text-sub">
              {dict.upload.tagsHint}
            </span>
          </div>
        </FormRow>
      )}
      <FormRow label={dict.upload.recommend}>
        <div className="flex flex-col gap-1">
          <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
            <label className="flex items-center gap-1 text-sm">
              <span className="whitespace-nowrap text-sub">
                {dict.upload.pickPos}：
              </span>
              <select
                value={posState}
                onChange={(e) => setPosState(Number(e.target.value))}
                className={INPUT_BAOZI}
              >
                <option value="0">{dict.upload.pickNone}</option>
                <option value="1">{dict.upload.pickL1}</option>
                <option value="2">{dict.upload.pickL2}</option>
              </select>
            </label>
            {posState > 0 && (
              <label className="flex items-center gap-1 text-sm">
                <span className="whitespace-nowrap text-sub">
                  {dict.upload.pickUntil}：
                </span>
                <input
                  type="datetime-local"
                  value={posUntil}
                  onChange={(e) => setPosUntil(e.target.value)}
                  className={INPUT_BAOZI}
                />
              </label>
            )}
            <label className="flex items-center gap-1 text-sm">
              <span className="whitespace-nowrap text-sub">
                {dict.upload.recommendMovie}：
              </span>
              <select
                value={pickType}
                onChange={(e) => setPickType(Number(e.target.value))}
                className={INPUT_BAOZI}
              >
                <option value="0">{dict.upload.recommendNone}</option>
                <option value="1">
                  {dict.upload.recommendNormal}
                </option>
                <option value="2">
                  {dict.upload.recommendClassic}
                </option>
              </select>
            </label>
          </div>
          <span className="text-xs text-sub">
            {dict.upload.recommendHint}
          </span>
        </div>
      </FormRow>
    </>
  );
}
