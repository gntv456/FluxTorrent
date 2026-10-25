"use client";

import { useEffect, useMemo, useState } from "react";
import { Modal } from "@/components/modal";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";
import type { ForumCategory } from "@fluxtorrent/domain-types";
import {
  BTN_LINE,
  BTN_ROW_DANGER,
  BTN_SKY,
  blankBoardForm,
  formFromForum,
  type ClassTuple,
  type ForumAdminForum,
  type ForumBoardForm,
  type ModTuple,
} from "./forum-structure-types";
import { ForumBoardFields } from "./forum-board-fields";
import { ForumModsField } from "./forum-mods-field";

/** 新建/编辑版块抽屉。
 *
 *  ⚠️ 两条硬约束（防 B3 复发）：
 *  1. 打开时必须用该版块数据**回填**；旧版表单初值全空且无回填，
 *     点「编辑」后只填名称保存，会把三档门槛静默写成 0/0/0（权限放开）。
 *  2. 未改动任何字段时「保存」禁用（脏检查），避免误触提交空表单。
 *
 *  字段区在 ./forum-board-fields，版主任免在 ./forum-mods-field
 *  （样图版块表只有 编辑/排序，其余动作收进抽屉）；删除版块在
 *  本组件底部危险按钮，非空版块要「把版块名原样打一遍」才放行。
 */
export function ForumBoardDrawer({
  open,
  forum,
  categories,
  classes,
  defaultCatId,
  mods,
  busy,
  run,
  onClose,
}: {
  open: boolean;
  forum: ForumAdminForum | null;
  categories: ForumCategory[];
  /** 等级档候选（GET /admin/forums 附带 user_classes）；空则回落数字输入 */
  classes: ClassTuple[];
  defaultCatId: number | "";
  /** 全部版主元组（父页数据，任命/移除后由 run→load 整页刷新） */
  mods: ModTuple[];
  busy: boolean;
  run: (fn: () => Promise<void>, ok: string) => void;
  onClose: () => void;
}) {
  const initial = useMemo<ForumBoardForm>(
    () => (forum ? formFromForum(forum) : blankBoardForm(defaultCatId)),
    [forum, defaultCatId],
  );
  const [form, setForm] = useState<ForumBoardForm>(initial);
  // 删版块二次确认（danger）；免版主的确认在 ForumModsField 内部
  const ask = useConfirm();
  // hook 必须在 `if (!open) return null` 之前调用
  const { dict } = useI18n();

  // 回填：open / 目标版块变化时重置表单，杜绝脏 state 残留
  useEffect(() => {
    if (open) setForm(initial);
  }, [open, initial]);

  // 门槛下拉候选：等级全量 + 兜底并入当前值（历史数据可能指向已不存在的档）
  const gateOpts = useMemo<ClassTuple[]>(() => {
    const list = [...classes];
    for (const v of [initial.mr, initial.mw, initial.mc]) {
      if (!list.some(([id]) => id === v)) list.push([v, String(v)]);
    }
    return list.sort((a, b) => a[0] - b[0]);
  }, [classes, initial]);

  if (!open) return null;

  const t = dict.adminForums;
  const dirty = JSON.stringify(form) !== JSON.stringify(initial);
  const canSave = forum ? dirty : form.name.trim().length > 0;
  const gatesBad = !(form.mr <= form.mw && form.mw <= form.mc);

  function set<K extends keyof ForumBoardForm>(
    k: K,
    v: ForumBoardForm[K],
  ) {
    setForm((f) => ({ ...f, [k]: v }));
  }

  function save() {
    if (!canSave || gatesBad) return;
    const payload = {
      name: form.name.trim(),
      descr: form.descr.trim() || null,
      rules: form.rules.trim(),
      minclassread: form.mr,
      minclasswrite: form.mw,
      minclasscreate: form.mc,
      protected: form.prot,
      category_id: form.catId === "" ? null : Number(form.catId),
      sort: form.sort,
      // 0376 乐观并发：把抽屉读到的版本戳原样带回，服务端比对不一致
      // 就返 409（两人同时改同一版块，后保存的不会静默覆盖先保存的）。
      // 缺省不带 = 退回修前的 last-write-wins（新建时本来也没有戳）。
      ...(forum?.mtime ? { expected_mtime: forum.mtime } : {}),
    };
    run(
      async () => {
        if (forum) {
          await api.put(`/api/v1/admin/forums/${forum.id}`, payload);
        } else {
          await api.post("/api/v1/admin/forums", payload);
        }
        onClose();
      },
      forum ? t.boardSaved : t.boardCreated,
    );
  }

  /** 删除版块（仅编辑态）。非空版块后端要求 force=true，这里用
   *  「把版块名原样打一遍才放行」替用户完成那个显式知悉。 */
  function removeBoard() {
    if (!forum) return;
    const msg =
      forum.topics > 0
        ? fmt(t.boardDelNonEmpty, { name: forum.name, n: forum.topics })
        : fmt(t.boardDelEmpty, { name: forum.name });
    ask.open({
      title: t.delBoardBtn,
      message: msg,
      tone: "danger",
      busy,
      requireText: forum.topics > 0 ? forum.name : undefined,
      run: () =>
        run(
          async () => {
            const q = forum.topics > 0 ? "?force=true" : "";
            await api.del(`/api/v1/admin/forums/${forum.id}${q}`);
            onClose();
          },
          t.boardDeleted,
        ),
    });
  }

  return (
    <Modal
      open
      onClose={onClose}
      title={forum ? t.editTitle : t.newTitle}
      align="right"
    >
      <p className="mb-4 text-xs text-sub">
        {forum
          ? fmt(t.editSub, {
              name: forum.name,
              id: forum.id,
              n: forum.topics,
            })
          : t.newSub}
      </p>

      <ForumBoardFields
        form={form}
        set={set}
        categories={categories}
        gateOpts={gateOpts}
        gatesBad={gatesBad}
      />

      {forum && (
        <ForumModsField
          forumId={forum.id}
          mods={mods}
          busy={busy}
          run={run}
        />
      )}

      <div className="mt-5 flex items-center gap-2 border-t border-line pt-4">
        {forum && (
          <button
            type="button"
            disabled={busy}
            className={BTN_ROW_DANGER}
            onClick={removeBoard}
          >
            {t.delBoardBtn}
          </button>
        )}
        <span className="flex-1" />
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
      <AskDialog {...ask} />
    </Modal>
  );
}
