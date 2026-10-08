"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import type { SubtitleRow } from "@/components/subtitle-board-shared";
import {
  SubtitleRowActions,
  SubtitleVoteSelect,
} from "@/components/subtitle-row-actions";
import {
  flagText,
  fmtKB,
  langLabelOf,
  loadLangDict,
  timeAgo,
} from "@/components/subtitle-board-format";
import { isStaffClass } from "@/lib/domain/user-class";

export {
  boldRule,
  flagText,
  fmtKB,
  langLabelOf,
  langOptions,
  loadLangDict,
  timeAgo,
  LETTERS,
} from "@/components/subtitle-board-format";

/** 字幕列表子面板（从 components/subtitle-board.tsx 按域拆出）：
 *  七列表格（语言/标题/添加时间/大小/点击/上传者/举报）+ 语言映射与
 *  展示工具（旗帜/KB/相对时间/规则加粗）。主组件与筛选/上传留在原文件。
 *  0146：语言读字典接口、大小真实值、评分列、分页条、坏字幕标记。 */

/** 字幕列表（七列）：本地附件走 BLOB 下载，外部直链跳新页；举报走 prompt。
 *  0147 行内治理：评分（1-10 下拉）、本人/管理编辑（标题/语言/匿名）、删除。 */
export function SubtitleListTable({
  rows,
  onMsg,
  onReload,
  langs,
}: {
  rows: SubtitleRow[];
  onMsg: (m: string) => void;
  onReload: () => void;
  langs?: { code: string; name: string; flag: string }[];
}) {
  const { dict, locale } = useI18n();
  const t = dict.subtitles;
  const [me, setMe] = useState<{
    id: number;
    class_id?: number;
  } | null>(null);
  useEffect(() => {
    api
      .get<{ id: number; class_id?: number }>("/api/v1/me")
      .then(setMe)
      .catch(() => setMe(null));
  }, []);
  const canModify = (s: SubtitleRow) =>
    !!me && (me.id === s.user_id || isStaffClass(me.class_id));
  return (
    <div
      className="baozi-wide-table-scroll subtitles-table-scroll"
      role="region"
    >
      <table className="nexus-table subtitles-list-table">
        <tbody>
          <tr>
            <td className="colhead">{t.colLang}</td>
            <td className="colhead text-center">{t.colTitle}</td>
            <td className="colhead text-center">{t.colTime}</td>
            <td className="colhead text-center">{t.colSize}</td>
            <td className="colhead text-center">{t.colHits}</td>
            <td className="colhead text-center">{t.colUploader}</td>
            <td className="colhead text-center">{t.colReport}</td>
          </tr>
          {rows.map((s) => (
            <tr key={s.id}>
              <td className="text-center" title={langLabelOf(s.lang)}>
                <span className="subtitles-flag" aria-hidden="true">
                  {flagText(s.lang)}
                </span>
              </td>
              <td>
                <a
                  href={`/subtitles/${s.id}`}
                  className="font-bold"
                  onClick={(e) => {
                    // 修饰键（新窗/命令）放行默认导航；普通点击由 Next 接管
                    if (e.metaKey || e.ctrlKey || e.shiftKey) return;
                  }}
                >
                  {s.title}
                </a>
                <a
                  href={`/api/v1/subtitles/${s.id}/download`}
                  className="ml-1.5 text-xs text-sub"
                  title={t.downloadTitle ?? "download"}
                  target="_blank"
                  rel="noreferrer"
                  onClick={async (e) => {
                    // 本地附件端点直接回文件字节：带凭证拉取后触发保存；
                    // 外部直链后端回 JSON，跳新页由其自行下载
                    e.preventDefault();
                    try {
                      const buf = await api.getBlob(
                        `/api/v1/subtitles/${s.id}/download`,
                      );
                      const ext = s.ext || "srt";
                      const safe =
                        s.title.replace(/[\\/:*?"<>|]/g, "_");
                      const name = `${safe}.${ext}`;
                      const blob = new Blob([buf], {
                        type: "application/octet-stream",
                      });
                      const url = URL.createObjectURL(blob);
                      const a = document.createElement("a");
                      a.href = url;
                      a.download = name;
                      a.click();
                      URL.revokeObjectURL(url);
                      onMsg(t.downloadOk?.replace("{title}", s.title));
                      onReload();
                    } catch (err) {
                      onMsg(err instanceof Error ? err.message : "fail");
                    }
                  }}
                >
                  ⬇
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
                  <span
                    className="ml-1 text-[11px]"
                    title={t.goldTip ?? "gold"}
                  >
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
                {typeof s.rating === "number" && s.rating > 0 && (
                  <span className="ml-1 text-[11px] text-sub">
                    ★ {s.rating}
                    {s.rating_count ? `(${s.rating_count})` : ""}
                  </span>
                )}
                {me && (
                  <SubtitleVoteSelect
                    s={s}
                    onMsg={onMsg}
                    onReload={onReload}
                  />
                )}
              </td>
              <td
                className="nowrap text-center"
                title={new Date(s.created_at).toLocaleString(dateLocale(locale))}
              >
                {timeAgo(s.created_at)}
              </td>
              <td className="num text-center">{fmtKB(s.size ?? 0)}</td>
              <td className="num text-center">{s.downloads}</td>
              <td className="text-center">
                <span className="nowrap">
                  {s.username ?? t.noAccount}
                  {s.cert_tier && (
                    <span
                      className={
                        s.cert_tier === "gold"
                          ? "ml-1 text-[11px] font-bold text-amber-500"
                          : "ml-1 text-[11px] font-bold text-mint"
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
                </span>
              </td>
              <SubtitleRowActions
                s={s}
                canModify={canModify(s)}
                onMsg={onMsg}
                onReload={onReload}
                langs={langs}
              />
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td colSpan={7} className="py-8 text-center text-sub">
                {t.empty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
