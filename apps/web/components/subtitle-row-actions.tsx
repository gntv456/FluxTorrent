"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { SubtitleRow } from "@/components/subtitle-board-shared";
import { SubtitleEditDialog } from "@/components/subtitle-edit-dialog";

/** 字幕行操作列（从 subtitle-board-table.tsx 拆出，300 行门禁）：
 *  举报 ⚑ + 本人/管理可见的 ✎ 编辑（0150 起弹全元数据表单）/ ✕ 删除。 */

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
  const { dict } = useI18n();
  const t = dict.subtitles;
  const [editing, setEditing] = useState(false);
  return (
    <td className="text-center">
      <button
        type="button"
        className="subtitles-report"
        title={t.reportTitle}
        aria-label={t.reportTitle}
        onClick={async () => {
          if (
            !window.confirm(
              t.badMarkConfirm,
            )
          )
            return;
          try {
            await api.post(
              `/api/v1/subtitles/${s.id}/report`,
              {},
            );
            onMsg(t.reportOk);
            onReload();
          } catch (e) {
            onMsg(
              e instanceof Error
                ? e.message
                : t.reportFail,
            );
          }
        }}
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
            onClick={async () => {
              if (!window.confirm(t.delConfirm ?? "delete?"))
                return;
              try {
                await api.del(`/api/v1/subtitles/${s.id}`);
                onMsg(t.delOk ?? "ok");
                onReload();
              } catch (e) {
                onMsg(
                  e instanceof Error ? e.message : "fail",
                );
              }
            }}
          >
            ✕
          </button>
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
                onMsg(t.editOk ?? "ok");
                onReload();
              }}
            />
          )}
        </>
      )}
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
          onMsg(t.voteOk ?? "ok");
          onReload();
        } catch (err) {
          onMsg(err instanceof Error ? err.message : "fail");
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
