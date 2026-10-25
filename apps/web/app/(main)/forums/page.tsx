import Link from "next/link";
import {
  getForumIndex,
  getForumRecent,
  getForumSide,
  getForumUserCard,
} from "@/lib/data";
import { hasSessionToken } from "@/lib/session-token";
import { getDict } from "@/i18n/server";
import { ForumSearch } from "@/components/forum-search";
import { TopicComposer } from "@/components/forum-composer";
import {
  ForumHomeStream,
  type StreamType,
} from "@/components/forum-home-stream";
import { ForumSideColumn } from "@/components/forum-home-aside";
import { ForumLeftColumn, type BoardGroup } from "@/components/forum-left-column";
import { ForumMobileBar } from "@/components/forum-mobile-bar";
import { requireModule } from "@/components/module-gate";
/* 页面专属样式走路由级 import（globals.css 尾注的纪律）：只有论坛首页
   需要三列与两翼的表面，不往全站 @import 链里堆。 */
import "@/app/styles/forums-home.css";

export const dynamic = "force-dynamic";

/** 论坛首页：三列（样图集 § tplForum）——左列用户卡 + 版块导航、
 * 中栏全站话题流、右列统计/热帖/最新回复/新手必读。
 *
 * 中栏从「分区→版块卡片目录」换成**话题流**是本次对齐样图的核心：
 * 样图中列就是跨版块的最新主题列表（头像 + 标题 + 版块徽章 + 回复 /
 * 最后回复），版块目录的职能改由左栏导航承担（全部版块 + 分组 +
 * 各版块带计数），与样图的信息架构一致。 */
export default async function ForumsPage({
  searchParams,
}: {
  searchParams: Promise<{ type?: string }>;
}) {
  // U1 模块页守卫 + 字典 + 会话三连并发（2026-10-18 主菜单切换提速：
  // 此前三者串行各一跳 API，RSC 数据段被拉长 ~2 个 RTT；守卫仅在
  // 模块关闭时才消费 dict，把 dict 取数挪到守卫命中分支之后不再阻塞）。
  const gateP = requireModule("forums");
  const dictP = getDict();
  const signedP = hasSessionToken();
  const { type: typeRaw } = await searchParams;
  // 白名单四档（对齐样图筛选行）：notice=公告 / sticky=置顶 / digest=精华；
  // 其余（含爬虫乱造的）一律回落全量。
  const type: StreamType =
    typeRaw === "notice" || typeRaw === "sticky" || typeRaw === "digest"
      ? typeRaw
      : undefined;
  const gate = await gateP;
  if (gate) return gate;
  const { dict, currency } = await dictP;
  // 会话先判、后取数：/forums/recent 与 /me 都要凭证，游客打了只会
  // 被包成 null，于是「接口挂了」和「你还没登录」在界面上长得一样。
  const signed = await signedP;
  const [{ categories, forums }, me, side, recent] = await Promise.all([
    getForumIndex(),
    signed ? getForumUserCard() : Promise.resolve(null),
    signed ? getForumSide() : Promise.resolve(null),
    signed ? getForumRecent(type) : Promise.resolve(null),
  ]);
  const sideState = !signed ? "guest" : side ? "ok" : "fail";

  // 分区是分组骨架：没有版块的分区照样成组（左栏渲染空态占位）
  const groups: BoardGroup[] = categories.map((c) => ({
    key: `cat-${c.id}`,
    name: c.name,
    hidden: !c.visible,
    items: forums.filter((f) => f.category_id === c.id),
  }));
  // 「未分组」= category_id 为空，或指向本次未返回的分区
  const orphans = forums.filter(
    (f) =>
      f.category_id == null || !categories.some((c) => c.id === f.category_id),
  );
  if (orphans.length > 0) {
    groups.push({
      key: "cat-none",
      name: dict.forums.uncategorized,
      hidden: false,
      items: orphans,
    });
  }
  const totalTopics = forums.reduce((a, f) => a + f.topics, 0);
  // 中栏三态：游客 / 取档失败 / 有数据（含真的空）——不拿空列表
  // 冒充「今天没人发帖」，也不拿取档失败冒充空。
  const stream = signed ? recent : null;

  // 手机端四档筛选（与桌面话题流同一组白名单；第三列是圆点色）
  const filterOpts: [StreamType, string, string][] = [
    [undefined, dict.forums.filterAll, "bg-sky"],
    ["notice", dict.forums.filterNotice, "bg-coral"],
    ["sticky", dict.forums.filterSticky, "bg-indigo"],
    ["digest", dict.forums.filterDigest, "bg-sun"],
  ];
  const filterLabel = (filterOpts.find(([v]) => v === type) ??
    filterOpts[0])[1];
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

      {/* 中栏：页头（搜索/关注流/发帖）+ 手机工具条 + 话题流 */}
      <div className="pting-board">
        {/* 无障碍标题放头部之外：手机端整条页头隐藏（搜索/关注流/发帖都进了
            弹窗，留着只剩一个空色块），但 h1 仍要在无障碍树里 */}
        <h1 className="sr-only">{dict.forums.title}</h1>
        <header className="pting-board__head">
          <span className="fh-head__search">{searchBox}</span>
          <span className="fh-head__feed">
            <Link href="/forums/feed" className="pting-feed-pill">
              {dict.forums.feedTitle}
            </Link>
          </span>
          <span className="fh-head__new">
            <TopicComposer />
          </span>
        </header>

        {/* 手机端（≤768）四按钮一排：筛选 / 版块 / 更多 / 搜帖 */}
        <ForumMobileBar
          filterLabel={filterLabel}
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
            filter: (
              <div className="fh-pick">
                {filterOpts.map(([v, label, dot]) => (
                  <Link
                    key={v ?? "all"}
                    href={v ? `/forums?type=${v}` : "/forums"}
                    className={
                      "fh-pick__row" + (type === v ? " on" : undefined)
                    }
                    aria-current={type === v ? "true" : undefined}
                  >
                    <span
                      className={"fh-pick__dot " + dot}
                      aria-hidden="true"
                    />
                    {label}
                  </Link>
                ))}
              </div>
            ),
            nav: leftColumn,
            more: (
              <>
                {/* 用户要求：关注流 / 发布主题 两个按钮放在右列弹窗的上面 */}
                <div className="fh-macts">
                  <Link href="/forums/feed" className="fh-macts__a">
                    {dict.forums.feedTitle}
                  </Link>
                  <TopicComposer />
                </div>
                <ForumSideColumn
                  side={side}
                  state={sideState}
                  dict={dict}
                />
              </>
            ),
            search: searchBox,
          }}
        />

        {!signed ? (
          /* 游客：话题流与右栏热帖一样要登录——说清楚，别摆空列表 */
          <div className="fh-stream__guest">
            <b>{dict.forums3.guestTitle}</b>
            <p>{dict.forums3.guestDesc}</p>
            <Link href="/login" className="fh-guide__cta">
              {dict.common.login}
            </Link>
          </div>
        ) : recent === null ? (
          /* 取档失败：与「真的空」分开说 */
          <div className="fh-stream__guest">
            <b>{dict.forums3.failTitle}</b>
            <p>{dict.forums3.failDesc}</p>
          </div>
        ) : (
          <ForumHomeStream
            topics={recent.topics ?? []}
            type={type}
            dict={dict}
          />
        )}
      </div>

      {/* 右列：论坛统计 / 今日热帖 / 最新回复 / 新手必读。
          游客与取档失败各自出声，不拿空列表冒充「今天没人发帖」。 */}
      <ForumSideColumn side={side} state={sideState} dict={dict} />
    </div>
  );
}
