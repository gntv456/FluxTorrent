import { formatBytes } from "@/lib/format";
import { Fold } from "@/components/torrent-detail-parts";
import { GroupSubscribeButton } from "@/components/group-subscribe-button";
import { CommentDeleteButton } from "@/components/comment-delete-button";
import { CommentLikeButton } from "@/components/comment-like-button";
import { CommentReplyButton } from "@/components/comment-reply-button";
import { dateLocale, type Locale } from "@/i18n/config";
import type { Dict } from "@/i18n/server";
import type { TorrentComment } from "@fluxtorrent/domain-types";

/** 种子详情页交互区块（从 app/(main)/torrent/[id]/page.tsx 按域拆出）：
 *  详情扩展数据类型、同组版本表、感谢者名单、评论区。数据装载留在 page.tsx。 */

/** 详情扩展数据（/detail 接口；descr/MediaInfo/views 等非列表字段） */
export interface TorrentDetailExt {
  descr: string | null;
  numfiles: number;
  thanks_count: number;
  bookmark_count: number;
  last_action: string | null;
  views: number;
  price: number;
  purchased: boolean;
  is_owner: boolean;
  /** 动态属性（0085/0087；B2 六类型）：kind → { dict_id, name, label, sort,
   *  field_type, values[] }。`name` = 首个展示值（向后兼容），多值看 `values`。 */
  sections?: Record<
    string,
    {
      dict_id: number | null;
      name: string;
      label: string;
      sort: number;
      field_type?: string;
      values?: string[];
    }
  >;
  /** MediaInfo 全文（发布表单录入，折叠块展示） */
  mediainfo?: string | null;
  /** 推荐位回显（0184 编辑表单）：置顶位置/截止/推荐影片 */
  pos_state?: number;
  pos_state_until?: string | null;
  pick_type?: number;
}

/** 聚合组（0069）：同一资源的多个版本 */
export interface GroupInfo {
  group: { id: number; name: string; descr: string | null } | null;
  items: Array<{
    id: number;
    name: string;
    small_descr: string | null;
    size: number;
    seeders: number;
    leechers: number;
    times_completed: number;
    official: boolean;
    current: boolean;
  }>;
}

export interface ThankItem {
  username: string | null;
  created_at: string;
}

/** 同组版本（0069 聚合组：同一资源的多个年份/版本/清晰度） */
export function GroupVersions({
  group,
  dict,
}: {
  group: GroupInfo;
  dict: Dict;
}) {
  const d = dict.tdetail;
  return (
    <Fold
      title={`${d?.groupTitle}：${group.group?.name}`}
      count={group.items.length}
    >
      {/* 组订阅（0075）：新版本过审时通知 */}
      <div className="mb-2 flex justify-end">
        <GroupSubscribeButton groupId={group.group!.id} />
      </div>
      <table className="td-files">
        <tbody>
          {group.items.map((g) => (
            <tr key={g.id} className={g.current ? "font-bold" : undefined}>
              <td className="min-w-0 truncate">
                {g.current ? (
                  <span title={g.name}>{g.name}</span>
                ) : (
                  <a href={`/torrent/${g.id}`} className="hover:underline">
                    {g.name}
                  </a>
                )}
                {g.official && (
                  <span className="sticker bg-indigo text-white">
                    {dict.torrent.official}
                  </span>
                )}
              </td>
              <td className="num shrink-0 text-right text-sub">
                {g.current ? (
                  (d?.current)
                ) : (
                  <>
                    <span className="text-green">{g.seeders}</span> ·{" "}
                    {formatBytes(g.size)}
                  </>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </Fold>
  );
}

/** 感谢者（馒头口径：头像占位 + 用户名） */
export function Thankers({
  thanks,
  count,
  dict,
  locale,
}: {
  thanks: ThankItem[];
  count: number;
  dict: Dict;
  locale: Locale;
}) {
  return (
    <Fold title={dict.torrent.thankersTitle} count={count}>
      <p className="flex flex-wrap gap-2">
        {thanks.map((th, i) => (
          <span
            key={i}
            className="td-thanker"
            title={new Date(th.created_at).toLocaleString(dateLocale(locale))}
          >
            <span className="td-thanker__avatar" aria-hidden>
              {(th.username ?? "?").slice(0, 1).toUpperCase()}
            </span>
            {th.username ?? dict.torrent.anonymous}
          </span>
        ))}
      </p>
    </Fold>
  );
}

/** 评论区（staff 删评按钮常显，无权限由后端 403 兜底；
 *  0156 嵌套回复：回复挂根楼层、组内缩进一层平铺 + 「回复 @xxx」标注） */
export function Comments({
  comments,
  torrentId,
  dict,
  locale,
  relTime,
}: {
  comments: TorrentComment[];
  torrentId: number;
  dict: Dict;
  locale: Locale;
  relTime: (iso: string) => string;
}) {
  return (
    <section className="td-comments nexus-detail">
      <h2 className="td-sec-title">
        {dict.torrent.commentsTitle.replace("{n}", String(comments.length))}
      </h2>
      {comments.map((c) => (
        <div
          key={c.id}
          className={`td-comment ${c.parent_id ? "td-comment--reply" : ""}`}
        >
          <div className="td-comment__side">
            <span className="td-comment__avatar" aria-hidden>
              {(c.username ?? "?").slice(0, 1).toUpperCase()}
            </span>
            <p className="text-sm font-bold">
              {c.username ?? dict.torrent.anonymous}
            </p>
            <p
              className="mt-1 text-[11px] text-sub"
              title={new Date(c.created_at).toLocaleString(dateLocale(locale))}
            >
              {relTime(c.created_at)}
            </p>
            {/* staff 删评：按钮常显，无权限由后端 403 兜底 */}
            <CommentDeleteButton torrentId={torrentId} commentId={c.id} />
          </div>
          <p className="td-comment__body">
            {c.reply_to_user && (
              <span className="td-comment__replyto">
                {dict.torrent.replyTo.replace("{user}", c.reply_to_user)}
              </span>
            )}
            {c.body}
          </p>
          <div className="td-comment__ops">
            <CommentReplyButton
              target={{
                torrentId,
                targetId: c.parent_id ?? c.id,
                targetUser: c.username ?? "",
              }}
            />
            <CommentLikeButton
              torrentId={torrentId}
              commentId={c.id}
              initialLikes={c.likes ?? 0}
              initialLiked={Boolean(c.liked_by_me)}
            />
          </div>
        </div>
      ))}
      {comments.length === 0 && (
        <p className="py-4 text-center text-sub">{dict.torrent.noComments}</p>
      )}
    </section>
  );
}
