"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import type {
  SubtitleListResp,
  SubtitleRow,
} from "@/components/subtitle-board-shared";
import {
  downloadSubtitle,
  flagText,
  fmtKB,
  langLabelOf,
  timeAgo,
} from "@/components/subtitle-board-format";
import { SubtitleUploadDialog } from "@/components/subtitle-upload-dialog";
import { TorrentSubtitleCards } from "@/components/torrent-subtitle-cards";
import { useIsCompact } from "@/lib/hooks/use-media";

/** 种子详情页字幕面板（0146 P0-6 → 0148 C1 IMDB 合并）。
 *  两组：「本种子字幕」（torrent_id 命中）+「本片字幕」（同 imdb_id 的
 *  其他版本，C1 自动合并）。
 *  空态「暂无字幕」；面板始终渲染（模块开关已由页面 requireModule 与
 *  API 网关把守）。
 *
 *  2026-11-13 发布改弹窗（标签组省高改版续）：标题行右侧「上传字幕」
 *  按钮弹 SubtitleUploadDialog 发布（表单/校验/busy 在弹窗件内，
 *  300 行门禁拆件）；此前内联展开会把表格顶下去。 */

export function TorrentSubtitles({
  torrentId,
  imdbId,
  bare,
}: {
  torrentId: number;
  imdbId?: string | null;
  /** tab panel / 折叠内嵌：标题行自持（head 共用），不再依赖外部 h2 */
  bare?: boolean;
}) {
  const { dict, locale } = useI18n();
  const t = dict.subtitles;
  const [rows, setRows] = useState<SubtitleRow[] | null>(null);
  const [open, setOpen] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  /** 拉取失败（与「确实没有字幕」区分开，见 load 内注释） */
  const [err, setErr] = useState(false);
  // M5.6 §13.6：窄屏（<640）字幕表降级为卡片列表（见 torrent-subtitle-cards）
  const compact = useIsCompact();
  const load = useCallback(async () => {
    // 三态（方案 P0-6）：此前失败与「确实没有字幕」都落到空表，用户无法区分是接口挂了还是本来没有
    setErr(false);
    try {
      const params = new URLSearchParams({
        torrent_id: String(torrentId),
        per_page: "100",
      });
      if (imdbId) params.set("imdb", imdbId);
      const resp = await api.get<SubtitleListResp | SubtitleRow[]>(
        `/api/v1/subtitles?${params.toString()}`,
      );
      const all = Array.isArray(resp) ? resp : resp.items;
      // imdb 合并查询会把同片其他版本一并带回：本种子 = torrent_id 命中
      setRows(all);
    } catch {
      setErr(true);
      setRows([]);
    }
  }, [torrentId, imdbId]);

  useEffect(() => {
    load();
  }, [load]);

  /** 标题行（bare 与 section 共用）：标题 + 计数 + 右挂上传按钮 */
  const head = (
    <div className="td-sub-head">
      <b className="td-sub-head__title">
        {t.panelTitle ?? "Subtitles"}
        {rows !== null && rows.length > 0 ? (
          <span className="num td-sub-head__count">{rows.length}</span>
        ) : null}
      </b>
      <button
        type="button"
        className="btn2 td-sub-head__btn"
        onClick={() => setOpen(true)}
      >
        {t.uploadTitle}
      </button>
    </div>
  );

  const bodyInner = (
    <>
      {err ? (
        <p className="py-4 text-center text-sub" role="status">
          {dict.common.loadFailed}
          <button type="button" className="ml-2 underline" onClick={load}>
            {dict.common.retry}
          </button>
        </p>
      ) : rows === null ? (
        <p className="py-4 text-center text-sub">…</p>
      ) : compact ? (
        <TorrentSubtitleCards
          rows={rows}
          torrentId={torrentId}
          onMsg={setMsg}
        />
      ) : (
        <div className="brand-wide-table-scroll">
        <table className="nexus-table subtitles-list-table">
          <tbody>
            {rows.map((s) => (
              <tr key={s.id}>
                <td className="text-center" title={langLabelOf(s.lang)}>
                  <span aria-hidden="true">{flagText(s.lang)}</span>
                </td>
                <td>
                  <a
                    href={`/api/v1/subtitles/${s.id}/download`}
                    target="_blank"
                    rel="noreferrer"
                    onClick={async (e) => {
                      e.preventDefault();
                      try {
                        await downloadSubtitle(s);
                      } catch (err) {
                        setMsg(apiErrorMessage(dict, err));
                      }
                    }}
                  >
                    {s.title}
                  </a>
                  {/* C1 合并标记：非本种子的同片字幕注明来源 */}
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
                </td>
                <td className="num text-center">{fmtKB(s.size ?? 0)}</td>
                <td className="num text-center">{s.downloads}</td>
                <td className="text-center text-sub">
                  {s.username ?? (t.anonLabel ?? "anon")}
                  {s.cert_tier && (
                    <span
                      className={
                        s.cert_tier === "gold"
                          ? "ml-1 text-[11px] font-bold [color:var(--sun)]"
                          : "ml-1 text-[11px] font-bold [color:var(--mint)]"
                      }
                    >
                      {s.cert_tier === "gold" ? "✎★" : "✎"}
                    </span>
                  )}
                </td>
                <td
                  className="nowrap text-center text-sub"
                  title={new Date(s.created_at).toLocaleString(dateLocale(locale))}
                >
                  {timeAgo(s.created_at)}
                </td>
              </tr>
            ))}
            {rows.length === 0 && (
              <tr>
                <td colSpan={6} className="py-4 text-center text-sub">
                  {t.empty}
                  {/* 0424 同类站对标：空态外链搜索（IMDb ID 有值时给
                      OpenSubtitles；片名搜索给 assrt） */}
                  <a
                    className="ml-2 underline"
                    target="_blank"
                    rel="noreferrer noopener"
                    href={
                      imdbId
                        ? `https://www.opensubtitles.com/search/imdb-${imdbId.replace(/^tt/, "")}`
                        : "https://assrt.net/subtitle-search/"
                    }
                  >
                    {t.searchExternal}
                  </a>
                </td>
              </tr>
            )}
          </tbody>
        </table>
        </div>
      )}
      {/* 发布弹窗（2026-11-13）：表单/校验/busy 全在 SubtitleUploadDialog
          （300 行门禁拆件）；成功后 onClose 收窗 + load 刷新列表 */}
      <SubtitleUploadDialog
        torrentId={torrentId}
        open={open}
        onClose={() => setOpen(false)}
        onDone={load}
      />
      {msg && (
        <p className="mt-1 text-sm" role="status">
          {msg}
        </p>
      )}
    </>
  );
  if (bare) {
    return (
      <>
        {head}
        {bodyInner}
      </>
    );
  }
  return (
    <section className="nexus-detail td-subtitles">
      {/* head 自带标题排版（td-sub-head）；不再套 h2——按钮/计数不是
          标题内容，进了大纲反而打乱读屏器 */}
      {head}
      {bodyInner}
    </section>
  );
}
