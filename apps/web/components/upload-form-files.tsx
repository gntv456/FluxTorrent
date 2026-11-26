"use client";

import { useMemo, useState, type RefObject } from "react";
import { api } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { FormRow, fieldCls } from "@/components/upload-form-parts";
import { UploadNfoAttach } from "@/components/upload-form-nfo-attach";
import { UploadNameWizard } from "@/components/upload-name-wizard";
import {
  UploadPricePosterRows,
  UploadMetaIdsRow,
  UploadImdbRow,
} from "@/components/upload-price-poster";
import {
  buildName,
  resolveSecName,
  type SecDictRow,
} from "@/lib/name-builder";

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
  setFileErr,
  metaSources,
  secVals,
  secDict,
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
  /** 选到文件后立刻撤掉「请先选择 .torrent」错误条（P1-4） */
  setFileErr: (v: boolean) => void;
  metaSources: string[];
  /** 命名向导（0525）：质量维度选中值与字典——主表单透传，
   *  与「更多选项」同款折叠面板挂在主标题输入框下。 */
  secVals: Record<string, string>;
  secDict: Record<string, SecDictRow[]>;
}) {
  const { dict, currency } = useI18n();
  // 窄屏：辅助元数据字段默认折起（桌面照旧全展开）
  const [moreOpen, setMoreOpen] = useState(false);
  // 命名向导（0525）折叠态 + PT-Gen 回包的建议名（此前被丢弃）
  const [wizardOpen, setWizardOpen] = useState(false);
  const [metaName, setMetaName] = useState("");

  /** 向导预览：元数据建议名 + 维度选中名实时拼装（与 buildName 同口径） */
  const wizardPreview = useMemo(() => {
    if (!wizardOpen) return "";
    return buildName({
      metaName,
      source: resolveSecName("source", secVals, secDict),
      standard: resolveSecName("standard", secVals, secDict),
      codec: resolveSecName("codec", secVals, secDict),
      audio: resolveSecName("audio_codec", secVals, secDict),
      team: resolveSecName("team", secVals, secDict),
    });
  }, [wizardOpen, metaName, secVals, secDict]);

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
      // 0525 命名向导：回包 name（中文名+外文名+年份）此前被丢弃——
      // 存为向导建议段；主标题为空时直接预填，否则等用户在向导里应用。
      if (r.name?.trim()) {
        setMetaName(r.name.trim());
        if (!name.trim()) setName(r.name.trim());
      }
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
          <label className="uf-drop uf-drop--required" tabIndex={-1}>
            <input
              ref={fileRef}
              name="file"
              type="file"
              accept=".torrent,application/x-bittorrent"
              required
              className="sr-only"
              onChange={(e) => {
                setFileName(e.target.files?.[0]?.name ?? "");
                setFileErr(false);
              }}
            />
            {fileName || dict.upload.formFileHint}
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
          {/* 命名向导（0525）：拆独立件防行数门禁，状态就近（300 行门禁） */}
          <UploadNameWizard
            open={wizardOpen}
            setOpen={setWizardOpen}
            metaName={metaName}
            preview={wizardPreview}
            onApply={setName}
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
        <UploadImdbRow
          imdb={imdb}
          setImdb={setImdb}
          onAutoFill={autoFill}
        />
      )}
      {/* 0525 豆瓣/TMDB 双输入拆至 upload-price-poster.tsx 同族件（行数门禁） */}
      {(metaSources.includes("douban") || metaSources.includes("tmdb")) && (
        <UploadMetaIdsRow
          douban={douban}
          setDouban={setDouban}
          tmdb={tmdb}
          setTmdb={setTmdb}
        />
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
                className={"min-h-10 flex-1 rounded-[var(--r-sm)] " +
                  "border border-[var(--compat-line)] " +
                  "bg-[var(--compat-paper)] px-3 text-sm text-ink " +
                  "outline-none focus:border-[var(--compat-orange)]"}
              />
              <button
                type="button"
                disabled={ptgenBusy || !ptgenUrl.trim()}
                onClick={() => genDescr()}
                className={"min-h-10 rounded-[10px] " +
                  "border border-[var(--compat-line)] px-4 text-sm " +
                  "font-bold text-ink hover:border-[var(--compat-orange)] " +
                  "disabled:opacity-50"}
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
            className={"w-full rounded-[var(--r-sm)] border border-line " +
              "bg-[var(--surface-card)] p-2 font-mono text-xs"}
            maxLength={60000}
          />
        </FormRow>
      )}
      {/* 0525 定价/海报拆至 upload-price-poster.tsx（行数门禁） */}
      <UploadPricePosterRows
        price={price}
        setPrice={setPrice}
        poster={poster}
        setPoster={setPoster}
      />
    </>
  );
}
