"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import type {
  SubtitleListResp,
  SubtitleRow,
} from "@/components/subtitle-board-shared";
import {
  flagText,
  fmtKB,
  langLabelOf,
  timeAgo,
} from "@/components/subtitle-board-table";
import { uploadAttachment } from "@/components/subtitle-request-panel";

/** 种子详情页字幕面板（0146 P0-6 → 0148 C1 IMDB 合并）。
 *  两组：「本种子字幕」（torrent_id 命中）+「本片字幕」（同 imdb_id 的
 *  其他版本，C1 自动合并）；快捷上传（带 torrent_id）。
 *  空态「暂无字幕」；面板始终渲染（模块开关已由页面 requireModule 与 API 网关把守）。 */

export function TorrentSubtitles({
  torrentId,
  imdbId,
}: {
  torrentId: number;
  imdbId?: string | null;
}) {
  const { dict, locale } = useI18n();
  const t = dict.subtitles;
  const [rows, setRows] = useState<SubtitleRow[] | null>(null);
  const [open, setOpen] = useState(false);
  const [fTitle, setFTitle] = useState("");
  const [fLang, setFLang] = useState("");
  const [fFile, setFFile] = useState<File | null>(null);
  const [fAnon, setFAnon] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  /** 拉取失败（与「确实没有字幕」区分开，见 load 内注释） */
  const [err, setErr] = useState(false);

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

  async function upload() {
    if (!fTitle.trim() || !fLang || !fFile) return;
    setBusy(true);
    setMsg(null);
    try {
      const sha = await uploadAttachment(fFile);
      await api.post("/api/v1/subtitles", {
        torrent_id: torrentId,
        title: fTitle,
        lang: fLang,
        file_sha: sha,
        anon: fAnon,
      });
      setFTitle("");
      setFFile(null);
      setMsg(t.uploadDone);
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="nexus-detail td-subtitles">
      <h2 className="td-sec-title">
        {t.panelTitle ?? "Subtitles"}
        {rows !== null && rows.length > 0 ? ` (${rows.length})` : ""}
      </h2>
      {err ? (
        <p className="py-4 text-center text-sub" role="status">
          {dict.common.loadFailed}
          <button type="button" className="ml-2 underline" onClick={load}>
            {dict.common.retry}
          </button>
        </p>
      ) : rows === null ? (
        <p className="py-4 text-center text-sub">…</p>
      ) : (
        <div className="baozi-wide-table-scroll">
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
                        const buf = await api.getBlob(
                          `/api/v1/subtitles/${s.id}/download`,
                        );
                        const ext = s.ext || "srt";
                        const blob = new Blob([buf], {
                          type: "application/octet-stream",
                        });
                        const url = URL.createObjectURL(blob);
                        const a = document.createElement("a");
                        a.href = url;
                        const safe =
                          s.title.replace(/[\\/:*?"<>|]/g, "_");
                        a.download = `${safe}.${ext}`;
                        a.click();
                        URL.revokeObjectURL(url);
                      } catch (err) {
                        setMsg(
                          err instanceof Error ? err.message : "failed",
                        );
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
                          ? "ml-1 text-[11px] font-bold text-amber-500"
                          : "ml-1 text-[11px] font-bold text-mint"
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
                </td>
              </tr>
            )}
          </tbody>
        </table>
        </div>
      )}
      <div className="mt-2">
        {!open ? (
          <button
            type="button"
            className="btn2"
            onClick={() => setOpen(true)}
          >
            {t.uploadTitle}
          </button>
        ) : (
          <form
            className="flex flex-wrap items-center gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              void upload();
            }}
          >
            <input
              type="file"
              accept=".srt,.ass,.ssa,.sup,.idx,.sub,.cue,.zip,.rar,.7z,.lrc"
              onChange={(e) => setFFile(e.target.files?.[0] ?? null)}
              aria-label={t.file}
            />
            <input
              type="text"
              className="w-40"
              placeholder={t.titleLabel}
              value={fTitle}
              onChange={(e) => setFTitle(e.target.value)}
            />
            <LangSelect value={fLang} onChange={setFLang} />
            <label className="flex items-center gap-1 text-sm">
              <input
                type="checkbox"
                checked={fAnon}
                onChange={(e) => setFAnon(e.target.checked)}
              />
              {t.anonLabel ?? "Anonymous"}
            </label>
            <button type="submit" className="btn" disabled={busy}>
              {t.uploadBtn}
            </button>
            <button
              type="button"
              className="btn2"
              onClick={() => setOpen(false)}
            >
              {dict.common.cancel}
            </button>
          </form>
        )}
        {msg && <p className="mt-1 text-sm">{msg}</p>}
      </div>
    </section>
  );
}

/** 面板内语言选择（读字典；失败回落常见六语） */
function LangSelect({
  value,
  onChange,
}: {
  value: string;
  onChange: (v: string) => void;
}) {
  const { dict: langDict } = useI18n();
  const t = langDict.subtitles;
  const s = dict_safe(t);
  const [langs, setLangs] = useState<
    { code: string; name: string; flag: string }[]
  >([
    { code: "chs", name: t.langChs, flag: "🇨🇳" },
    { code: "cht", name: t.langCht, flag: "🇹🇼" },
    { code: "eng", name: t.langEng, flag: "🇬🇧" },
    { code: "jpn", name: t.langJpn, flag: "🇯🇵" },
    { code: "kor", name: t.langKor, flag: "🇰🇷" },
    { code: "other", name: t.langOther, flag: "🌐" },
  ]);
  useEffect(() => {
    import("@/components/subtitle-board-table")
      .then((m) => m.loadLangDict())
      .then((d) => {
        if (d.length) setLangs(d);
      })
      .catch(() => {});
  }, []);
  return (
    <select value={value} onChange={(e) => onChange(e.target.value)}>
      <option value="">{s}</option>
      {langs.map((l) => (
        <option key={l.code} value={l.code}>
          {l.flag} {l.name}
        </option>
      ))}
    </select>
  );
}
function dict_safe(t: { langSelect?: string }): string {
  return t.langSelect ?? "(select)";
}
