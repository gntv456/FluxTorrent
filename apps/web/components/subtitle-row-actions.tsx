"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import type { SubtitleRow } from "@/components/subtitle-board-shared";
import { SubtitleEditDialog } from "@/components/subtitle-edit-dialog";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";

/** 字幕行操作列（从 subtitle-board-table.tsx 拆出，300 行门禁）：
 *  举报 ⚑ + 本人/管理可见的 ✎ 编辑（0150 起弹全元数据表单）/ ✕ 删除。 */

/** 行操作内容（⚑ 举报 + 本人/管理可见的 ✎ 编辑 / ✕ 删除 + 各对话框）。
 *  M5.6 拆出：桌面表格包 <td> 用 SubtitleRowActions；移动卡片直接内联
 *  Inner——逻辑单源，不因视图分叉。 */
export function SubtitleRowActionsInner({
  s,
  canModify,
  onMsg,
  onReload,
  langs,
}: {
  s: SubtitleRow;
  canModify: boolean;
  onMsg: (m: string) => void;
  onReload: () => void;
  langs?: { code: string; name: string; flag: string }[];
}) {
  const { dict } = useI18n();
  const t = dict.subtitles;
  const [editing, setEditing] = useState(false);
  // A1 反馈协议：window.confirm → ConfirmDialog（举报/删除各一态）
  const [ask, setAsk] = useState<"report" | "del" | null>(null);

  async function confirmAsk() {
    if (!ask) return;
    try {
      if (ask === "report") {
        await api.post(`/api/v1/subtitles/${s.id}/report`, {});
        onMsg(t.reportOk);
      } else {
        await api.del(`/api/v1/subtitles/${s.id}`);
        onMsg(t.delOk ?? "");
      }
      setAsk(null);
      onReload();
    } catch (e) {
      onMsg(apiErrorMessage(dict, e));
    }
  }

  return (
    <>
      <button
        type="button"
        className="subtitles-report"
        title={t.reportTitle}
        aria-label={t.reportTitle}
        onClick={() => setAsk("report")}
      >
        ⚑
      </button>
      {canModify && (
        <>
          {" "}
          <button
            type="button"
            className="subtitles-report"
            title={t.editLabel}
            aria-label={t.editLabel}
            onClick={() => setEditing(true)}
          >
            ✎
          </button>
          {" "}
          <button
            type="button"
            className="subtitles-report"
            title={t.delLabel}
            aria-label={t.delLabel}
            onClick={() => setAsk("del")}
          >
            ✕
          </button>
          <ConfirmDialog
            open={ask !== null}
            onClose={() => setAsk(null)}
            onConfirm={() => void confirmAsk()}
            tone={ask === "del" ? "danger" : "neutral"}
            title={ask === "del" ? (t.delLabel ?? "delete") : t.reportTitle}
            message={
              ask === "del"
                ? (t.delConfirm ?? "delete?")
                : t.badMarkConfirm
            }
            confirmLabel={
              ask === "del" ? dict.ui.confirmDelete : dict.ui.confirmOk
            }
          />
          {editing && (
            <SubtitleEditDialog
              sid={s.id}
              langs={langs}
              initial={{
                title: s.title,
                fps: "",
                source: "",
                producer: "",
                proofreader: "",
                author_name: "",
                machine_translated: s.ai_state !== "human",
                lang: s.lang ?? "",
              }}
              onClose={() => setEditing(false)}
              onSaved={() => {
                setEditing(false);
                onMsg(t.editOk ?? "");
                onReload();
              }}
            />
          )}
        </>
      )}
    </>
  );
}

export function SubtitleRowActions({
  s,
  canModify,
  onMsg,
  onReload,
  langs,
}: {
  s: SubtitleRow;
  canModify: boolean;
  onMsg: (m: string) => void;
  onReload: () => void;
  langs?: { code: string; name: string; flag: string }[];
}) {
  return (
    <td className="text-center">
      <SubtitleRowActionsInner
        s={s}
        canModify={canModify}
        onMsg={onMsg}
        onReload={onReload}
        langs={langs}
      />
    </td>
  );
}

/** 行内评分下拉（登入可见；1-10 覆盖投票） */
export function SubtitleVoteSelect({
  s,
  onMsg,
  onReload,
}: {
  s: SubtitleRow;
  onMsg: (m: string) => void;
  onReload: () => void;
}) {
  const { dict } = useI18n();
  const t = dict.subtitles;
  return (
    <select
      className="ml-1 border-0 bg-transparent text-[11px] text-sub"
      aria-label={t.voteLabel}
      defaultValue=""
      onChange={async (e) => {
        const v = e.target.value;
        if (!v) return;
        e.target.value = "";
        try {
          await api.post(
            `/api/v1/subtitles/${s.id}/vote`,
            { score: Number(v) },
          );
          onMsg(t.voteOk ?? "");
          onReload();
        } catch (err) {
          onMsg(apiErrorMessage(dict, err));
        }
      }}
    >
      <option value="">{t.voteLabel}</option>
      {[10, 9, 8, 7, 6, 5, 4, 3, 2, 1].map((n) => (
        <option key={n} value={n}>
          {n}
        </option>
      ))}
    </select>
  );
}
