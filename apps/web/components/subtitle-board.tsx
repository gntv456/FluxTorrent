"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type {
  SubtitleListResp,
  SubtitleRow,
} from "@/components/subtitle-board-shared";
import {
  LETTERS,
  fmtKB,
  boldRule,
  loadLangDict,
  SubtitleListTable,
} from "@/components/subtitle-board-table";
import { SubtitleRequestPanel } from "@/components/subtitle-request-panel";
import { SubtitleUploadForm } from "@/components/subtitle-upload-form";

// 字幕区（参考站 subtitles.php 复刻 + 0146 治理）：
// 标题条 → 规则卡（按 kind 切影视/歌词两套）→ 上传表单（元数据 + kind 显隐）
// → 筛选条 → 分页列表（评分/真实大小/坏字幕标记）→ 求字幕悬赏（pots）。

const PER_PAGES = [25, 50, 100];

export function SubtitleBoard({
  fixedTorrentId,
}: { fixedTorrentId?: number }) {
  const { dict } = useI18n();
  const t = dict.subtitles;
  const [kind, setKind] = useState<"subtitle" | "lyric">("subtitle");
  const [langs, setLangs] = useState<
    { code: string; name: string; flag: string }[]
  >([]);
  const [rows, setRows] = useState<SubtitleRow[]>([]);
  const [total, setTotal] = useState(0);
  const [totalSize, setTotalSize] = useState(0);
  const [search, setSearch] = useState("");
  const [langCode, setLangCode] = useState("");
  const [letter, setLetter] = useState("");
  const [page, setPage] = useState(1);
  const [perPage, setPerPage] = useState(50);
  // 上传表单
  const [msg, setMsg] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);

  useEffect(() => {
    loadLangDict().then(setLangs);
    fetch("/api/v1/site-profile")
      .then((r) => r.json())
      .then((b: { data?: { subtitle_kind?: string } }) => {
        if (b.data?.subtitle_kind === "lyric") setKind("lyric");
      })
      .catch(() => {});
  }, []);

  const load = useCallback(async () => {
    try {
      const params = new URLSearchParams();
      if (search.trim()) params.set("search", search.trim());
      if (langCode) params.set("lang_id", langCode);
      if (letter) params.set("letter", letter);
      if (fixedTorrentId) {
        params.set("torrent_id", String(fixedTorrentId));
      }
      params.set("page", String(page));
      params.set("per_page", String(perPage));
      const resp = await api.get<SubtitleListResp | SubtitleRow[]>(
        `/api/v1/subtitles?${params.toString()}`,
      );
      if (Array.isArray(resp)) {
        setRows(resp);
        setTotal(resp.length);
      } else {
        setRows(resp.items);
        setTotal(resp.total);
      }
    } catch {
      setRows([]);
      setTotal(0);
    }
  }, [search, langCode, letter, page, perPage, fixedTorrentId]);

  useEffect(() => {
    load();
  }, [load]);

  useEffect(() => {
    // 页头总上传量（全量口径，不受筛选影响）
    api
      .get<SubtitleListResp>("/api/v1/subtitles?per_page=100")
      .then((r) =>
        setTotalSize(
          (r.items ?? []).reduce((acc, x) => acc + (x.size ?? 0), 0),
        ),
      )
      .catch(() => {});
  }, [msg, reloadKey]);

  const rules = kind === "lyric" ? (t.lyricRules ?? t.rules) : t.rules;

  const pages = Math.max(1, Math.ceil(total / perPage));
  const pageWin = useMemo(() => {
    const arr: number[] = [];
    const lo = Math.max(1, page - 2);
    const hi = Math.min(pages, lo + 4);
    for (let i = lo; i <= hi; i++) arr.push(i);
    return arr;
  }, [page, pages]);

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

      {/* 规则卡（kind 两套） */}
      <section className="subtitles-rules">
        <h2>{t.rulesTitle}</h2>
        <ul>
          {rules.map((r) => (
            <li key={r} dangerouslySetInnerHTML={{ __html: boldRule(r) }} />
          ))}
        </ul>
        <p className="subtitles-required-note">
          {t.requiredNote.replace("*", "*")}
        </p>
      </section>

      <SubtitleUploadForm
        kind={kind}
        langs={langs}
        fixedTorrentId={fixedTorrentId}
        onMsg={setMsg}
        onUploaded={() => {
          load();
          setReloadKey((k) => k + 1);
        }}
      />

      {/* 搜索：关键词 + 语言下拉 + 每页 */}
      <form
        className="subtitles-search-form"
        onSubmit={(e) => e.preventDefault()}
      >
        <label htmlFor="subtitles-search">{t.searchLabel}</label>
        <input
          id="subtitles-search"
          type="text"
          value={search}
          onChange={(e) => {
            setSearch(e.target.value);
            setPage(1);
          }}
        />
        <select
          value={langCode}
          onChange={(e) => {
            setLangCode(e.target.value);
            setPage(1);
          }}
          aria-label={t.lang}
        >
          <option value="">{t.allLangs}</option>
          {langs.map((l) => (
            <option key={l.code} value={l.code}>
              {l.flag} {l.name}
            </option>
          ))}
        </select>
        <select
          value={perPage}
          onChange={(e) => {
            setPerPage(Number(e.target.value));
            setPage(1);
          }}
          aria-label={t.perPageLabel}
        >
          {PER_PAGES.map((n) => (
            <option key={n} value={n}>
              {n}
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
            onClick={() => {
              setLetter(letter === l ? "" : l);
              setPage(1);
            }}
          >
            {l}
          </button>
        ))}
      </nav>

      {/* 字幕列表（七列） */}
      <SubtitleListTable rows={rows} onMsg={setMsg} onReload={load} />

      {/* 分页条（A7：count 同谓词，翻页不重叠） */}
      {pages > 1 && (
        <nav className="mt-2 flex items-center justify-center gap-2 text-sm">
          <button
            type="button"
            className="btn2"
            disabled={page <= 1}
            onClick={() => setPage(page - 1)}
          >
            ‹
          </button>
          {pageWin.map((p) => (
            <button
              key={p}
              type="button"
              className={p === page ? "btn" : "btn2"}
              onClick={() => setPage(p)}
            >
              {p}
            </button>
          ))}
          <button
            type="button"
            className="btn2"
            disabled={page >= pages}
            onClick={() => setPage(page + 1)}
          >
            ›
          </button>
          <span className="text-sub">
            {total} · {page}/{pages}
          </span>
        </nav>
      )}

      {/* 求字幕悬赏（pots） */}
      {!fixedTorrentId && (
        <SubtitleRequestPanel
          langs={langs}
          onMsg={setMsg}
          reloadKey={reloadKey}
        />
      )}
    </div>
  );
}
