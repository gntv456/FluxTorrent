"use client";

import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import type { SubtitleRow } from "@/components/subtitle-board-shared";
import {
  downloadSubtitle,
  flagText,
  fmtKB,
  langLabelOf,
  timeAgo,
} from "@/components/subtitle-board-format";

/** 样式常量（与主字幕区卡片同范式，避免 JSX 行超 80 列门禁） */
const CARD =
  "rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-3";
const META = "flex justify-between gap-2";

/** 种子详情页字幕卡片（M5.6 §13.6 窄屏降级）：
 *  桌面是六列表格（旗标/标题/大小/下载数/上传者/时间），详情页内嵌
 *  实测 390px 下 584px 宽。主行 = 旗标 + 片名（同片/MT 徽标）+ ⬇；
 *  次行 = 语言/格式 + 大小 + 下载数 + 时间；页脚 = 上传者。
 *  此面板无投票/治理操作（与桌面口径一致）；C1 合并的同片字幕注明来源。 */
export function TorrentSubtitleCards({
  rows,
  torrentId,
  onMsg,
}: {
  rows: SubtitleRow[];
  torrentId: number;
  onMsg: (m: string) => void;
}) {
  const { dict, locale } = useI18n();
  const t = dict.subtitles;
  return (
    <ul className="flex flex-col gap-2">
      {rows.map((s) => (
        <li key={s.id} className={CARD}>
          <div className="flex items-start justify-between gap-2">
            <span className="min-w-0 text-sm">
              <span aria-hidden="true">{flagText(s.lang)}</span>
              <a href={`/subtitles/${s.id}`} className="ml-1 font-bold">
                {s.title}
              </a>
              {s.torrent_id !== torrentId && (
                <span
                  className="ml-1 text-[11px] text-sub"
                  title={t.fromSameFilm ?? "same film"}
                >
                  {t.sameFilmBadge ?? "same film"}
                </span>
              )}
              {s.ai_state && s.ai_state !== "human" && (
                <span className="ml-1 text-[11px] text-sub">
                  {s.ai_state === "ai_proofread"
                    ? (t.aiBadgeProof ?? "MT✓")
                    : (t.aiBadgePure ?? "MT")}
                </span>
              )}
            </span>
            <a
              href={`/api/v1/subtitles/${s.id}/download`}
              className="shrink-0 px-1 text-base text-sub"
              target="_blank"
              rel="noreferrer"
              onClick={async (e) => {
                // 本地附件带凭证拉取后触发保存；外部直链跳新页自行下载
                e.preventDefault();
                try {
                  await downloadSubtitle(s);
                } catch (err) {
                  onMsg(err instanceof Error ? err.message : "failed");
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
          <p className="mt-2 truncate text-xs text-sub">
            {s.username ?? (t.anonLabel ?? "anon")}
            {s.cert_tier && (
              <span
                className={
                  s.cert_tier === "gold"
                    ? "ml-1 font-bold text-amber-500"
                    : "ml-1 font-bold text-mint"
                }
              >
                {s.cert_tier === "gold" ? "✎★" : "✎"}
              </span>
            )}
          </p>
        </li>
      ))}
      {rows.length === 0 && (
        <li className="py-4 text-center text-sub">{t.empty}</li>
      )}
    </ul>
  );
}
