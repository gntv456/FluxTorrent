/** 详情页字典切片（RSC 瘦身 2026-11-12）：详情页的 client 边界组件
 *  （锚点条/同组版本/感谢者/评论区）此前 prop 收整本 Dict——React 序列化
 *  按运行时值不按 TS 类型，整本字典（bank/invites/getrss… 与详情页无关
 *  的 131KB）被塞进 /torrent/{id} 的 flight payload（占该页 RSC 60%）。
 *  收窄为仅消费的键后，服务端传切片、序列化只含所需文案（实测 gz
 *  142KB→33KB）。类型与组装函数独立成件：detail-blocks 已顶 300 行
 *  门禁；每消费方一个专用函数（键拼错编译期红），调用侧一行。 */

import type { Dict } from "@/i18n/zh-CN";

/** 详情页交互区块（同组版本/感谢者/评论区）消费的切片形状。 */
export type DetailBlocksDict = Pick<
  Dict["torrent"],
  | "anonymous"
  | "commentsTitle"
  | "noComments"
  | "official"
  | "replyTo"
  | "thankersTitle"
  | "commentBusy"
  | "commentsEnd"
  | "commentsMore"
> &
  Pick<
    Dict["tdetail"],
    "groupTitle" | "current" | "justNow" | "hourUnit" | "dayUnit" | "agoUnit"
  >;

/** 同组版本表（GroupVersions）：只消费三键，prop 用本 Pick 收窄。 */
export type GroupDict = Pick<
  DetailBlocksDict,
  "official" | "groupTitle" | "current"
>;

/** 同组版本表（GroupVersions）：三键切片。 */
export function groupDict(dict: Dict): GroupDict {
  return {
    official: dict.torrent.official,
    groupTitle: dict.tdetail.groupTitle,
    current: dict.tdetail.current,
  };
}

/** 尾部（感谢者/评论区，TorrentDetailTail）：全部消费键的切片。 */
export function tailDict(dict: Dict): DetailBlocksDict {
  return {
    anonymous: dict.torrent.anonymous,
    commentsTitle: dict.torrent.commentsTitle,
    noComments: dict.torrent.noComments,
    official: dict.torrent.official,
    replyTo: dict.torrent.replyTo,
    thankersTitle: dict.torrent.thankersTitle,
    commentBusy: dict.torrent.commentBusy,
    commentsEnd: dict.torrent.commentsEnd,
    commentsMore: dict.torrent.commentsMore,
    groupTitle: dict.tdetail.groupTitle,
    current: dict.tdetail.current,
    justNow: dict.tdetail.justNow,
    hourUnit: dict.tdetail.hourUnit,
    dayUnit: dict.tdetail.dayUnit,
    agoUnit: dict.tdetail.agoUnit,
  };
}

/** 移动端锚点条（TorrentAnchorBar）消费的切片形状。 */
export type AnchorDict = Pick<
  Dict["torrent"],
  "descrTitle" | "filesTitle" | "commentsTitle" | "anchorNav"
> &
  Pick<Dict["subtitles"], "panelTitle">;

/** 锚点条切片：5 枚文案。 */
export function anchorDict(dict: Dict): AnchorDict {
  return {
    descrTitle: dict.torrent.descrTitle,
    filesTitle: dict.torrent.filesTitle,
    commentsTitle: dict.torrent.commentsTitle,
    anchorNav: dict.torrent.anchorNav,
    panelTitle: dict.subtitles.panelTitle,
  };
}
