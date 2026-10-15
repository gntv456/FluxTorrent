"use client";

import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import type { SubtitleRow } from "@/components/subtitle-board-shared";
import {
  SubtitleRowActionsInner,
  SubtitleVoteSelect,
} from "@/components/subtitle-row-actions";
import {
  downloadSubtitle,
  flagText,
  fmtKB,
  langLabelOf,
  timeAgo,
} from "@/components/subtitle-board-format";

/** 样式常量（抽出来避免 JSX 内联 className 行超 80 列门禁） */
const CARD =
  "rounded-[var(--r-md)] border border-line " +
  "bg-[var(--surface-card)] p-3";
const META = "flex justify-between gap-2";

/** 字幕卡片视图（窄屏 <640 渲染；M5.6 §13.6 多列表格列降级）。
 *
 * 桌面是七列表格（语言/标题/时间/大小/点击/上传者/操作），
 * baozi-wide-table-scroll 只保证不撑破 body，看全一行仍要横向拖。
 * 按概念稿口径降级：主行 = 旗帜 + 片名（含 ✓/👑/MT 徽标）+ ⬇；
 * 次行 = 语言/格式 + 大小 + 下载数 + 时间；页脚 = 上传者 · 评分 +
 * 投票下拉 + 举报/编辑/删除（与桌面同一份 Inner，逻辑不分叉）。
 * 空数据给可见占位（桌面表格有空行，卡片侧用段落，防 sr-only 类事故）。
 */
export function SubtitleCardList({
  rows,
  me,
  canModify,
  onMsg,
  onReload,
  langs,
}: {
  rows: SubtitleRow[];
  me: { id: number; class_id?: number } | null;
  canModify: (s: SubtitleRow) => boolean;
  onMsg: (m: string) => void;
  onReload: () => void;
  langs?: { code: string; name: string; flag: string }[];
}) {
  const { dict, locale } = useI18n();
  const t = dict.subtitles;
  return (
    <ul className="flex flex-col gap-2">
      {rows.map((s) => (
        <li key={s.id} className={CARD}>
          <div className="flex items-start justify-between gap-2">
            <span className="min-w-0 text-sm">
              <span className="subtitles-flag" aria-hidden="true">
                {flagText(s.lang)}
              </span>
              <a href={`/subtitles/${s.id}`} className="ml-1 font-bold">
                {s.title}
              </a>
              {s.verified && (
                <span
                  className="ml-1 text-[11px] font-bold text-mint"
                  title={t.verifiedTip ?? "verified"}
                >
                  ✓
                </span>
              )}
              {s.award_rank === 1 && (
                <span className="ml-1 text-[11px]" title={t.goldTip ?? "gold"}>
                  👑
                </span>
              )}
              {s.ai_state && s.ai_state !== "human" && (
                <span
                  className="ml-1 text-[11px] text-sub"
                  title={
                    s.ai_state === "ai_proofread"
                      ? (t.aiProofreadTip ?? "AI + proofread")
                      : (t.aiTip ?? "AI")
                  }
                >
                  {s.ai_state === "ai_proofread"
                    ? (t.aiBadgeProof ?? "MT✓")
                    : (t.aiBadgePure ?? "MT")}
                </span>
              )}
            </span>
            <a
              href={`/api/v1/subtitles/${s.id}/download`}
              className="shrink-0 px-1 text-base text-sub"
              title={t.downloadTitle ?? "download"}
              target="_blank"
              rel="noreferrer"
              onClick={async (e) => {
                e.preventDefault();
                try {
                  await downloadSubtitle(s);
                  onMsg(t.downloadOk?.replace("{title}", s.title) ?? "ok");
                  onReload();
                } catch (err) {
                  onMsg(err instanceof Error ? err.message : "fail");
                }
              }}
            >
              ⬇
            </a>
          </div>
          <dl className="mt-2 grid grid-cols-2 gap-x-3 gap-y-1 text-xs">
            <div className={META}>
              <dt className="text-sub">{t.colLang}</dt>
              <dd>
                {langLabelOf(s.lang)}
                {s.ext ? ` · ${s.ext}` : ""}
              </dd>
            </div>
            <div className={META}>
              <dt className="text-sub">{t.colSize}</dt>
              <dd className="num">{fmtKB(s.size ?? 0)}</dd>
            </div>
            <div className={META}>
              <dt className="text-sub">{t.colHits}</dt>
              <dd className="num">{s.downloads}</dd>
            </div>
            <div className={META}>
              <dt className="text-sub">{t.colTime}</dt>
              <dd
                title={new Date(s.created_at).toLocaleString(
                  dateLocale(locale),
                )}
              >
                {timeAgo(s.created_at)}
              </dd>
            </div>
          </dl>
          <div className="mt-2 flex items-center justify-between gap-2">
            <span className="truncate text-xs text-sub">
              {s.username ?? t.noAccount}
              {s.cert_tier && (
                <span
                  className={
                    s.cert_tier === "gold"
                      ? "ml-1 font-bold text-amber-500"
                      : "ml-1 font-bold text-mint"
                  }
                  title={
                    s.cert_tier === "gold"
                      ? (t.certGoldTip ?? "gold")
                      : (t.certTip ?? "certified")
                  }
                >
                  {s.cert_tier === "gold" ? "✎★" : "✎"}
                </span>
              )}
              {typeof s.rating === "number" && s.rating > 0 && (
                <span>
                  {" "}
                  · ★ {s.rating}
                  {s.rating_count ? `(${s.rating_count})` : ""}
                </span>
              )}
            </span>
            <span className="flex shrink-0 items-center gap-1">
              {me && (
                <SubtitleVoteSelect
                  s={s}
                  onMsg={onMsg}
                  onReload={onReload}
                />
              )}
              <SubtitleRowActionsInner
                s={s}
                canModify={canModify(s)}
                onMsg={onMsg}
                onReload={onReload}
                langs={langs}
              />
            </span>
          </div>
        </li>
      ))}
      {rows.length === 0 && (
        <li className="py-8 text-center text-sub">{t.empty}</li>
      )}
    </ul>
  );
}
