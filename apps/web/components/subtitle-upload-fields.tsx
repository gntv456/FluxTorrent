"use client";

import { useI18n } from "@/i18n/client";

/** 字幕上传表单字段区（rowhead/rowfollow 经典表格；kind 决定 FPS 显隐/来源/accept）。
 *  从 subtitle-upload-form.tsx 拆出（300 行门禁）：纯受控展示，
 *  状态与提交逻辑留在宿主。 */

export const SUB_EXTS = ".srt,.ass,.ssa,.sup,.idx,.sub,.cue,.zip,.rar,.7z";
export const LYRIC_EXTS = ".lrc,.srt,.ass,.zip";

export interface SubUploadState {
  title: string;
  torrentId: string;
  lang: string;
  file: File | null;
  fps: string;
  source: string;
  author: string;
  anon: boolean;
  ai: boolean;
  aiProof: boolean;
  proofreader: string;
}

export function SubtitleUploadFields({
  kind,
  langs,
  fixedTorrentId,
  s,
  set,
  busy,
}: {
  kind: "subtitle" | "lyric";
  langs: { code: string; name: string; flag: string }[];
  fixedTorrentId?: number;
  s: SubUploadState;
  set: (patch: Partial<SubUploadState>) => void;
  busy: boolean;
}) {
  const { dict } = useI18n();
  const t = dict.subtitles;
  const sourceOptions =
    kind === "lyric" ? (t.lyricSources ?? []) : (t.sources ?? []);
  const accept = kind === "lyric" ? LYRIC_EXTS : SUB_EXTS;
  return (
    <table className="nexus-table subtitles-upload-table">
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
              onChange={(e) => set({ file: e.target.files?.[0] ?? null })}
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
                value={s.torrentId}
                onChange={(e) =>
                  set({ torrentId: e.target.value.replace(/\D/g, "") })
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
              value={s.title}
              onChange={(e) => set({ title: e.target.value })}
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
              value={s.lang}
              onChange={(e) => set({ lang: e.target.value })}
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
                value={s.fps}
                onChange={(e) =>
                  set({ fps: e.target.value.replace(/[^\d.]/g, "") })
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
              value={s.source}
              onChange={(e) => set({ source: e.target.value })}
            >
              <option value="">{t.langSelect}</option>
              {sourceOptions.map((x) => (
                <option key={x} value={x}>
                  {x}
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
              value={s.author}
              onChange={(e) => set({ author: e.target.value })}
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
                checked={s.anon}
                onChange={(e) => set({ anon: e.target.checked })}
              />
              {t.anonNote}
            </label>
          </td>
        </tr>
        <tr>
          <td className="rowhead">{t.aiLabel}</td>
          <td className="rowfollow">
            <label className="flex items-center gap-1">
              <input
                type="checkbox"
                checked={s.ai}
                onChange={(e) => {
                  set({ ai: e.target.checked });
                  if (!e.target.checked) set({ aiProof: false });
                }}
              />
              {t.aiNote}
            </label>
            {s.ai && (
              <>
                <br />
                <label className="flex items-center gap-1">
                  <input
                    type="checkbox"
                    checked={s.aiProof}
                    onChange={(e) => set({ aiProof: e.target.checked })}
                  />
                  {t.aiProofLabel}
                </label>
                {s.aiProof && (
                  <input
                    type="text"
                    className="uc-input-wide mt-1"
                    placeholder={t.proofreaderPh}
                    value={s.proofreader}
                    onChange={(e) => set({ proofreader: e.target.value })}
                  />
                )}
              </>
            )}
          </td>
        </tr>
        <tr>
          <td className="toolbox" colSpan={2} align="center">
            <input
              type="submit"
              className="btn"
              value={t.uploadBtn}
              disabled={busy}
            />
            <input
              type="reset"
              className="btn2"
              value={t.resetBtn}
              onClick={() =>
                set({
                  title: "",
                  torrentId: fixedTorrentId ? String(fixedTorrentId) : "",
                  lang: "",
                  file: null,
                  fps: "",
                  source: "",
                  author: "",
                  anon: false,
                  ai: false,
                  aiProof: false,
                  proofreader: "",
                })
              }
            />
          </td>
        </tr>
      </tbody>
    </table>
  );
}
