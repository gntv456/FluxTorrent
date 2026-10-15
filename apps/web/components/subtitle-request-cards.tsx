"use client";

import { useI18n } from "@/i18n/client";
import type { SubtitleRequestRow } from "@/components/subtitle-board-shared";
import type { SubReqAct } from "@/components/subtitle-request-actions";
import { SubtitleRequestActionsInner } from
  "@/components/subtitle-request-actions";
import { timeAgo } from "@/components/subtitle-board-format";

/** 样式常量（与字幕区卡片同范式，避免 JSX 行超 80 列门禁） */
const CARD =
  "rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-3";

/** 求字幕悬赏卡片（M5.6 §13.6 窄屏降级）：
 *  桌面四列表格（语言/发起人/悬赏/操作）390px 下挤爆。主行 = 语言 +
 *  悬赏（Free 标记）；次行 = 发起人 · 时间；页脚 = 工作流按钮
 *  （凑赏/认领/撤销/交稿/弃单/验收，与桌面同一份 Inner，逻辑不分叉）。 */
export function SubtitleRequestCards({
  reqs,
  langs,
  currency,
  meId,
  meName,
  wfOn,
  onAct,
  onContribute,
}: {
  reqs: SubtitleRequestRow[];
  langs: { code: string; name: string; flag: string }[];
  currency: string;
  meId: number;
  meName: string;
  wfOn: boolean;
  onAct: (a: SubReqAct, r: SubtitleRequestRow) => void;
  onContribute: (r: SubtitleRequestRow) => void;
}) {
  const { dict } = useI18n();
  const t = dict.subtitles;
  return (
    <ul className="flex flex-col gap-2">
      {reqs.map((r) => {
        // 语言码回显人话：与提交下拉同一份 langs 表；未知码回落原值
        const l = langs.find((x) => x.code === r.lang);
        return (
          <li key={r.id} className={CARD}>
            <div className="flex items-start justify-between gap-2">
              <span className="min-w-0 text-sm font-bold">
                {l ? `${l.flag} ${l.name}` : r.lang}
              </span>
              <span className="shrink-0 text-sm font-bold">
                {r.bounty} {currency}
                {r.offer_free && (
                  <span className="ml-1 text-[11px] text-mint">Free</span>
                )}
              </span>
            </div>
            <p className="mt-1 truncate text-xs text-sub">
              {r.username ?? "—"} · {timeAgo(r.created_at)}
            </p>
            <div className="mt-2 flex flex-wrap items-center gap-1">
              <SubtitleRequestActionsInner
                r={r}
                meId={meId}
                meName={meName}
                wfOn={wfOn}
                onAct={onAct}
                onContribute={() => onContribute(r)}
              />
            </div>
          </li>
        );
      })}
      {reqs.length === 0 && (
        <li className="py-4 text-center text-sub">{t.reqEmpty}</li>
      )}
    </ul>
  );
}
