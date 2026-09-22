/**
 * 种子列表页搜索盒（从 app/(main)/torrents/page.tsx 按域拆出）：
 * TorrentsSearchBox = 常驻搜索行 + 已选条件摘要条 + 折叠高级面板外壳；
 * 0118 语义分组卡片在 ./torrents-adv-groups.tsx。
 * 无 hooks：保持 server component，数据由页面注入。
 */

import type { Dict } from "@/i18n/zh-CN";
import type {
  SectionDictRow,
  SectionKindMeta,
  TorrentsSP,
} from "./torrents-utils";
import type { TorrentChip } from "./torrents-chips";
import { TorrentsAdvGroups } from "./torrents-adv-groups";
import { TorrentsAdvGroupsTail } from "./torrents-adv-groups-tail";
import { AdvGear } from "./torrents-adv-gear";

export interface TorrentsSearchBoxProps {
  dict: Dict;
  sp: TorrentsSP;
  categories: { id: number; label: string }[];
  tags: { id: number; name: string; kind: string }[];
  secDict: Record<string, SectionDictRow[]> | null;
  dimKinds: SectionKindMeta[];
  selectedCats: Set<number>;
  chips: TorrentChip[];
  advancedOpen: boolean;
  nonSearchChips: number;
  showImdb: boolean;
  withParam: (sp: TorrentsSP, key: string, value: string | undefined) => string;
}

export function TorrentsSearchBox(props: TorrentsSearchBoxProps) {
  const {
    dict,
    sp,
    categories,
    tags,
    secDict,
    dimKinds,
    selectedCats,
    chips,
    advancedOpen,
    nonSearchChips,
    showImdb,
    withParam,
  } = props;
  const t2 = dict.torrents2;

  return (
    <form action="/torrents" method="get" className="torrent-search-box">
      {/* ── 常驻搜索行 ── */}
      <div className="tsb-bar">
        <label className="tsb-field tsb-field--scope">
          <span className="tsb-field__label">{t2.scope}</span>
          <select
            name="search_area"
            defaultValue={sp.search_area ?? "0"}
            aria-label={t2.scope}
          >
            <option value="0">{t2.areaTitle}</option>
            <option value="1">{t2.areaDescr}</option>
            <option value="3">{t2.areaUploader}</option>
            {/* IMDb 范围随站点元数据源显隐（与上传页条目输入同口径） */}
            {showImdb && <option value="4">IMDb</option>}
          </select>
        </label>
        <input
          id="searchinput"
          name="search"
          type="text"
          className="tsb-input"
          defaultValue={sp.search}
          placeholder={t2.keywordPh}
          autoComplete="off"
        />
        <label className="tsb-field tsb-field--mode">
          <span className="tsb-field__label">{t2.mode}</span>
          <select
            name="search_mode"
            defaultValue={sp.search_mode ?? "0"}
            aria-label={t2.mode}
          >
            <option value="0">{t2.modeAnd}</option>
            <option value="2">{t2.modeExact}</option>
          </select>
        </label>
        <button type="submit" className="baozi-button tsb-submit">
          {t2.searchBtn}
        </button>
        {/* 高级搜索触发器：紧跟「给我搜」之后的小齿轮按钮（好学站口径），
            点击开合下方 details 面板（client 组件内 document.getElementById） */}
        <AdvGear label={t2.advanced} badge={nonSearchChips} />
      </div>

      {/* ── 已选条件摘要（可单个移除） ── */}
      {chips.length > 0 && (
        <div className="tsb-active">
          <span className="tsb-active__title">{t2.activeTitle}</span>
          <div className="tsb-active__list">
            {chips.map((c) => (
              <a
                key={c.id}
                href={c.href}
                className="tsb-active__chip"
                title={t2.removeFilter}
              >
                <span>{c.text}</span>
                <span className="tsb-active__x" aria-hidden="true">
                  ×
                </span>
              </a>
            ))}
          </div>
          <a href="/torrents" className="tsb-active__clear">
            {t2.clearAll}
          </a>
        </div>
      )}

      {/* ── 高级搜索面板（0147：触发入口只剩常驻行的齿轮按钮，原 summary
            整行隐藏——details 仅作开合容器；有筛选条件时默认展开） ── */}
      <details
        className="tsb-adv tsb-adv--gear"
        id="tsb-adv"
        open={advancedOpen}
      >
        <summary className="tsb-adv__summary" title={t2.advanced}>
          <span className="tsb-adv__icon" aria-hidden="true">
            ⚙
          </span>
          <span>{t2.advanced}</span>
          {nonSearchChips > 0 && (
            <span className="tsb-adv__badge">{nonSearchChips}</span>
          )}
        </summary>
        <div className="tsb-adv__body">
          <p className="tsb-adv__hint">{t2.advHint}</p>

          <TorrentsAdvGroups
            dict={dict}
            sp={sp}
            categories={categories}
            tags={tags}
            secDict={secDict}
            dimKinds={dimKinds}
            selectedCats={selectedCats}
            withParam={withParam}
          />

          <TorrentsAdvGroupsTail
            dict={dict}
            sp={sp}
            categories={categories}
            tags={tags}
            secDict={secDict}
            dimKinds={dimKinds}
            selectedCats={selectedCats}
            withParam={withParam}
          />

          {/* 面板操作条 */}
          <div className="tsb-actions">
            <a href="/torrents" className="tsb-reset">
              {t2.advReset}
            </a>
            <button type="submit" className="baozi-button">
              {t2.advApply}
            </button>
          </div>
        </div>
      </details>
    </form>
  );
}
