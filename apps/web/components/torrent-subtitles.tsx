"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
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

/** 种子详情页字幕面板（0146 P0-6：最大流量入口）。
 *  列该种子字幕（语言/标题/大小/点击/上传者/时间）+ 快捷上传（带 torrent_id）。
 *  空态「暂无字幕」；面板始终渲染（模块开关已由页面 requireModule 与 API 网关把守）。 */

export function TorrentSubtitles({ torrentId }: { torrentId: number }) {
  const { dict } = useI18n();
  const t = dict.subtitles;
  const [rows, setRows] = useState<SubtitleRow[] | null>(null);
  const [open, setOpen] = useState(false);
  const [fTitle, setFTitle] = useState("");
  const [fLang, setFLang] = useState("");
  const [fFile, setFFile] = useState<File | null>(null);
  const [fAnon, setFAnon] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const resp = await api.get<SubtitleListResp | SubtitleRow[]>(
        `/api/v1/subtitles?torrent_id=${torrentId}&per_page=100`,
      );
      setRows(Array.isArray(resp) ? resp : resp.items);
    } catch {
      setRows([]);
    }
  }, [torrentId]);

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
      {rows === null ? (
        <p className="py-4 text-center text-sub">…</p>
      ) : (
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
                </td>
                <td className="num text-center">{fmtKB(s.size ?? 0)}</td>
                <td className="num text-center">{s.downloads}</td>
                <td className="text-center text-sub">
                  {s.username ?? (t.anonLabel ?? "anon")}
                </td>
                <td
                  className="nowrap text-center text-sub"
                  title={new Date(s.created_at).toLocaleString("zh-CN")}
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
  const [langs, setLangs] = useState<
    { code: string; name: string; flag: string }[]
  >([
    { code: "chs", name: "简体中文", flag: "🇨🇳" },
    { code: "cht", name: "繁體中文", flag: "🇹🇼" },
    { code: "eng", name: "English", flag: "🇬🇧" },
    { code: "jpn", name: "日本語", flag: "🇯🇵" },
    { code: "kor", name: "한국어", flag: "🇰🇷" },
    { code: "other", name: "其他", flag: "🌐" },
  ]);
  useEffect(() => {
    import("@/components/subtitle-board-table")
      .then((m) => m.loadLangDict())
      .then((d) => {
        if (d.length) setLangs(d);
      })
      .catch(() => {});
  }, []);
  const { dict: langDict } = useI18n();
  const s = dict_safe(langDict.subtitles);
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
