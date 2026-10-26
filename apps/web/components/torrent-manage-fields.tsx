"use client";

import { useI18n } from "@/i18n/client";
import { api, ApiError } from "@/lib/api-client";
import { ATTACH_ACCEPT, FLD, PILL } from "@/components/torrent-manage-parts";
import { MetaNfoFields } from "@/components/torrent-manage-meta-nfo";
import type { SectionKindMeta } from "@/components/admin-sections-shared";
import { SectionKindControl } from "@/components/section-kind-control";
import { catPath } from "@/lib/site-profile";

type Dict = ReturnType<typeof useI18n>["dict"];

const ATTACH_DROP =
  "flex min-h-[44px] cursor-pointer items-center justify-center "
  + "rounded-[var(--r-sm)] border border-dashed border-line "
  + "bg-[var(--head-b)] px-3 text-sm text-sub "
  + "hover:border-[var(--brand-orange)]";
const IMDB_INPUT =
  "w-40 rounded-[var(--r-sm)] border border-line px-2 py-1 text-sm";

/** 编辑弹层·基础字段块 + 元数据块（0184 从 torrent-manage.tsx 拆出，
 *  300 行门禁）：标题/副题/分类/多维质量（分类联动由主组件驱动）、
 *  封面/价格；PT-Gen/MediaInfo/IMDB（按站型 metaSources 显隐）+ 图床附件。 */

/** 编辑弹层·基础字段块（0184 从 torrent-manage.tsx 拆出）：标题/副题/分类/
 *  多维质量下拉（分类联动由主组件驱动的 kinds/dict）/封面/价格行 */
export function ManageBasicFields({
  dict,
  fName,
  setFName,
  fSub,
  setFSub,
  fCat,
  setFCat,
  cats,
  catKinds,
  catDict,
  fSec,
  setFSec,
  fPoster,
  setFPoster,
  fPrice,
  setFPrice,
  currency,
}: {
  dict: Dict;
  fName: string;
  setFName: (v: string) => void;
  fSub: string;
  setFSub: (v: string) => void;
  fCat: number;
  setFCat: (v: number) => void;
  cats: { id: number; name: string; parent_id?: number | null }[];
  catKinds: SectionKindMeta[];
  catDict: Record<string, { id: number; name: string }[]>;
  fSec: Record<string, string>;
  setFSec: React.Dispatch<React.SetStateAction<Record<string, string>>>;
  fPoster: string;
  setFPoster: (v: string) => void;
  fPrice: number;
  setFPrice: (v: number) => void;
  currency: string;
}) {
  const u = dict.upload;
  return (
    <>
      <label className="flex flex-col gap-1 text-xs">
        {dict.torrentManage2.fieldName}
        <input
          value={fName}
          onChange={(e) => setFName(e.target.value)}
          className={FLD}
        />
      </label>
      <label className="flex flex-col gap-1 text-xs">
        {dict.torrentManage2.fieldSub}
        <input
          value={fSub}
          onChange={(e) => setFSub(e.target.value)}
          className={FLD}
        />
      </label>
      <label className="flex flex-col gap-1 text-xs">
        {u.category}
        <select
          value={fCat}
          onChange={(e) => setFCat(Number(e.target.value))}
          className={FLD}
        >
          {cats.map((c) => (
            <option key={c.id} value={c.id}>
              {catPath(cats, c)}
            </option>
          ))}
        </select>
      </label>
      {/* 多维属性（B2 六类型）：两列网格 + 标签在上控件在下（对齐模态字段
          风格），不再 flex-wrap 平铺——42 个维度混排时观感即「乱」（与
          发布页同源整改）。控件实现收敛到 section-kind-control（发布页
          同款，ariaLabel 供可达名）；空选项枚举维度不显示（同发布页）。 */}
      {catKinds.length > 0 && (
        <div className="grid gap-2 sm:grid-cols-2">
          {catKinds
            .filter((k) => {
              const type = k.field_type ?? "select";
              const opts = catDict[k.kind] ?? [];
              return type === "multiselect" || type === "select"
                ? opts.length > 0
                : true;
            })
            .map((k) => {
              const type = k.field_type ?? "select";
              const val = fSec[k.kind] ?? "";
              const set = (v: string) =>
                setFSec((prev) => ({ ...prev, [k.kind]: v }));
              return (
                <div key={k.kind} className="flex flex-col gap-1 text-xs">
                  <span className="whitespace-nowrap text-sub">
                    {k.label}
                    {k.required ? " *" : ""}
                  </span>
                  <SectionKindControl
                    type={type}
                    value={val}
                    onChange={set}
                    opts={catDict[k.kind] ?? []}
                    noneLabel={u.gradeNone}
                    ariaLabel={k.label}
                    inputCls={FLD}
                  />
                </div>
              );
            })}
        </div>
      )}
      {/* 0173 对齐发布页：封面外链 + 付费价格 */}
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-xs">
        <label className="flex items-center gap-1">
          <span className="whitespace-nowrap text-sub">{u.poster}：</span>
          <input
            value={fPoster}
            placeholder="https://…"
            onChange={(e) => setFPoster(e.target.value)}
            className={`${FLD} w-56`}
          />
        </label>
        <label className="flex items-center gap-1">
          <span className="whitespace-nowrap text-sub">{u.price}：</span>
          <input
            type="number"
            min={0}
            max={1000000}
            value={fPrice}
            onChange={(e) => setFPrice(Number(e.target.value) || 0)}
            className={`${FLD} w-28`}
          />
          <span className="max-w-40 text-[11px] text-sub">
            {currency}（0 = 免费）
          </span>
        </label>
      </div>
    </>
  );
}

/** 编辑弹层·元数据块（0184）：PT-Gen 生成简介（按站型显隐）+ MediaInfo +
 *  IMDB 条目输入（imdb 源启用时） */
export function ManageMetaFields({
  dict,
  metaSrc,
  fPtgenUrl,
  setFPtgenUrl,
  ptgenBusy,
  onGen,
  fMediainfo, setFMediainfo,
  fNfo, setFNfo,
  fImdb,
  setFImdb,
  attach,
}: {
  dict: Dict;
  metaSrc: string[];
  fPtgenUrl: string;
  setFPtgenUrl: (v: string) => void;
  ptgenBusy: boolean;
  onGen: () => void;
  fMediainfo: string; setFMediainfo: (v: string) => void;
  fNfo: string; setFNfo: (v: string) => void;
  fImdb: string;
  setFImdb: (v: string) => void;
  attach: { busy: boolean; msg: string | null; upload: (f: File) => void };
}) {
  const u = dict.upload;
  const t = dict.torrentManage2;
  return (
    <>
      {metaSrc.length > 0 && (
        <div className="flex flex-wrap items-center gap-2 text-xs">
          <span className="whitespace-nowrap font-bold">{u.ptgen}</span>
          <input
            value={fPtgenUrl}
            placeholder={u.ptgenPlaceholder}
            onChange={(e) => setFPtgenUrl(e.target.value)}
            className={`${FLD} w-64`}
          />
          <button
            type="button"
            disabled={ptgenBusy || !fPtgenUrl.trim()}
            onClick={onGen}
            className={`${PILL} bg-sky text-white`}
          >
            {ptgenBusy ? u.ptgenBusy : u.ptgenBtn}
          </button>
          <span className="text-[11px] text-sub">{u.ptgenHint}</span>
        </div>
      )}
      {/* 图床附件（0184 对齐发布页）：上传成功追加进简介 */}
      <div className="flex flex-col gap-1 text-xs">
        <span className="font-bold">{u.attachLabel}</span>
        <label className={ATTACH_DROP}>
          <input
            type="file"
            className="sr-only"
            accept={ATTACH_ACCEPT}
            disabled={attach.busy}
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) void attach.upload(f);
              e.currentTarget.value = "";
            }}
          />
          {attach.busy ? dict.upload.attachUploading : u.attachHint}
        </label>
        {attach.msg && (
          <p className="text-[11px] text-sky-deep">{attach.msg}</p>
        )}
      </div>
      {/* MediaInfo + NFO 两域（批 B 拆出，见新文件头注释的行数缘由） */}
      <MetaNfoFields
        dict={dict} metaSrc={metaSrc}
        fMediainfo={fMediainfo} setFMediainfo={setFMediainfo}
        fNfo={fNfo} setFNfo={setFNfo}
      />
      {/* 条目输入按站型显隐（0184 对齐发布页 metaSources 口径） */}
      {metaSrc.includes("imdb") && (
        <label className="flex flex-col gap-1 text-xs">
          {t.fieldImdb ?? "IMDB"}
          <input
            value={fImdb}
            placeholder="tt1234567"
            onChange={(e) =>
              setFImdb(e.target.value.replace(/[^tT0-9]/g, "").slice(0, 10))
            }
            className={IMDB_INPUT}
          />
          <span className="text-[11px] text-sub">{t.fieldImdbNote}</span>
        </label>
      )}
    </>
  );
}
