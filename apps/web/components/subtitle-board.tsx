"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { api, ApiError, rawFetchHelpers } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { SubtitleRow } from "@/components/subtitle-board-shared";
import {
  LANGS,
  LANG_CODE_TO_ID,
  LETTERS,
  fmtKB,
  boldRule,
  SubtitleListTable,
} from "@/components/subtitle-board-table";

// 字幕区（参考站 subtitles.php 复刻）：
// 标题条（字 字幕区 上传字幕-总上传量）→ 规则卡 → 上传表单（rowhead/rowfollow）
// → 语言筛选 + 首字母条 → 语言/标题/添加时间/大小/点击/上传者/举报 七列表格。
// 语言映射/展示工具/七列列表已按域拆出 @/components/subtitle-board-table。

export function SubtitleBoard() {
  const { dict, currency } = useI18n();
  const t = dict.subtitles;
  const [rows, setRows] = useState<SubtitleRow[] | null>(null);
  const [search, setSearch] = useState("");
  const [langId, setLangId] = useState("0");
  const [letter, setLetter] = useState("");
  // 上传表单
  const [fTitle, setFTitle] = useState("");
  const [fTorrentId, setFTorrentId] = useState("");
  const [fLang, setFLang] = useState("0");
  const [fFile, setFFile] = useState<File | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const params = new URLSearchParams();
      if (search.trim()) params.set("search", search.trim());
      // 后端存的是语言代码（chs/cht/eng…），数字 id 是旧站展示口径：提交前转换
      if (langId !== "0") {
        const code = Object.entries(LANG_CODE_TO_ID).find(
          ([, id]) => id === langId,
        )?.[0];
        if (code) params.set("lang_id", code);
      }
      if (letter) params.set("letter", letter);
      const qs = params.toString();
      setRows(
        await api.get<SubtitleRow[]>(`/api/v1/subtitles${qs ? `?${qs}` : ""}`),
      );
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
    if (!fTitle.trim() || fLang === "0" || !fFile) return;
    setBusy(true);
    setMsg(null);
    try {
      // 真实文件链路（审计修复 P1 空壳）：先 multipart 上传到 attachments 拿 sha256，
      // 再建字幕记录绑定 attach://<sha>——下载端直接回文件字节，不再只回引用。
      const form = new FormData();
      form.append("file", fFile);
      // 凭证由 HttpOnly flux_token cookie 自动携带（P1 收敛，token 不再进 JS）
      const lang = rawFetchHelpers.lang();
      const upRes = await fetch(
        rawFetchHelpers.base() + "/api/v1/attachments",
        {
          method: "POST",
          headers: {
            ...(lang ? { "Accept-Language": lang } : {}),
          },
          body: form,
        },
      );
      const upBody = (await upRes.json()) as {
        code: number;
        message?: string;
        data?: { sha256: string };
      };
      if (upBody.code !== 0 || !upBody.data?.sha256) {
        throw new ApiError(upBody.code, upBody.message ?? "字幕文件上传失败");
      }
      // lang 存旧站代码（chs/cht/eng…）
      const code =
        Object.entries(LANG_CODE_TO_ID).find(([, id]) => id === fLang)?.[0] ??
        "other";
      await api.post("/api/v1/subtitles", {
        torrent_id: fTorrentId ? Number(fTorrentId) : 0,
        title: fTitle,
        lang: code,
        file_sha: upBody.data.sha256,
      });
      setFTitle("");
      setFFile(null);
      setMsg(t.reward.replace("{magic}", currency));
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
                  onChange={(e) => setFFile(e.target.files?.[0] ?? null)}
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
                  onChange={(e) =>
                    setFTorrentId(e.target.value.replace(/\D/g, ""))
                  }
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
                <select
                  value={fLang}
                  onChange={(e) => setFLang(e.target.value)}
                >
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
                  onClick={() => {
                    setFTitle("");
                    setFTorrentId("");
                    setFLang("0");
                    setFFile(null);
                  }}
                />
              </td>
            </tr>
          </tbody>
        </table>
        {msg && <p className="subtitles-msg">{msg}</p>}
      </form>

      {/* 搜索：关键词 + 语言下拉 */}
      <form
        className="subtitles-search-form"
        onSubmit={(e) => e.preventDefault()}
      >
        <label htmlFor="subtitles-search">{t.searchLabel}</label>
        <input
          id="subtitles-search"
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <select
          value={langId}
          onChange={(e) => setLangId(e.target.value)}
          aria-label={t.lang}
        >
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
      {rows !== null && (
        <SubtitleListTable rows={rows} onMsg={setMsg} onReload={load} />
      )}
      <p className="text-xs text-sub">1 - {rows?.length ?? 0}</p>
    </div>
  );
}
