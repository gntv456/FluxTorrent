"use client";

import { useState, type RefObject } from "react";
import { api } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { FormRow, fieldCls } from "@/components/upload-form-parts";
import { UploadNfoAttach } from "@/components/upload-form-nfo-attach";

/** 发布表单·文件与 NFO / 附件块（从 upload-form.tsx 按域拆出，300 行门禁）：
 *  种子文件选择、标题/副标题、IMDb 与 PT-Gen、NFO、图床附件、MediaInfo、
 *  封面 URL、价格。fileRef/nfoRef/fileName 由主表单传入（submit 读取）；
 *  descr 同理（PT-Gen 生成与图床插入都会追加进同一段简介）。 */

/** 「更多选项」按钮的样式（单独提出来：写在 JSX 里那一行超 80 列） */
const MORE_BTN_CLS =
  "min-h-10 rounded-[var(--r-sm)] border border-[var(--compat-line)] " +
  "px-3 text-sm font-bold text-ink";

export function UploadFilesBlock({
  fileRef,
  nfoRef,
  fileName,
  setFileName,
  name,
  setName,
  smallDescr,
  setSmallDescr,
  imdb,
  setImdb,
  douban,
  setDouban,
  tmdb,
  setTmdb,
  ptgenUrl,
  setPtgenUrl,
  ptgenBusy,
  setPtgenBusy,
  setDescr,
  mediainfo,
  setMediainfo,
  poster,
  setPoster,
  price,
  setPrice,
  setMsg,
  metaSources,
}: {
  fileRef: RefObject<HTMLInputElement | null>;
  nfoRef: RefObject<HTMLInputElement | null>;
  fileName: string;
  setFileName: (v: string) => void;
  name: string;
  setName: (v: string) => void;
  smallDescr: string;
  setSmallDescr: (v: string) => void;
  imdb: string;
  setImdb: (v: string) => void;
  douban: string;
  setDouban: (v: string) => void;
  tmdb: string;
  setTmdb: (v: string) => void;
  ptgenUrl: string;
  setPtgenUrl: (v: string) => void;
  ptgenBusy: boolean;
  setPtgenBusy: (b: boolean) => void;
  setDescr: (v: string | ((prev: string) => string)) => void;
  mediainfo: string;
  setMediainfo: (v: string) => void;
  poster: string;
  setPoster: (v: string) => void;
  price: number;
  setPrice: (v: number) => void;
  setMsg: (m: string | null) => void;
  metaSources: string[];
}) {
  const { dict, currency } = useI18n();
  // 窄屏：辅助元数据字段默认折起（桌面照旧全展开）
  const [moreOpen, setMoreOpen] = useState(false);

  /** 元数据自动拉取（G7）：IMDb 链接失焦、且用户还没自己填 PT-Gen 链接时
   *  自动拉一次简介。手动按钮行为不变（仍可改链接后重拉）。 */
  async function autoFill() {
    const u = imdb.trim();
    if (!u || ptgenBusy || ptgenUrl.trim()) return;
    await genDescr(u);
  }

  async function genDescr(urlArg?: string) {
    const url = (urlArg ?? ptgenUrl).trim();
    if (!url || ptgenBusy) return;
    setPtgenBusy(true);
    try {
      const r = await api.get<{ name: string; descr: string }>(
        `/api/v1/ptgen?url=${encodeURIComponent(url)}`,
      );
      setDescr((prev) => (prev.trim() ? `${prev.trim()}\n\n` : "") + r.descr);
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
    } finally {
      setPtgenBusy(false);
    }
  }

  return (
    <>
      <FormRow label={dict.upload.fileLabel}>
        <div className="flex flex-col gap-1">
          {/* 2026-10-03：必填/选填此前视觉完全一致，加 uf-drop* 区分 */}
          <label className="uf-drop uf-drop--required">
            <input
              ref={fileRef}
              name="file"
              type="file"
              accept=".torrent,application/x-bittorrent"
              required
              className="sr-only"
              onChange={(e) => setFileName(e.target.files?.[0]?.name ?? "")}
            />
            {fileName ? `📎 ${fileName}` : dict.upload.formFileHint}
          </label>
        </div>
      </FormRow>
      <FormRow label={dict.upload.titleName}>
        <div className="flex flex-col gap-1">
          <input
            type="text"
            name="name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={dict.upload.nameHint}
            maxLength={200}
            className={fieldCls}
          />
        </div>
      </FormRow>
      <FormRow label={dict.upload.smallDescr}>
        <input
          type="text"
          name="small_descr"
          value={smallDescr}
          onChange={(e) => setSmallDescr(e.target.value)}
          placeholder={dict.upload.smallDescrPlaceholder}
          maxLength={120}
          className={fieldCls}
        />
      </FormRow>
      {metaSources.includes("imdb") && (
        <FormRow label={dict.upload.imdb}>
          <div className="flex flex-col gap-1">
            <input
              type="url"
              name="imdb"
              value={imdb}
              onChange={(e) => setImdb(e.target.value)}
              onBlur={autoFill}
              placeholder="https://www.imdb.com/title/tt0468569/"
              maxLength={300}
              className={fieldCls}
            />
            <span className="text-xs text-sub">{dict.upload.imdbHint}</span>
          </div>
        </FormRow>
      )}
      {(metaSources.includes("douban") || metaSources.includes("tmdb")) && (
        <FormRow label={dict.upload.metaIds}>
          {/* 窄屏上下堆叠：并排时两个 placeholder 都被截成
              「movie.douban.com/s」「themoviedb.org/mov」，看不出该填什么 */}
          <div className="flex flex-col gap-2 sm:flex-row">
            <input value={douban} name="douban"
              onChange={(e) => setDouban(e.target.value)}
              placeholder="movie.douban.com/subject/1292052" maxLength={300}
              className={`${fieldCls} min-w-0 sm:flex-1`} />
            <input value={tmdb} name="tmdb"
              onChange={(e) => setTmdb(e.target.value)}
              placeholder="themoviedb.org/movie/603" maxLength={300}
              className={`${fieldCls} min-w-0 sm:flex-1`} />
          </div>
          <span className="text-xs text-sub">{dict.upload.metaIdsHint}</span>
        </FormRow>
      )}
      {/* 窄屏：辅助元数据字段（PT-Gen / NFO / MediaInfo）折进「更多选项」，
          桌面照旧全展开（桌面表格放得下）。 */}
      <FormRow className="uf-more-toggle">
        <button
          type="button"
          onClick={() => setMoreOpen((v) => !v)}
          className={MORE_BTN_CLS}
        >
          {moreOpen ? dict.upload.lessOptions : dict.upload.moreOptions}
        </button>
      </FormRow>
      {metaSources.length > 0 && (
        <FormRow
          label={dict.upload.ptgen ?? "PT-Gen"}
          className={moreOpen ? "" : "uf-opt"}
        >
          <div className="flex flex-col gap-1">
            <div className="flex flex-wrap items-center gap-2">
              <input
                type="url"
                name="ptgen_url"
                value={ptgenUrl}
                onChange={(e) => setPtgenUrl(e.target.value)}
                placeholder={
                  dict.upload.ptgenPlaceholder ??
                  fmt(dict.upload.attachPtgenPh, {
                    sources: metaSources.join(" / "),
                  })
                }
                maxLength={300}
                className="min-h-10 flex-1 rounded-[var(--r-sm)] border border-[var(--compat-line)] bg-[var(--compat-paper)] px-3 text-sm text-ink outline-none focus:border-[var(--compat-orange)]"
              />
              <button
                type="button"
                disabled={ptgenBusy || !ptgenUrl.trim()}
                onClick={() => genDescr()}
                className="min-h-10 rounded-[10px] border border-[var(--compat-line)] px-4 text-sm font-bold text-ink hover:border-[var(--compat-orange)] disabled:opacity-50"
              >
                {ptgenBusy ? dict.upload.ptgenBusy : dict.upload.ptgenBtn}
              </button>
            </div>
            <span className="text-xs text-sub">{dict.upload.ptgenHint}</span>
          </div>
        </FormRow>
      )}
      <UploadNfoAttach
        nfoRef={nfoRef}
        setDescr={setDescr}
        rowCls={moreOpen ? "" : "uf-opt"}
      />
      {/* MediaInfo（0209 P2-14）：影视向字段——站长在元数据源里去掉 mediainfo 即隐藏 */}
      {metaSources.includes("mediainfo") && (
        <FormRow
          label={dict.upload.mediainfoLabel ?? "MediaInfo"}
          className={moreOpen ? "" : "uf-opt"}
        >
          <textarea
            rows={4}
            name="mediainfo"
            value={mediainfo}
            onChange={(e) => setMediainfo(e.target.value)}
            placeholder={dict.upload.mediainfoHint}
            className="w-full rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] p-2 font-mono text-xs"
            maxLength={60000}
          />
        </FormRow>
      )}
      <FormRow label={dict.upload.price}>
        <div className="flex flex-col gap-1">
          <div className="flex flex-wrap items-center gap-2">
            <input
              type="number"
              name="price"
              aria-label={dict.upload.price}
              min={0}
              max={1000000}
              value={price}
              onChange={(e) => setPrice(Number(e.target.value))}
              className="min-h-10 w-36 rounded-[var(--r-sm)] border border-[var(--compat-line)] bg-[var(--compat-paper)] px-3 text-sm text-ink outline-none focus:border-[var(--compat-orange)]"
            />
            <span className="text-sm text-sub">
              {currency}（0 = 免费，最大 1000000）
            </span>
          </div>
          <span className="text-xs text-sub">{dict.upload.priceHint}</span>
        </div>
      </FormRow>
      <FormRow label={dict.upload.poster}>
        <div className="flex flex-col gap-1">
          <input
            type="url"
            name="poster"
            value={poster}
            onChange={(e) => setPoster(e.target.value)}
            placeholder={dict.upload.posterPh}
            maxLength={500}
            className={fieldCls}
          />
          <span className="text-xs text-sub">{dict.upload.posterHint}</span>
        </div>
      </FormRow>
    </>
  );
}
