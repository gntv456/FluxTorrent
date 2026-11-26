"use client";

import { useI18n } from "@/i18n/client";
import { FormRow, fieldCls } from "@/components/upload-form-parts";

/** 发布表单展示行（0525 从 upload-form-files.tsx 拆出防行数门禁）：
 *  定价/海报行 + 豆瓣/TMDB 双输入行。纯展示输入块，
 *  state 仍由主表单透传。 */

export function UploadPricePosterRows({
  price,
  setPrice,
  poster,
  setPoster,
}: {
  price: number;
  setPrice: (v: number) => void;
  poster: string;
  setPoster: (v: string) => void;
}) {
  const { dict, currency } = useI18n();
  return (
    <>
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
              className={"min-h-10 w-36 rounded-[var(--r-sm)] " +
                "border border-[var(--compat-line)] " +
                "bg-[var(--compat-paper)] px-3 text-sm text-ink " +
                "outline-none focus:border-[var(--compat-orange)]"}
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

/** 豆瓣 / TMDB 元数据 ID 行（窄屏上下堆叠：并排时两个 placeholder
 *  都被截成「movie.douban.com/s」「themoviedb.org/mov」，看不出该填什么） */
export function UploadMetaIdsRow({
  douban,
  setDouban,
  tmdb,
  setTmdb,
}: {
  douban: string;
  setDouban: (v: string) => void;
  tmdb: string;
  setTmdb: (v: string) => void;
}) {
  const { dict } = useI18n();
  return (
    <FormRow label={dict.upload.metaIds}>
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
  );
}

/** IMDb 链接行（onAutoFill = 失焦自动拉简介，0525 拆件随行） */
export function UploadImdbRow({
  imdb,
  setImdb,
  onAutoFill,
}: {
  imdb: string;
  setImdb: (v: string) => void;
  onAutoFill: () => void;
}) {
  const { dict } = useI18n();
  return (
    <FormRow label={dict.upload.imdb}>
      <div className="flex flex-col gap-1">
        <input
          type="url"
          name="imdb"
          value={imdb}
          onChange={(e) => setImdb(e.target.value)}
          onBlur={onAutoFill}
          placeholder="https://www.imdb.com/title/tt0468569/"
          maxLength={300}
          className={fieldCls}
        />
        <span className="text-xs text-sub">{dict.upload.imdbHint}</span>
      </div>
    </FormRow>
  );
}
