/**
 * 详情页尾部三段（E6 拆出：感谢者 / InfoHash / 评论区；300 行门禁）。
 * 段落显隐走 sectOn（view_hidden.sections），数据态条件保留在调用侧。
 */

import { Comments, Thankers } from "@/components/torrent-detail-blocks";
import type { Dict } from "@/i18n/zh-CN";
import type { Locale } from "@/i18n/config";
import type { TorrentListItem } from "@fluxtorrent/domain-types";
import type {
  ThankItem,
} from "@/components/torrent-detail-blocks";
import type { TorrentComment } from "@fluxtorrent/domain-types";

export function TorrentDetailTail({
  t,
  thanks,
  thanksCount,
  comments,
  dict,
  locale,
  relTime,
  sectOn,
}: {
  t: TorrentListItem;
  thanks: ThankItem[];
  thanksCount: number;
  comments: TorrentComment[];
  dict: Dict;
  locale: Locale;
  relTime: (ts: string) => string;
  sectOn: (k: string) => boolean;
}) {
  return (
    <>
      {sectOn("thankers") && thanks.length > 0 && (
        <Thankers
          thanks={thanks}
          count={thanksCount}
          dict={dict}
          locale={locale}
        />
      )}

      {sectOn("infohash") && (
        <p className="break-all border-t border-line pt-2 text-[11px] text-sub">
          <span className="font-bold">{dict.torrent.infoHash}</span> ·{" "}
          {t.info_hash.trim()}
        </p>
      )}

      {sectOn("comments") && (
        <Comments
          comments={comments}
          torrentId={t.id}
          dict={dict}
          locale={locale}
          relTime={relTime}
        />
      )}
    </>
  );
}
