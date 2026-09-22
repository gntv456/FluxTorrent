"use client";

import { useEffect, useMemo, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import type { ForumCategory } from "@fluxtorrent/domain-types";
import {
  BTN_LINE, BTN_SKY, INPUT_CLS, NUM_INPUT,
  type ForumAdminForum,
} from "./forum-structure-types";

const DRAWER_CLS =
  "relative h-full w-[368px] overflow-y-auto border-l " +
  "border-line bg-cream p-5 shadow-2xl";

interface Form {
  name: string;
  descr: string;
  mr: number;
  mw: number;
  mc: number;
  prot: boolean;
  catId: number | "";
  sort: number;
}

/** 空表单（新建用；分区预选为当前选中的分区） */
function blank(defaultCatId: number | ""): Form {
  return {
    name: "",
    descr: "",
    mr: 0,
    mw: 0,
    mc: 0,
    prot: false,
    catId: defaultCatId,
    sort: 0,
  };
}

/** 由已有版块构造表单 —— 这是「编辑不可用」的修复核心 */
function fromForum(f: ForumAdminForum): Form {
  return {
    name: f.name,
    descr: f.descr ?? "",
    mr: f.minclassread,
    mw: f.minclasswrite,
    mc: f.minclasscreate,
    prot: f.protected,
    catId: typeof f.category_id === "number" ? f.category_id : "",
    sort: f.sort,
  };
}

/** 新建/编辑版块抽屉。
 *
 *  ⚠️ 两条硬约束（防 B3 复发）：
 *  1. 打开时必须用该版块数据**回填**；旧版表单初值全空且无回填，
 *     点「编辑」后只填名称保存，会把三档门槛静默写成 0/0/0（权限放开）。
 *  2. 未改动任何字段时「保存」禁用（脏检查），避免误触提交空表单。
 */
export function ForumBoardDrawer({
  open,
  forum,
  categories,
  defaultCatId,
  busy,
  run,
  onClose,
}: {
  open: boolean;
  forum: ForumAdminForum | null;
  categories: ForumCategory[];
  defaultCatId: number | "";
  busy: boolean;
  run: (fn: () => Promise<void>, ok: string) => void;
  onClose: () => void;
}) {
  const initial = useMemo<Form>(
    () => (forum ? fromForum(forum) : blank(defaultCatId)),
    [forum, defaultCatId],
  );
  const [form, setForm] = useState<Form>(initial);
  // hook 必须在 `if (!open) return null` 之前调用
  const { dict } = useI18n();

  // 回填：open / 目标版块变化时重置表单，杜绝脏 state 残留
  useEffect(() => {
    if (open) setForm(initial);
  }, [open, initial]);

  if (!open) return null;

  const t = dict.adminForums;
  const dirty = JSON.stringify(form) !== JSON.stringify(initial);
  const canSave = forum ? dirty : form.name.trim().length > 0;
  const gatesBad = !(form.mr <= form.mw && form.mw <= form.mc);

  function set<K extends keyof Form>(k: K, v: Form[K]) {
    setForm((f) => ({ ...f, [k]: v }));
  }

  function save() {
    if (!canSave || gatesBad) return;
    const payload = {
      name: form.name.trim(),
      descr: form.descr.trim() || null,
      minclassread: form.mr,
      minclasswrite: form.mw,
      minclasscreate: form.mc,
      protected: form.prot,
      category_id: form.catId === "" ? null : Number(form.catId),
      sort: form.sort,
    };
    run(async () => {
      if (forum) {
        await api.put(`/api/v1/admin/forums/${forum.id}`, payload);
      } else {
        await api.post("/api/v1/admin/forums", payload);
      }
      onClose();
    }, forum ? t.boardSaved : t.boardCreated);
  }

  return (
    <div className="fixed inset-0 z-50 flex justify-end">
      <button
        type="button"
        aria-label={t.close}
        className="absolute inset-0 bg-ink/30"
        onClick={onClose}
      />
      <section className={DRAWER_CLS}>
        <h3 className="font-display text-base font-bold text-ink">
          {forum ? t.editTitle : t.newTitle}
        </h3>
        <p className="mb-4 text-xs text-sub">
          {forum
            ? fmt(t.editSub, {
                name: forum.name,
                id: forum.id,
                n: forum.topics,
              })
            : t.newSub}
        </p>

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
            <input
              type="number"
              min={0}
              max={99}
              value={form.mr}
              onChange={(e) => set("mr", Number(e.target.value))}
              className={NUM_INPUT}
            />
            <input
              type="number"
              min={0}
              max={99}
              value={form.mw}
              onChange={(e) => set("mw", Number(e.target.value))}
              className={NUM_INPUT}
            />
            <input
              type="number"
              min={0}
              max={99}
              value={form.mc}
              onChange={(e) => set("mc", Number(e.target.value))}
              className={NUM_INPUT}
            />
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

        <div className="mt-5 flex justify-end gap-2 border-t border-line pt-4">
          <button type="button" className={BTN_LINE} onClick={onClose}>
            {t.cancel}
          </button>
          <button
            type="button"
            disabled={busy || !canSave || gatesBad}
            className={BTN_SKY}
            onClick={save}
            title={canSave ? undefined : t.untouched}
          >
            {forum ? t.saveBtn : t.boardCreateBtn}
          </button>
        </div>
      </section>
    </div>
  );
}
