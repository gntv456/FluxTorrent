"use client";

import { useState } from "react";
import type { BoardBrief } from "@fluxtorrent/domain-types";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

/** 版块页「版主模式」批量工具栏。
 *
 *  对应 `POST /forums/topics/manage-batch`：一次提交一组主题的同一类改动。
 *  权限口径与单主题一致（逐个校验涉及版块的 can_mod，无权整体 403），
 *  所以这里不做"部分成功"的乐观提示——失败就整批失败并如实报错。
 *
 *  删除不在批量里（误选代价太大）：单主题删除在主题详情页的版主操作区。
 */

const BTN =
  "min-h-[32px] rounded-full border border-line bg-[var(--surface-card)] " +
  "px-3 text-xs font-bold text-sub active:scale-[0.97] " +
  "disabled:opacity-40";

const SELECT =
  "min-h-[32px] rounded-full border border-line bg-cloud px-3 " +
  "text-xs outline-none focus:border-sky disabled:opacity-40";

const PICKED_TAG =
  "rounded-full border border-[var(--border-strong)] " +
  "bg-[var(--surface-card)] px-2 py-0.5 text-[11px] " +
  "font-bold text-[var(--sky-deep)]";

/** 下拉标签带上分区名。
 *
 *  库里存在「分区与版块同名」的情况（实测有分区「资源交流」和版块「资源交流」），
 *  只显示版块名会让版主选错目标 —— 挂个分区名就区分开了。 */
function boardLabel(f: BoardBrief): string {
  return f.category_name ? `${f.name} · ${f.category_name}` : f.name;
}

export function ForumModBar({
  ids,
  pageIds,
  forums,
  onPickedAll,
  onDone,
}: {
  /** 当前选中的主题 id */
  ids: number[];
  /** 本页全部主题 id（全选用） */
  pageIds: number[];
  /** 可移动到的版块（全站，当前用户可读） */
  forums: BoardBrief[];
  onPickedAll: (ids: number[]) => void;
  onDone: () => void;
}) {
  const { dict } = useI18n();
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const empty = ids.length === 0;
  const allPicked = pageIds.length > 0 && ids.length === pageIds.length;

  async function run(op: Record<string, unknown>, label: string) {
    if (empty) return;
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/forums/topics/manage-batch", {
        ids,
        ...op,
      });
      setMsg(fmt(dict.forums.modDoneTpl, { label, n: ids.length }));
      onPickedAll([]);
      onDone();
    } catch (e) {
      setMsg(
        e instanceof ApiError
          ? fmt(dict.forums.modOpFailed, { msg: e.message })
          : dict.forums.modOpRetry,
      );
    } finally {
      setBusy(false);
    }
  }

  function move(target: number) {
    if (empty) return;
    const hit = forums.find((f) => f.id === target);
    const name = hit ? boardLabel(hit) : target;
    if (
      !window.confirm(
        fmt(dict.forums.modMoveConfirm, { n: ids.length, name: String(name) }),
      )
    ) {
      return;
    }
    run({ move_to_forum_id: target }, dict.forums.modMovedLabel);
  }

  return (
    <div
      className={
        "mb-2 flex flex-wrap items-center gap-2 rounded-[var(--r-md)] " +
        "border border-[var(--border-strong)] bg-[var(--sky-soft)] " +
        "px-3 py-2"
      }
    >
      <span className="text-xs font-bold text-[var(--sky-deep)]">
        {dict.forums.modBarTitle}
      </span>
      <span className={PICKED_TAG}>
        {fmt(dict.forums.modPicked, { n: ids.length })}
      </span>

      <span className="h-4 w-px bg-[var(--border-strong)]" aria-hidden="true" />

      <button
        type="button"
        className={BTN}
        disabled={busy || empty}
        onClick={() => run({ sticky: true }, dict.forums.modSticky)}
      >
        {dict.forums.modSticky}
      </button>
      <button
        type="button"
        className={BTN}
        disabled={busy || empty}
        onClick={() => run({ sticky: false }, dict.forums.modUnsticky)}
      >
        {dict.forums.modUnsticky}
      </button>
      <button
        type="button"
        className={BTN}
        disabled={busy || empty}
        onClick={() => run({ locked: true }, dict.forums.modLock)}
      >
        {dict.forums.modLock}
      </button>
      <button
        type="button"
        className={BTN}
        disabled={busy || empty}
        onClick={() => run({ locked: false }, dict.forums.modUnlock)}
      >
        {dict.forums.modUnlock}
      </button>
      <button
        type="button"
        className={BTN}
        disabled={busy || empty}
        onClick={() => run({ digest: true }, dict.forums.modDigest)}
      >
        {dict.forums.modDigest}
      </button>
      <button
        type="button"
        className={BTN}
        disabled={busy || empty}
        onClick={() => run({ digest: false }, dict.forums.modUndigest)}
      >
        {dict.forums.modUndigest}
      </button>

      <select
        className={SELECT}
        disabled={busy || empty}
        value=""
        onChange={(e) => {
          const t = Number(e.target.value);
          if (t) move(t);
        }}
        title={empty ? dict.forums.modPickFirst : dict.forums.modMoveTitle}
      >
        <option value="">{dict.forums.modMoveLabel}</option>
        {forums.map((f) => (
          <option key={f.id} value={f.id}>
            {boardLabel(f)}
          </option>
        ))}
      </select>

      <button
        type="button"
        className={BTN}
        disabled={busy || empty}
        onClick={() => onPickedAll([])}
      >
        {dict.forums.modClearPick}
      </button>

      <span className="flex-1" />

      <button
        type="button"
        className={BTN}
        disabled={pageIds.length === 0}
        onClick={() => onPickedAll(allPicked ? [] : pageIds)}
      >
        {allPicked ? dict.forums.modUnpickAll : dict.forums.modPickAll}
      </button>

      {msg && (
        <p role="status" className="w-full text-xs text-sub">
          {msg}
        </p>
      )}
    </div>
  );
}
