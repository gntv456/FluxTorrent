import { ForumMineLinks, ForumUserCardBlock } from "./forum-home-aside";
import { ForumNav, type BoardGroup } from "./forum-nav";
import type { ForumUserCard } from "@/lib/data-forum";
import type { Dict } from "@/i18n/server";

/** 论坛左列（样图集 § tplForum 左栏三块）：用户卡 + 版块导航 + 我的。
 *
 *  抽成独立组件是因为它要出现两次：桌面落在 `.fh-left` 列里，
 *  手机端放进「版块」弹窗（见 forum-mobile-bar.tsx）——同一份 RSC
 *  节点两处复用，不复制 JSX。
 *
 *  当前版块高亮由 ForumNav 在客户端按 pathname 现算：本组件活在
 *  三栏壳 layout 里，切版块时不会重新渲染。 */

export type { BoardGroup };

export function ForumLeftColumn({
  me,
  dict,
  currency,
  groups,
  totalTopics,
}: {
  me: ForumUserCard | null;
  dict: Dict;
  currency: string;
  groups: BoardGroup[];
  totalTopics: number;
}) {
  return (
    <>
      <ForumUserCardBlock me={me} dict={dict} currency={currency} />
      <ForumNav
        groups={groups}
        totalTopics={totalTopics}
        labels={{
          title: dict.forums.title,
          allNodes: dict.forums2.allNodes,
          navAll: dict.forums2.navAll,
          hiddenTag: dict.forums2.hiddenTag,
        }}
      />
      <ForumMineLinks dict={dict} />
    </>
  );
}
