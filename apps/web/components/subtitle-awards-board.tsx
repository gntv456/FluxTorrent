"use client";

/** 评选榜（0148 C5）：金字字幕各期 Top 榜（subtitle_award=1 才挂载）。
 *  从 subtitle-request-panel.tsx 按域拆出（300 行门禁）。 */

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { useIsCompact } from "@/lib/hooks/use-media";
import type { SubtitleAwardRow } from "@/components/subtitle-board-shared";

/** 卡片样式常量（M5.6 §13.6 窄屏降级用，避免 JSX 行超 80 列门禁） */
const CARD =
  "rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-3";

/** 评选榜（0148 C5）：金字字幕各期 Top 榜（subtitle_award=1 才挂载） */
export function SubtitleAwardsBoard() {
  const { dict } = useI18n();
  const t = dict.subtitles;
  const [rows, setRows] = useState<SubtitleAwardRow[] | null>(null);
  // 评选开关（0148 C5 / 二审 G2-6 修复）：subtitle_award=0 时整个榜单不挂载，
  // 也不发起请求——关闭评选的站不再出现空榜占位。
  const [awardOn, setAwardOn] = useState(true);
  const compact = useIsCompact();
  useEffect(() => {
    fetch("/api/v1/site-profile")
      .then((r) => r.json())
      .then((b: { data?: { subtitle_award?: boolean } }) =>
        setAwardOn(b?.data?.subtitle_award !== false),
      )
      .catch(() => setAwardOn(true));
  }, []);
  useEffect(() => {
    if (!awardOn) return;
    api
      .get<SubtitleAwardRow[] | { items: SubtitleAwardRow[] }>(
        "/api/v1/subtitles/awards",
      )
      .then((r) => {
        const list = Array.isArray(r)
          ? r
          : ((r as { items?: SubtitleAwardRow[] }).items ?? []);
        setRows(list);
      })
      .catch(() => setRows([]));
  }, [awardOn]);
  if (!awardOn) return null;
  if (rows === null) return <p className="py-2 text-center text-sub">…</p>;
  const won = rows.filter((r) => r.rank > 0);
  if (!won.length) {
    return (
      <p className="py-2 text-center text-sub">{t.awardEmpty ?? "empty"}</p>
    );
  }
  const medals = ["👑", "🥈", "🥉"];
  if (compact) {
    // 窄屏卡片：主行 = 奖牌 + 片名（保持点标题即下载）；次行 = 上传者 ·
    // 层级 · 期数。纯展示无操作列。
    return (
      <ul className="flex flex-col gap-2">
        {won.slice(0, 30).map((r) => (
          <li key={r.id} className={CARD}>
            <div className="flex items-start justify-between gap-2">
              <span className="min-w-0 text-sm">
                <span aria-hidden="true">{medals[r.rank - 1] ?? "•"}</span>
                <a
                  href={`/api/v1/subtitles/${r.subtitle_id}/download`}
                  className="ml-1 font-bold"
                  target="_blank"
                  rel="noreferrer"
                >
                  {r.title}
                </a>
              </span>
              <span className="shrink-0 text-xs text-sub">{r.period}</span>
            </div>
            <p className="mt-1 truncate text-xs text-sub">
              {r.username ?? "—"} ·{" "}
              {r.tier === "ai" ? (t.tierAI ?? "AI") : (t.tierHuman ?? "human")}
            </p>
          </li>
        ))}
      </ul>
    );
  }
  return (
    <div className="baozi-wide-table-scroll">
    <table className="nexus-table subtitles-list-table">
      <tbody>
        {won
          .slice(0, 30)
          .map((r) => (
            <tr key={r.id}>
              <td className="text-center">{medals[r.rank - 1] ?? "•"}</td>
              <td>
                <a
                  href={`/api/v1/subtitles/${r.subtitle_id}/download`}
                  className="font-bold"
                  target="_blank"
                  rel="noreferrer"
                >
                  {r.title}
                </a>
              </td>
              <td className="text-sub">{r.username ?? "—"}</td>
              <td className="text-center text-sub">
                {r.tier === "ai"
                  ? (t.tierAI ?? "AI")
                  : (t.tierHuman ?? "human")}
              </td>
              <td className="num text-center text-sub">{r.period}</td>
            </tr>
          ))}
      </tbody>
    </table>
    </div>
  );
}
