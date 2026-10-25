import Link from "next/link";
import type { Forum } from "@fluxtorrent/domain-types";
import { ForumMineLinks, ForumUserCardBlock } from "./forum-home-aside";
import type { ForumUserCard } from "@/lib/data-forum";
import type { Dict } from "@/i18n/server";

/** 论坛首页左列（样图集 § tplForum 左栏三块）：用户卡 + 版块导航 + 我的。
 *
 *  抽成独立组件是因为它要出现两次：桌面落在 `.fh-left` 列里，
 *  手机端放进「版块」弹窗（见 forum-mobile-bar.tsx）——同一份 RSC
 *  节点两处复用，不复制 JSX。
 */

/** 版块导航圆点色板（样图 f3b-nav__item i 的六色，按版块 id 取） */
const NAV_DOTS = [
  "#2fa8ff",
  "#2fbf9b",
  "#ffc93c",
  "#5b6bf5",
  "#ff8fc7",
  "#ff7a59",
];

export interface BoardGroup {
  key: string;
  name: string;
  hidden: boolean;
  items: Forum[];
}

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
      <aside className="pting-sidenav">
        <h2>{dict.forums2.allNodes}</h2>
        <nav aria-label={dict.forums.title}>
          {/* 一级：全部版块（样图该项没有圆点，选中态 sky-soft） */}
          <Link href="/forums" className="on">
            {dict.forums2.navAll}
            <span className="fh-nav__cnt num">{totalTopics}</span>
          </Link>
          {groups.map((g) => (
            <div key={g.key}>
              <div className="fh-nav__group">
                {g.name}
                {g.hidden && (
                  <span className="pting-card__count">
                    {dict.forums2.hiddenTag}
                  </span>
                )}
              </div>
              {g.items.map((f) => (
                <Link
                  key={f.id}
                  href={`/forums/${f.id}`}
                  className={
                    "fh-nav__sub " +
                    (g.hidden ? "fh-nav__hidden" : undefined)
                  }
                >
                  {/* 二级圆点（样图 f3b-nav__item i：6px 实心圆，
                      按版块 id 取色板；pting-sidenav__dot 是 26px
                      渐变方块，给字母用的，这里不能复用） */}
                  <span
                    className="fh-nav__dot"
                    style={{ background: NAV_DOTS[f.id % NAV_DOTS.length] }}
                    aria-hidden="true"
                  />
                  {f.name}
                  <span className="fh-nav__cnt num">{f.topics}</span>
                </Link>
              ))}
            </div>
          ))}
        </nav>
      </aside>
      <ForumMineLinks dict={dict} />
    </>
  );
}
