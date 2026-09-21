"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { uploadAttachment } from "@/components/subtitle-request-panel";

/** 字幕上传表单（rowhead/rowfollow 经典表格；kind 决定 FPS 显隐/来源/accept）。
 *  从 subtitle-board.tsx 按域拆出（300 行门禁）。 */

const SUB_EXTS = ".srt,.ass,.ssa,.sup,.idx,.sub,.cue,.zip,.rar,.7z";
const LYRIC_EXTS = ".lrc,.srt,.ass,.zip";

export function SubtitleUploadForm({
  kind,
  langs,
  fixedTorrentId,
  onMsg,
  onUploaded,
}: {
  kind: "subtitle" | "lyric";
  langs: { code: string; name: string; flag: string }[];
  fixedTorrentId?: number;
  onMsg: (m: string) => void;
  onUploaded: () => void;
}) {
  const { dict, currency } = useI18n();
  const t = dict.subtitles;
  const [fTitle, setFTitle] = useState("");
  const [fTorrentId, setFTorrentId] = useState(
    fixedTorrentId ? String(fixedTorrentId) : "",
  );
  const [fLang, setFLang] = useState("");
  const [fFile, setFFile] = useState<File | null>(null);
  const [fFps, setFFps] = useState("");
  const [fSource, setFSource] = useState("");
  const [fAuthor, setFAuthor] = useState("");
  const [fAnon, setFAnon] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [busyLocal, setBusyLocal] = useState(false);
  const sourceOptions =
    kind === "lyric" ? (t.lyricSources ?? []) : (t.sources ?? []);
  const accept = kind === "lyric" ? LYRIC_EXTS : SUB_EXTS;

  async function upload() {
    if (!fTitle.trim() || !fLang || !fFile) return;
    if (fFile.size > 8 * 1024 * 1024) {
      setMsg(t.tooLarge ?? "file too large");
      return;
    }
    setBusyLocal(true);
    setMsg(null);
    try {
      const sha = await uploadAttachment(fFile);
      await api.post("/api/v1/subtitles", {
        torrent_id: fTorrentId ? Number(fTorrentId) : 0,
        title: fTitle,
        lang: fLang,
        file_sha: sha,
        fps: fFps ? Number(fFps) : undefined,
        source: fSource || undefined,
        author_name: fAuthor || undefined,
        anon: fAnon,
      });
      setFTitle("");
      setFFile(null);
      setMsg(t.reward.replace("{magic}", currency));
      onUploaded();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusyLocal(false);
    }
  }

  return (
    <>
      {/* 上传表单（rowhead/rowfollow 经典表格） */}
      <form
        className="subtitles-upload-form"
        onSubmit={(e) => {
          e.preventDefault();
          void upload();
        }}
      >
        <table className="nexus-table nexus-form subtitles-upload-table">
          <tbody>
            <tr>
              <td className="rowhead">
                {t.file}
                <span className="req-star">*</span>
              </td>
              <td className="rowfollow">
                <input
                  type="file"
                  aria-label={t.file}
                  accept={accept}
                  onChange={(e) => setFFile(e.target.files?.[0] ?? null)}
                />
                <br />
                {kind === "lyric"
                  ? (t.lyricFileNote ?? t.fileNote)
                  : t.fileNote}
              </td>
            </tr>
            {!fixedTorrentId && (
              <tr>
                <td className="rowhead">
                  {t.torrentId}
                  <span className="req-star">*</span>
                </td>
                <td className="rowfollow">
                  <input
                    type="text"
                    className="uc-input-wide"
                    value={fTorrentId}
                    onChange={(e) =>
                      setFTorrentId(e.target.value.replace(/\D/g, ""))
                    }
                  />
                  <br />
                  {t.torrentIdNote}
                </td>
              </tr>
            )}
            <tr>
              <td className="rowhead">{t.titleLabel}</td>
              <td className="rowfollow">
                <input
                  type="text"
                  className="uc-input-wide"
                  value={fTitle}
                  onChange={(e) => setFTitle(e.target.value)}
                />
                <br />
                {kind === "lyric"
                  ? (t.lyricTitleNote ?? t.titleNote)
                  : t.titleNote}
              </td>
            </tr>
            <tr>
              <td className="rowhead">
                {t.lang}
                <span className="req-star">*</span>
              </td>
              <td className="rowfollow">
                <select
                  value={fLang}
                  onChange={(e) => setFLang(e.target.value)}
                >
                  <option value="">{t.langSelect}</option>
                  {langs.map((l) => (
                    <option key={l.code} value={l.code}>
                      {l.flag} {l.name}
                    </option>
                  ))}
                </select>
              </td>
            </tr>
            {kind === "subtitle" && (
              <tr>
                <td className="rowhead">{t.fpsLabel}</td>
                <td className="rowfollow">
                  <input
                    type="text"
                    className="uc-input-wide"
                    value={fFps}
                    onChange={(e) =>
                      setFFps(e.target.value.replace(/[^\d.]/g, ""))
                    }
                    placeholder="23.976"
                  />
                </td>
              </tr>
            )}
            <tr>
              <td className="rowhead">{t.sourceLabel}</td>
              <td className="rowfollow">
                <select
                  value={fSource}
                  onChange={(e) => setFSource(e.target.value)}
                >
                  <option value="">{t.langSelect}</option>
                  {sourceOptions.map((s) => (
                    <option key={s} value={s}>
                      {s}
                    </option>
                  ))}
                </select>
              </td>
            </tr>
            <tr>
              <td className="rowhead">{t.authorLabel}</td>
              <td className="rowfollow">
                <input
                  type="text"
                  className="uc-input-wide"
                  value={fAuthor}
                  onChange={(e) => setFAuthor(e.target.value)}
                />
                <br />
                {t.authorNote}
              </td>
            </tr>
            <tr>
              <td className="rowhead">{t.anonLabel}</td>
              <td className="rowfollow">
                <label className="flex items-center gap-1">
                  <input
                    type="checkbox"
                    checked={fAnon}
                    onChange={(e) => setFAnon(e.target.checked)}
                  />
                  {t.anonNote}
                </label>
              </td>
            </tr>
            <tr>
              <td className="toolbox" colSpan={2} align="center">
                <input
                  type="submit"
                  className="btn"
                  value={t.uploadBtn}
                  disabled={busyLocal}
                />
                <input
                  type="reset"
                  className="btn2"
                  value={t.resetBtn}
                  onClick={() => {
                    setFTitle("");
                    setFTorrentId(
                      fixedTorrentId ? String(fixedTorrentId) : "",
                    );
                    setFLang("");
                    setFFile(null);
                    setFFps("");
                    setFSource("");
                    setFAuthor("");
                    setFAnon(false);
                  }}
                />
              </td>
            </tr>
          </tbody>
        </table>
        {msg && <p className="subtitles-msg">{msg}</p>}
      </form>
    </>
  );
}
