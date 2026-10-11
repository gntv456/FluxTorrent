"use client";

import { useI18n } from "@/i18n/client";
import type { SubtitleRequestRow } from "@/components/subtitle-board-shared";

/** 求字幕行操作列（从 subtitle-request-panel.tsx 拆出，300 行门禁）：
 *  凑赏 / 认领 / 撤销（P-43）/ 交稿 / 弃单 / 验收。
 *
 *  归属口径（P-45）：验收按钮此前写成 `r.status === 4 && r.username`，
 *  即**任何登录者都看得见**、点下去被服务端 403——按钮的可见性应当说真话。
 *  现统一按 `r.username === me.username` 判发起人（列表只回 username，
 *  没有 user_id），管理侧兜底走接口不走这个按钮。 */

export type SubReqAct =
  | "claim"
  | "abandon"
  | "deliver"
  | "accept"
  | "cancel";

export function SubtitleRequestActions({
  r,
  meId,
  meName,
  wfOn,
  onAct,
  onContribute,
}: {
  r: SubtitleRequestRow;
  meId: number;
  meName: string;
  wfOn: boolean;
  onAct: (a: SubReqAct, r: SubtitleRequestRow) => void;
  onContribute: (id: number) => void;
}) {
  const { dict } = useI18n();
  const t = dict.subtitles;
  const mine = !!r.username && r.username === meName;
  return (
    <td>
      <button type="button" className="btn2" onClick={() => onContribute(r.id)}>
        {t.contributeBtn}
      </button>
      {wfOn && r.status === 0 && (
        <button
          type="button"
          className="btn2 ml-1"
          onClick={() => onAct("claim", r)}
        >
          {t.claimBtn}
        </button>
      )}
      {r.status === 0 && mine && (
        <button
          type="button"
          className="btn2 ml-1"
          onClick={() => onAct("cancel", r)}
        >
          {t.cancelBtn}
        </button>
      )}
      {wfOn && r.status === 3 && r.claimed_by === meId && (
        <>
          <button
            type="button"
            className="btn2 ml-1"
            onClick={() => onAct("deliver", r)}
          >
            {t.deliverBtn}
          </button>
          <button
            type="button"
            className="btn2 ml-1"
            onClick={() => onAct("abandon", r)}
          >
            {t.abandonBtn}
          </button>
        </>
      )}
      {wfOn && r.status === 4 && mine && (
        <button
          type="button"
          className="btn2 ml-1"
          onClick={() => onAct("accept", r)}
        >
          {t.acceptBtn}
        </button>
      )}
    </td>
  );
}
