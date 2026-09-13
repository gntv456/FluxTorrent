"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

interface SubtitleRow {
  id: number;
  torrent_id: number | null;
  username: string | null;
  title: string;
  lang: string | null;
  downloads: number;
  size: number | null;
  created_at: string;
}

/** NexusPHP 字幕语言表（value 与旧站 sel_lang 一致） */
const LANGS: [string, string][] = [
  ["1", "Bulgarian"], ["2", "Croatian"], ["3", "Czech"], ["4", "Danish"],
  ["5", "Dutch"], ["6", "English"], ["7", "Estonian"], ["8", "Finnish"],
  ["9", "French"], ["10", "German"], ["11", "Greek"], ["12", "Hebrew"],
  ["13", "Hungarian"], ["14", "Italian"], ["15", "日本語"], ["16", "한국어"],
  ["17", "Norwegian"], ["18", "Other"], ["19", "Polish"], ["20", "Portuguese"],
  ["21", "Romanian"], ["22", "Russian"], ["23", "Serbian"], ["24", "Slovak"],
  ["25", "简体中文"], ["26", "Spanish"], ["27", "Swedish"], ["28", "繁體中文"],
  ["29", "Turkish"], ["30", "Slovenian"], ["31", "Thai"],
];
/** lang 存储值（chs/cht/eng…）→ 旧站数字 id 映射 */
const LANG_CODE_TO_ID: Record<string, string> = {
  chs: "25", cht: "28", eng: "6", jpn: "15", kor: "16", other: "18",
};
const LANG_ID_TO_LABEL = (id: string) => LANGS.find(([v]) => v === id)?.[1] ?? id;

const LETTERS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ".split("");

/** 字幕区（包子站 subtitles.php 复刻）：
 *  标题条（字 字幕区 上传字幕-总上传量）→ 规则卡 → 上传表单（rowhead/rowfollow）
 *  → 语言筛选 + 首字母条 → 语言/标题/添加时间/大小/点击/上传者/举报 七列表格 */
export function SubtitleBoard() {
  const { dict } = useI18n();
  const t = dict.subtitles;
  const [rows, setRows] = useState<SubtitleRow[] | null>(null);
  const [search, setSearch] = useState("");
  const [langId, setLangId] = useState("0");
  const [letter, setLetter] = useState("");
  // 上传表单
  const [fTitle, setFTitle] = useState("");
  const [fTorrentId, setFTorrentId] = useState("");
  const [fLang, setFLang] = useState("0");
  const [fFileName, setFFileName] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const params = new URLSearchParams();
      if (search.trim()) params.set("search", search.trim());
      if (langId !== "0") params.set("lang_id", langId);
      if (letter) params.set("letter", letter);
      const qs = params.toString();
      setRows(await api.get<SubtitleRow[]>(`/api/v1/subtitles${qs ? `?${qs}` : ""}`));
    } catch {
      setRows([]);
    }
  }, [search, langId, letter]);

  useEffect(() => {
    load();
  }, [load]);

  const totalSize = useMemo(
    () => (rows ?? []).reduce((acc, r) => acc + (r.size ?? 0), 0),
    [rows],
  );

  async function upload() {
    if (!fTitle.trim() || fLang === "0") return;
    setBusy(true);
    setMsg(null);
    try {
      // lang 存旧站代码（chs/cht/eng…）
      const code = Object.entries(LANG_CODE_TO_ID).find(([, id]) => id === fLang)?.[0] ?? "other";
      await api.post("/api/v1/subtitles", {
        torrent_id: fTorrentId ? Number(fTorrentId) : 0,
        title: fTitle,
        lang: code,
        file_ref: fFileName ? `local://${fFileName}` : undefined,
      });
      setFTitle("");
      setFFileName("");
      setMsg(t.reward);
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="subtitles-wrap">
      {/* 标题条：字 字幕区 | 上传字幕 - 总上传量 */}
      <header className="subtitles-head">
        <span className="subtitles-head__mark" aria-hidden="true">
          字
        </span>
        <strong>{t.title}</strong>
        <small>
          {t.uploadTitle} - {t.totalUploaded} {fmtKB(totalSize)}
        </small>
      </header>

      {/* 规则卡 */}
      <section className="subtitles-rules">
        <h2>{t.rulesTitle}</h2>
        <ul>
          {t.rules.map((r) => (
            <li key={r} dangerouslySetInnerHTML={{ __html: boldRule(r) }} />
          ))}
        </ul>
        <p className="subtitles-required-note">
          {t.requiredNote.replace("*", "*")}
        </p>
      </section>

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
                  onChange={(e) => setFFileName(e.target.files?.[0]?.name ?? "")}
                />
                <br />
                {t.fileNote}
              </td>
            </tr>
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
                  onChange={(e) => setFTorrentId(e.target.value.replace(/\D/g, ""))}
                />
                <br />
                {t.torrentIdNote}
              </td>
            </tr>
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
                {t.titleNote}
              </td>
            </tr>
            <tr>
              <td className="rowhead">
                {t.lang}
                <span className="req-star">*</span>
              </td>
              <td className="rowfollow">
                <select value={fLang} onChange={(e) => setFLang(e.target.value)}>
                  <option value="0">{t.langSelect}</option>
                  {LANGS.map(([v, label]) => (
                    <option key={v} value={v}>
                      {label}
                    </option>
                  ))}
                </select>
              </td>
            </tr>
            <tr>
              <td className="toolbox" colSpan={2} align="center">
                <input type="submit" className="btn" value={t.uploadBtn} disabled={busy} />
                <input
                  type="reset"
                  className="btn2"
                  value={t.resetBtn}
                  onClick={() => {
                    setFTitle("");
                    setFTorrentId("");
                    setFLang("0");
                    setFFileName("");
                  }}
                />
              </td>
            </tr>
          </tbody>
        </table>
        {msg && <p className="subtitles-msg">{msg}</p>}
      </form>

      {/* 搜索：关键词 + 语言下拉 */}
      <form className="subtitles-search-form" onSubmit={(e) => e.preventDefault()}>
        <label htmlFor="subtitles-search">{t.searchLabel}</label>
        <input
          id="subtitles-search"
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <select value={langId} onChange={(e) => setLangId(e.target.value)} aria-label={t.lang}>
          <option value="0">{t.allLangs}</option>
          {LANGS.map(([v, label]) => (
            <option key={v} value={v}>
              {label}
            </option>
          ))}
        </select>
      </form>

      {/* 首字母条 */}
      <nav className="subtitles-letters" aria-label="A-Z">
        {LETTERS.map((l) => (
          <button
            key={l}
            type="button"
            data-active={letter === l ? "true" : undefined}
            onClick={() => setLetter(letter === l ? "" : l)}
          >
            {l}
          </button>
        ))}
      </nav>

      {/* 字幕列表（七列） */}
      <div className="baozi-wide-table-scroll subtitles-table-scroll" role="region">
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
            {(rows ?? []).map((s) => (
              <tr key={s.id}>
                <td className="text-center" title={langLabelOf(s.lang)}>
                  <span className="subtitles-flag" aria-hidden="true">
                    {flagText(s.lang)}
                  </span>
                </td>
                <td>
                  <a href={`/api/v1/subtitles/${s.id}/download`} className="font-bold">
                    {s.title}
                  </a>
                </td>
                <td className="nowrap text-center" title={new Date(s.created_at).toLocaleString("zh-CN")}>
                  {timeAgo(s.created_at)}
                </td>
                <td className="num text-center">{fmtKB(s.size ?? 0)}</td>
                <td className="num text-center">{s.downloads}</td>
                <td className="text-center">
                  <span className="nowrap">{s.username ?? t.noAccount}</span>
                </td>
                <td className="text-center">
                  <button
                    type="button"
                    className="subtitles-report"
                    title={t.reportTitle}
                    aria-label={t.reportTitle}
                    onClick={async () => {
                      const reason = window.prompt(t.reportTitle);
                      if (!reason?.trim()) return;
                      try {
                        await api.post("/api/v1/reports", {
                          ref_type: "subtitle",
                          ref_id: s.id,
                          reason: reason.trim(),
                        });
                        setMsg(t.reportOk ?? "举报已提交，感谢反馈");
                      } catch (e) {
                        setMsg(e instanceof Error ? e.message : (t.reportFail ?? "举报失败"));
                      }
                    }}
                  >
                    ⚑
                  </button>
                </td>
              </tr>
            ))}
            {rows !== null && rows.length === 0 && (
              <tr>
                <td colSpan={7} className="py-8 text-center text-sub">
                  {t.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      <p className="text-xs text-sub">1 - {rows?.length ?? 0}</p>
    </div>
  );
}

function langLabelOf(lang: string | null): string {
  if (!lang) return "Other";
  return LANG_ID_TO_LABEL(LANG_CODE_TO_ID[lang] ?? "18");
}
function flagText(lang: string | null): string {
  const label = langLabelOf(lang);
  if (label === "简体中文") return "🇨🇳";
  if (label === "繁體中文") return "🇹🇼";
  if (label === "English") return "🇬🇧";
  if (label === "日本語") return "🇯🇵";
  if (label === "한국어") return "🇰🇷";
  return "🌐";
}
function fmtKB(bytes: number): string {
  if (!bytes) return "—";
  const kb = bytes / 1024;
  return `${kb.toFixed(2)} KB`;
}
function timeAgo(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const m = Math.floor(diff / 60000);
  if (m < 60) return `${m}分钟`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}时${m % 60}分`;
  const d = Math.floor(h / 24);
  if (d < 30) return `${d}天${h % 24}时`;
  const mo = Math.floor(d / 30);
  return `${mo}月${d % 30}天`;
}
/** 规则文本加粗关键部分（同步/标题/合集/Vobsub/proper） */
function boldRule(r: string): string {
  return r
    .replace("字幕必须与视频文件同步", "<b>字幕必须与视频文件同步</b>")
    .replace("标题", "<b>标题</b>")
    .replace(/\&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/\*&lt;/g, "*<");
}
