import Link from "next/link";
import { getForumIndex, getForumSide, getForumUserCard } from "@/lib/data";
import { hasSessionToken } from "@/lib/session-token";
import { getDict } from "@/i18n/server";
import { ForumSearch } from "@/components/forum-search";
import { TopicComposer } from "@/components/forum-composer";
import { ForumSideColumn } from "@/components/forum-home-aside";
import {
  ForumLeftColumn,
  type BoardGroup,
} from "@/components/forum-left-column";
import { ForumMobileBar } from "@/components/forum-mobile-bar";
import {
  ForumMobileFilter,
  ForumMobileFilterLabel,
} from "@/components/forum-mobile-filter";
import { requireModule } from "@/components/module-gate";
/* 两份页面级样式都要：forums.css = 话题列表/规则条等中栏表面；
   forums-home.css = 三栏栅格与两翼。 */
import "@/app/styles/forums.css";
import "@/app/styles/forums-home.css";

export const dynamic = "force-dynamic";

/** 论坛三栏壳（首页与版块页共用）。
 *
 *  为什么要有这层 layout：/forums 与 /forums/[id] 此前各是一整页，
 *  版块导航点一下 ⇒ 左右两翼连同用户卡/统计一起重新取数、整块
 *  内容区换成骨架。把两翼收进路由组布局后，同一布局下的导航只
 *  替换 children（中栏），左列/右列实例保持挂载、不再重取。
 *
 *  当前版块高亮改由 ForumNav 按 pathname 在客户端现算——布局在
 *  切版块时不会重新渲染，服务端拿不到新 id。
 *
 *  手机工具条也放在壳里：它的「版块/更多/搜帖」弹层要复用左右两
 *  翼的同一份数据，放页面里会在每次切版块时重复取数；筛选弹层
 *  改为客户端组件，按当前 URL 现算链接与档名。 */
export default async function ForumsShellLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  const gate = await requireModule("forums");
  if (gate) return gate;
  const { dict, currency } = await getDict();
  // 会话先判、后取数：/forums/side 与 /me 都要凭证，游客打了只会
  // 被包成 null，于是「接口挂了」和「你还没登录」在界面上长得一样。
  const signed = await hasSessionToken();
  const [index, me, side] = await Promise.all([
    getForumIndex(),
    signed ? getForumUserCard() : Promise.resolve(null),
    signed ? getForumSide() : Promise.resolve(null),
  ]);
  // 分区是分组骨架：没有版块的分区照样成组（左栏渲染空态占位）
  const groups: BoardGroup[] = index.categories.map((c) => ({
    key: `cat-${c.id}`,
    name: c.name,
    hidden: !c.visible,
    items: index.forums.filter((f) => f.category_id === c.id),
  }));
  // 「未分组」= category_id 为空，或指向本次未返回的分区
  const orphans = index.forums.filter(
    (f) =>
      f.category_id == null ||
      !index.categories.some((c) => c.id === f.category_id),
  );
  if (orphans.length > 0) {
    groups.push({
      key: "cat-none",
      name: dict.forums.uncategorized,
      hidden: false,
      items: orphans,
    });
  }
  const totalTopics = index.forums.reduce((a, f) => a + f.topics, 0);
  const sideState = !signed ? "guest" : side ? "ok" : "fail";
  // 左列与搜索框的 RSC 节点：桌面原位 + 手机弹窗，两处复用同一份
  const leftColumn = (
    <ForumLeftColumn
      me={me}
      dict={dict}
      currency={currency}
      groups={groups}
      totalTopics={totalTopics}
    />
  );
  const searchBox = (
    <ForumSearch
      placeholder={dict.forums2.searchPh}
      button={dict.forums2.searchBtn}
    />
  );
  return (
    <div className="pting-forums">
      {/* 左列：用户卡 + 版块导航（≤768 隐藏，改由「版块」弹窗承载） */}
      <div className="fh-left">{leftColumn}</div>

      {/* 中栏：手机工具条 + 页面内容（children = 首页话题流或版块列表） */}
      <div className="pting-board">
        {/* 手机端（≤768）四按钮一排：筛选 / 版块 / 更多 / 搜帖 */}
        <ForumMobileBar
          filterLabel={<ForumMobileFilterLabel />}
          labels={{
            nav: dict.forums2.allNodes,
            more: dict.forums3.mbarMore,
            search: dict.forums2.searchBtn,
          }}
          titles={{
            filter: dict.forums3.mbarFilter,
            nav: dict.forums2.allNodes,
            more: dict.forums3.mbarMore,
            search: dict.forums2.searchBtn,
          }}
          panels={{
            filter: <ForumMobileFilter />,
            nav: leftColumn,
            more: (
              <>
                <div className="fh-macts">
                  <Link href="/forums/feed" className="fh-macts__a">
                    {dict.forums.feedTitle}
                  </Link>
                  <TopicComposer />
                </div>
                <ForumSideColumn side={side} state={sideState} dict={dict} />
              </>
            ),
            search: searchBox,
          }}
        />
        {children}
      </div>

      {/* 右列：论坛统计 / 今日热帖 / 最新回复 / 新手必读。
          游客与取档失败各自出声，不拿空列表冒充「今天没人发帖」。 */}
      <ForumSideColumn side={side} state={sideState} dict={dict} />
    </div>
  );
}
