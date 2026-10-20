/**
 * 外部条目链接徽标（豆瓣 / TMDB / IMDb）——作品卡与作品详情共用。
 *
 * 这三枚徽标是「作品层」存在的证据：同一部片的多版本靠发种时填的条目 ID
 * 归堆（见 publish_http/upload_ids.rs），点过去能核对是不是同一部。
 * 没填的就不出徽标——不拿标题去猜条目。
 */

import type { WorkListItem } from "@fluxtorrent/domain-types";
import type { Dict } from "@/i18n/zh-CN";

type Ids = Pick<
  WorkListItem,
  "imdb_id" | "tmdb_id" | "tmdb_url" | "douban_id"
>;

export function WorkIds({ work, dict }: { work: Ids; dict: Dict }) {
  const links: { key: string; label: string; href: string }[] = [];
  if (work.douban_id) {
    links.push({
      key: "douban",
      label: dict.works.douban,
      href: `https://movie.douban.com/subject/${work.douban_id}/`,
    });
  }
  if (work.tmdb_id) {
    links.push({
      key: "tmdb",
      label: dict.works.tmdb,
      // movie/tv 两条路径只有原始链接里才有，缺省按 movie 拼
      href:
        work.tmdb_url ??
        `https://www.themoviedb.org/movie/${work.tmdb_id}`,
    });
  }
  if (work.imdb_id) {
    links.push({
      key: "imdb",
      label: dict.works.imdb,
      href: `https://www.imdb.com/title/${work.imdb_id.toLowerCase()}/`,
    });
  }
  if (links.length === 0) return null;
  return (
    <span className="wk-ids">
      {links.map((l) => (
        <a
          key={l.key}
          href={l.href}
          target="_blank"
          rel="noopener noreferrer nofollow"
          className={`wk-id wk-id--${l.key}`}
          title={l.label}
        >
          {l.label}
        </a>
      ))}
    </span>
  );
}
