/**
 * 详情页尾部三段（E6 拆出：感谢者 / InfoHash / 评论区；300 行门禁）。
 * 段落显隐走 sectOn（view_hidden.sections），数据态条件保留在调用侧。
 */

import { Comments, Thankers } from "@/components/torrent-detail-blocks";
import { FoldMobile } from "@/components/fold-mobile";
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
  const cmtN = String(comments.length);
  const thankersOn = sectOn("thankers") && thanks.length > 0;
  // 本地完善版：InfoHash 并入感谢者折叠卡底部；无感谢者时独立成段
  const infoHash = sectOn("infohash") ? (
    <p className="td-infohash">
      <b>{dict.torrent.infoHash}</b> · {t.info_hash.trim()}
    </p>
  ) : null;
  return (
    <>
      {thankersOn && (
        <Thankers
          thanks={thanks}
          count={thanksCount}
          dict={dict}
          locale={locale}
          footer={infoHash}
        />
      )}

      {!thankersOn && infoHash}

      {sectOn("comments") && (
        <FoldMobile
          title={dict.torrent.commentsTitle.replace("{n}", cmtN)}
          id="td-sec-comments"
        >
          <Comments
            comments={comments}
            torrentId={t.id}
            dict={dict}
            locale={locale}
            relTime={relTime}
            embedded
          />
        </FoldMobile>
      )}
    </>
  );
}
