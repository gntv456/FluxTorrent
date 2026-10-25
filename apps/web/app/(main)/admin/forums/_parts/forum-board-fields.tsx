"use client";

import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import type { ForumCategory } from "@fluxtorrent/domain-types";
import {
  INPUT_CLS,
  NUM_INPUT,
  type ClassTuple,
  type ForumBoardForm,
} from "./forum-structure-types";

/** 版块抽屉的字段区（名称/描述/规则/分区/三档门槛/排序/受保护）。
 *
 *  从抽屉主体拆出（300 行门禁）；纯受控组件，state 归抽屉。
 *  三档门槛的等级候选由抽屉算好传入（含历史脏值兜底）。
 */
export function ForumBoardFields({
  form,
  set,
  categories,
  gateOpts,
  gatesBad,
}: {
  form: ForumBoardForm;
  set: <K extends keyof ForumBoardForm>(
    k: K,
    v: ForumBoardForm[K],
  ) => void;
  categories: ForumCategory[];
  gateOpts: ClassTuple[];
  gatesBad: boolean;
}) {
  const { dict } = useI18n();
  const t = dict.adminForums;

  return (
    <>
      <label className="mb-3 block">
        <span className="mb-1 block text-xs text-sub">{t.fldName}</span>
        <input
          value={form.name}
          onChange={(e) => set("name", e.target.value)}
          className={INPUT_CLS}
        />
      </label>

      <label className="mb-3 block">
        <span className="mb-1 block text-xs text-sub">{t.fldDescr}</span>
        <input
          value={form.descr}
          onChange={(e) => set("descr", e.target.value)}
          className={INPUT_CLS}
        />
      </label>

      {/* 版块规则（0392）：换行按 pre-line 原样渲染；maxLength 只是
          提前拦一下，真上限 1000 字由后端 forum_upsert_check 判。 */}
      <label className="mb-3 block">
        <span className="mb-1 block text-xs text-sub">{t.fldRules}</span>
        <textarea
          value={form.rules}
          onChange={(e) => set("rules", e.target.value)}
          rows={4}
          maxLength={1000}
          className={INPUT_CLS}
        />
      </label>

      <label className="mb-3 block">
        <span className="mb-1 block text-xs text-sub">{t.fldCat}</span>
        <select
          value={form.catId === "" ? "" : String(form.catId)}
          onChange={(e) =>
            set("catId", e.target.value === "" ? "" : Number(e.target.value))
          }
          className={INPUT_CLS}
        >
          <option value="">{t.noCategory}</option>
          {categories.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
        </select>
      </label>

      <div className="mb-1">
        <span className="mb-1 block text-xs text-sub">{t.fldGates}</span>
        <div className="grid grid-cols-3 gap-2">
          {(
            [
              ["mr", t.gateRead],
              ["mw", t.gateWrite],
              ["mc", t.gateCreate],
            ] as const
          ).map(([k, label]) => (
            <label key={k} className="flex flex-col gap-1">
              <span className="text-[11px] text-sub">{label}</span>
              {gateOpts.length > 0 ? (
                <select
                  value={form[k]}
                  onChange={(e) => set(k, Number(e.target.value))}
                  className={NUM_INPUT}
                >
                  {gateOpts.map(([id, name]) => (
                    <option key={id} value={id}>
                      {id} · {name}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  type="number"
                  min={0}
                  max={99}
                  value={form[k]}
                  onChange={(e) => set(k, Number(e.target.value))}
                  className={NUM_INPUT}
                />
              )}
            </label>
          ))}
        </div>
        <p
          className={`mt-1 text-xs ${gatesBad ? "text-danger" : "text-sub"}`}
        >
          {gatesBad
            ? t.gatesBad
            : fmt(t.gatesOk, {
                r: form.mr,
                w: form.mw,
                c: form.mc,
              })}
        </p>
      </div>

      <label className="mb-3 mt-3 block">
        <span className="mb-1 block text-xs text-sub">{t.fldSort}</span>
        <input
          type="number"
          value={form.sort}
          onChange={(e) => set("sort", Number(e.target.value))}
          className={NUM_INPUT}
        />
      </label>

      <label className="mb-3 flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={form.prot}
          onChange={(e) => set("prot", e.target.checked)}
        />
        {t.fldProtect}
      </label>
      <p className="text-xs text-sub">{t.protectNote}</p>
    </>
  );
}
