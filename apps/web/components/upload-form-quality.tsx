"use client";

import { INPUT_BRAND } from "@/lib/ui-classes";
import { catPath } from "@/lib/site-profile";

import { useI18n } from "@/i18n/client";
import { FormRow, fieldCls } from "@/components/upload-form-parts";
import { SectionKindControl } from "@/components/section-kind-control";
import type {
  ProfileCat,
  SectionDictRow,
  SectionKindMeta,
} from "@/components/upload-form";

/** 发布表单·分类/质量/标签/推荐块（从 upload-form.tsx 按域拆出，300 行门禁）：
 *  分类下拉、质量维度（section_kinds 数据驱动，B2 起六类型）、标签多选
 *  （NP tags 口径）、推荐位（NP 挑选口径：置顶位置/截止 + 推荐影片）。
 *
 *  维度排版口径（2026-10-26，对齐正规 PT 站发布页）：「质量」分节头 +
 *  单个 rowfollow 格内的**响应式网格**（窄屏 1 列 / ≥sm 2 列 / ≥2xl 3 列），
 *  每格「右对齐标签 + 撑满控件」，多选独占整行。既不像原先把 42 个维度
 *  平铺进一个 flex-wrap 行那样参差杂乱，也不至于一行一个白占半屏。
 *  控件实现收敛到 section-kind-control（与编辑弹层同源，防口径漂移）。 */

/** 六类型里需要「字典选项」的两类；其余为自由值（直接填） */
const ENUM_TYPES = new Set(["select", "multiselect"]);

/** 维度网格：窄屏 1 列 / ≥sm 2 列 / ≥2xl 3 列（音乐 12 维一屏放得下） */
const GRID_CLS = "grid gap-x-6 gap-y-2 sm:grid-cols-2 2xl:grid-cols-3";
const CELL_CLS = "flex items-center gap-2";
/** 多选选项多，独占整行 */
const CELL_WIDE_CLS = `${CELL_CLS} sm:col-span-2 2xl:col-span-3`;
/** 定宽右对齐标签列，让同一列的控件左边缘对齐 */
const LABEL_CLS = "w-[5.5rem] shrink-0 text-right text-sm text-sub";

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
  /** 六类型统一以字符串承载：枚举存 dict_id，自由值存原文 */
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
  const u = dict.upload;

  // 质量维度渲染清单（0087，B2 扩六类型）：维度由 section_kinds 驱动。
  // 枚举维度必须有选项才显示（无选项 = 填不了）；自由值维度恒显示。
  const kindVal = (kind: string) => secVals[kind] ?? "";
  const setKindVal = (kind: string, v: string) => {
    setSecVals((prev) => ({ ...prev, [kind]: v }));
  };
  const kindDefs = secKinds
    .map((k) => ({
      kind: k.kind,
      label: k.label,
      type: k.field_type ?? "select",
      required: Boolean(k.required),
      opts: (secDict[k.kind] ?? []).map((d) => ({ v: d.id, label: d.name })),
    }))
    .filter((k) => (ENUM_TYPES.has(k.type) ? k.opts.length > 0 : true));

  return (
    <>
      <FormRow label={dict.upload.category}>
        {/* FormRow 的 rowhead td 不构成编程关联，可达名就地补（axe select-name） */}
        <select
          aria-label={dict.upload.category}
          name="category_id"
          value={categoryId}
          onChange={(e) => setCategoryId(Number(e.target.value))}
          className={fieldCls}
        >
          {(profileCats ?? []).map((c) => (
            <option key={c.id} value={c.id}>
              {catPath(profileCats ?? [], c)}
            </option>
          ))}
        </select>
      </FormRow>
      {kindDefs.length > 0 && (
        <>
          {/* 分节头（NP 发布页口径）：维度列表与上/下区块之间的视觉边界 */}
          <tr>
            <td className="colhead" colSpan={2}>
              {dict.upload.quality}
            </td>
          </tr>
          {/* 维度网格：一格里「标签 + 控件」，≥sm 两列、≥2xl 三列——一行一个
              在宽屏上白占半屏（音乐 12 维要滚一屏半）。多选选项多，独占整行。
              控件可达名就地补（表格 rowhead 不构成编程关联，axe 需可达名）。 */}
          <tr>
            <td className="rowfollow" colSpan={2}>
              <div className={GRID_CLS}>
                {kindDefs.map((k) => {
                  const wide = k.type === "multiselect";
                  return (
                    <div
                      key={k.kind}
                      className={wide ? CELL_WIDE_CLS : CELL_CLS}
                    >
                      <span className={LABEL_CLS}>
                        {k.label}
                        {k.required && (
                          <span className="text-[var(--danger,#d33)]"> *</span>
                        )}
                      </span>
                      <SectionKindControl
                        type={k.type}
                        value={kindVal(k.kind)}
                        onChange={(v) => setKindVal(k.kind, v)}
                        opts={k.opts.map((o) => ({ id: o.v, name: o.label }))}
                        noneLabel={u.gradeNone}
                        ariaLabel={k.label}
                        fluid
                      />
                    </div>
                  );
                })}
              </div>
            </td>
          </tr>
        </>
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
                        ? dict.upload.tagGroupAttr
                        : dict.upload.tagGroupContent}
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
                            className="h-4 w-4 accent-[var(--brand-orange)]"
                          />
                          {t.name}
                        </label>
                      );
                    })}
                </div>
              ))}
            <span className="text-xs text-sub">{dict.upload.tagsHint}</span>
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
                className={INPUT_BRAND}
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
                  className={INPUT_BRAND}
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
                className={INPUT_BRAND}
              >
                <option value="0">{dict.upload.recommendNone}</option>
                <option value="1">{dict.upload.recommendNormal}</option>
                <option value="2">{dict.upload.recommendClassic}</option>
              </select>
            </label>
          </div>
          <span className="text-xs text-sub">{dict.upload.recommendHint}</span>
        </div>
      </FormRow>
    </>
  );
}
