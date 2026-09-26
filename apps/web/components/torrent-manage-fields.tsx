"use client";

import { useI18n } from "@/i18n/client";
import { api, ApiError } from "@/lib/api-client";
import { ATTACH_ACCEPT, FLD, PILL } from "@/components/torrent-manage-parts";
import type { SectionKindMeta } from "@/components/admin-sections-shared";
import { SectionKindBool } from "@/components/section-kind-bool";

type Dict = ReturnType<typeof useI18n>["dict"];

const ATTACH_DROP =
  "flex min-h-[44px] cursor-pointer items-center justify-center "
  + "rounded-[var(--r-sm)] border border-dashed border-line "
  + "bg-[var(--head-b)] px-3 text-sm text-sub "
  + "hover:border-[var(--baozi-orange)]";
const MONO_AREA =
  "rounded-[var(--r-sm)] border border-line px-2 py-1 font-mono text-xs";
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
  cats: { id: number; name: string }[];
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
              {c.name}
            </option>
          ))}
        </select>
      </label>
      {/* 多维属性（B2 六类型）：按 field_type 渲染控件，空 = 不设。
          multiselect 以逗号串承载多值（与发布表单同形） */}
      {catKinds.length > 0 && (
        <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-xs">
          {catKinds.map((k) => {
            const type = k.field_type ?? "select";
            const val = fSec[k.kind] ?? "";
            const fld = `${FLD} min-w-[6rem]`;
            const set = (v: string) =>
              setFSec((prev) => ({ ...prev, [k.kind]: v }));
            const opts = catDict[k.kind] ?? [];
            const multiIds = val
              .split(",")
              .map((s) => Number(s.trim()))
              .filter((n) => n > 0);
            return (
              <label key={k.kind} className="flex items-center gap-1">
                <span className="whitespace-nowrap text-sub">
                  {k.label}：
                </span>
                {type === "multiselect" ? (
                  <span className="flex flex-wrap items-center gap-x-2 gap-y-1">
                    {opts.map((o) => (
                      <label
                        key={o.id}
                        className="flex cursor-pointer items-center gap-1"
                      >
                        <input
                          type="checkbox"
                          checked={multiIds.includes(o.id)}
                          onChange={() => {
                            const next = multiIds.includes(o.id)
                              ? multiIds.filter((x) => x !== o.id)
                              : [...multiIds, o.id];
                            set(next.join(","));
                          }}
                          className="h-3.5 w-3.5 accent-[var(--baozi-orange)]"
                        />
                        {o.name}
                      </label>
                    ))}
                  </span>
                ) : type === "select" ? (
                  <select
                    value={val}
                    onChange={(e) => set(e.target.value)}
                    className={fld}
                  >
                    <option value="">{u.gradeNone}</option>
                    {opts.map((o) => (
                      <option key={o.id} value={o.id}>
                        {o.name}
                      </option>
                    ))}
                  </select>
                ) : type === "number" || type === "date" ? (
                  <input
                    type={type}
                    value={val}
                    onChange={(e) => set(e.target.value)}
                    className={`${fld} w-28`}
                  />
                ) : type === "bool" ? (
                  <SectionKindBool value={val} onChange={set} />
                ) : (
                  <input
                    value={val}
                    onChange={(e) => set(e.target.value)}
                    className={`${fld} w-40`}
                  />
                )}
              </label>
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
  fMediainfo,
  setFMediainfo,
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
  fMediainfo: string;
  setFMediainfo: (v: string) => void;
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
          {attach.busy ? "上传中…" : u.attachHint}
        </label>
        {attach.msg && (
          <p className="text-[11px] text-sky-deep">{attach.msg}</p>
        )}
      </div>
      {/* MediaInfo（0209 P2-14）：与发布页同口径，mediainfo 源关掉即隐藏 */}
      {metaSrc.includes("mediainfo") && (
        <label className="flex flex-col gap-1 text-xs">
          MediaInfo
          <textarea
            value={fMediainfo}
            rows={4}
            onChange={(e) => setFMediainfo(e.target.value)}
            className={MONO_AREA}
          />
        </label>
      )}
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
