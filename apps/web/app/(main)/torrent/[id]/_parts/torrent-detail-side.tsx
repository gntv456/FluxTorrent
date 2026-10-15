/**
 * 详情页右侧信息栏三件套（设计稿 §4/§8：桌面 sticky 侧栏，移动端流式/隐藏）：
 *  td-vitals 状态 2×2 大数字 + 健康度横幅；td-files-mini 前 5 行文件卡
 *  （「查看全部」锚到主栏完整文件折叠块）；td-meta 元信息聚合卡。
 * 数据装载与显隐门（sectOn）留在 page.tsx，部件只做呈现。
 */

import { dateLocale } from "@/i18n/config";
import type { Dict } from "@/i18n/zh-CN";
import type { Locale } from "@/i18n/config";
import type { TorrentListItem } from "@fluxtorrent/domain-types";
import type { TorrentDetailExt } from "@/components/torrent-detail-blocks";

interface FileItem {
  file_index: number;
  path: string;
  size: number;
}

interface SideProps {
  t: TorrentListItem;
  ext: TorrentDetailExt | null;
  files: FileItem[];
  category: string;
  inCollections: { id: number; kind: string; name: string }[];
  dict: Dict;
  locale: Locale;
  relTime: (iso: string) => string;
}

/** 文件大小（侧栏卡内联格式化） */
function fsize(bytes: number): string {
  const u = ["B", "KB", "MB", "GB", "TB"];
  let n = bytes;
  let i = 0;
  while (n >= 1024 && i < u.length - 1) {
    n /= 1024;
    i++;
  }
  return `${i === 0 ? n : n.toFixed(1)} ${u[i]}`;
}

/** 扩展名小徽标（侧栏文件行用，与桌面 FileTree 图标语义一致） */
function extBadge(path: string): string {
  const m = path.split(".").pop();
  if (!m || m.length > 4 || m.includes("/")) return "FILE";
  return m.toUpperCase();
}

/** td-vitals：2×2 大数字 + 健康度横幅（设计稿 §6 组件规范） */
export function TorrentVitals({
  t,
  ext,
  dict,
}: Pick<SideProps, "t" | "ext" | "dict">) {
  const d = dict.tdetail;
  return (
    <aside className="td-side-card td-vitals">
      <div className="td-vitals__grid">
        <div className="td-vitals__cell td-vitals__cell--seed">
          <b className="num">{t.seeders}</b>
          <span>{dict.torrent.seeding}</span>
        </div>
        <div className="td-vitals__cell td-vitals__cell--leech">
          <b className="num">{t.leechers}</b>
          <span>{dict.torrent.leeching}</span>
        </div>
        <div className="td-vitals__cell td-vitals__cell--done">
          <b className="num">{t.times_completed}</b>
          <span>{dict.torrent.completed}</span>
        </div>
        <div className="td-vitals__cell td-vitals__cell--thanks">
          <b className="num">{ext?.thanks_count ?? 0}</b>
          <span>{d?.thanksCountLabel ?? dict.torrent.thanksCount}</span>
        </div>
      </div>
      <p
        className={`td-vitals__health ${
          t.seeders === 0 ? "is-dead" : "is-alive"
        }`}
      >
        {t.seeders === 0 ? d?.healthDead : d?.healthAlive}
      </p>
      <p className="td-vitals__views">
        {d?.heatViews}
        <b className="num">{ext?.views ?? "—"}</b>
      </p>
    </aside>
  );
}

/** td-files-mini：侧栏前 5 行斑马纹 + 查看全部锚点（设计稿 §6） */
export function TorrentSideFiles({
  files,
  dict,
}: Pick<SideProps, "files" | "dict">) {
  const shown = files.slice(0, 5);
  return (
    <div className="td-side-card td-files-mini">
      <div className="td-side-card__head">
        <h3>{dict.torrent.filesTitle}</h3>
        <span className="num">{files.length}</span>
      </div>
      <div className="td-files-mini__list">
        {shown.map((f) => (
          <div className="td-files-mini__row" key={f.file_index}>
            <span className="td-files-mini__ext">{extBadge(f.path)}</span>
            <span className="td-files-mini__name" title={f.path}>
              {f.path.split("/").pop()}
            </span>
            <span className="td-files-mini__size num">
              {fsize(f.size)}
            </span>
          </div>
        ))}
      </div>
      {files.length > shown.length && (
        <a className="td-files-mini__more" href="#td-files">
          {dict.torrent.filesAll.replace("{n}", String(files.length))}
        </a>
      )}
    </div>
  );
}

/** td-meta：发布者/时间/分类/最后活动/合集聚合卡（设计稿 §6，
 *  桌面从头部 meta 行收敛到侧栏；<768 隐藏——头部 meta 行已承载） */
export function TorrentSideMeta({
  t,
  ext,
  category,
  inCollections,
  dict,
  locale,
  relTime,
}: SideProps) {
  const d = dict.tdetail;
  return (
    <div className="td-side-card td-meta">
      <div className="td-side-card__head">
        <h3>{dict.torrent.metaTitle}</h3>
      </div>
      <div className="td-meta__rows">
        <div className="td-meta__row">
          <span>{dict.torrent.uploader}</span>
          <b>
            {t.anonymous ? dict.torrent.anonymous : (t.owner_name ?? "—")}
          </b>
        </div>
        <div className="td-meta__row">
          <span>{dict.torrent.uploadedAt}</span>
          <b title={new Date(t.created_at).toLocaleString(dateLocale(locale))}>
            {relTime(t.created_at)}
          </b>
        </div>
        <div className="td-meta__row">
          <span>{d?.lastActivity}</span>
          <b
            title={new Date(
              ext?.last_action ?? t.created_at,
            ).toLocaleString(dateLocale(locale))}
          >
            {relTime(ext?.last_action ?? t.created_at)}
          </b>
        </div>
        <div className="td-meta__row">
          <span>{dict.torrent.category}</span>
          <b>{category}</b>
        </div>
        {inCollections.map((c) => (
          <div className="td-meta__row" key={c.id}>
            <span>{dict.collections.inTitle}</span>
            <a href={`/collections/${c.id}`} className="td-meta__link">
              {c.name}
            </a>
          </div>
        ))}
      </div>
    </div>
  );
}

/** 侧栏「更多」入口卡（设计稿 §6 .entry：当前在线/字幕/下载记录，
 *  点击锚点跳转主栏对应折叠卡；字幕数为客户端懒加载无 SSR 数据，
 *  不显示计数徽标）。<768 整卡隐藏（CSS @767）——锚点条承担导航 */
export function TorrentSideEntries({
  t,
  dict,
}: Pick<SideProps, "t" | "dict">) {
  return (
    <div className="td-side-card td-side-entries">
      <div className="td-side-card__head">
        <h3>{dict.torrents.moreActions}</h3>
      </div>
      <a className="td-entry" href="#td-sec-peers">
        {dict.torrents.peersTitle}
        <span className="td-entry__cnt num">{t.seeders + t.leechers}</span>
      </a>
      <a className="td-entry" href="#td-sec-subtitles">
        {dict.subtitles.panelTitle}
      </a>
      <a className="td-entry" href="#td-sec-snatches">
        {dict.snatches2.title}
        <span className="td-entry__cnt num">{t.times_completed}</span>
      </a>
    </div>
  );
}

/** 右侧信息栏组合（桌面上段 sticky；related 由 page 放进下段） */
export function TorrentDetailSide(props: SideProps) {
  return (
    <aside className="td-side">
      <TorrentVitals t={props.t} ext={props.ext} dict={props.dict} />
      {props.files.length > 0 && (
        <TorrentSideFiles files={props.files} dict={props.dict} />
      )}
      <TorrentSideMeta {...props} />
      <TorrentSideEntries t={props.t} dict={props.dict} />
    </aside>
  );
}
