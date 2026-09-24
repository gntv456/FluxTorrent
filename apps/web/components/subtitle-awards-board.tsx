"use client";

/** 评选榜（0148 C5）：金字字幕各期 Top 榜（subtitle_award=1 才挂载）。
 *  从 subtitle-request-panel.tsx 按域拆出（300 行门禁）。 */

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { SubtitleAwardRow } from "@/components/subtitle-board-shared";

/** 评选榜（0148 C5）：金字字幕各期 Top 榜（subtitle_award=1 才挂载） */
export function SubtitleAwardsBoard() {
  const { dict } = useI18n();
  const t = dict.subtitles;
  const [rows, setRows] = useState<SubtitleAwardRow[] | null>(null);
  // 评选开关（0148 C5 / 二审 G2-6 修复）：subtitle_award=0 时整个榜单不挂载，
  // 也不发起请求——关闭评选的站不再出现空榜占位。
  const [awardOn, setAwardOn] = useState(true);
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
  return (
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
  );
}
