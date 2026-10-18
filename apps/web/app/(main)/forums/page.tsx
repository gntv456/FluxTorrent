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
import {
  ForumMineLinks,
  ForumSideColumn,
  ForumUserCardBlock,
} from "@/components/forum-home-aside";
import { requireModule } from "@/components/module-gate";
/* 页面专属样式走路由级 import（globals.css 尾注的纪律）：只有论坛首页
   需要三列与两翼的表面，不往全站 @import 链里堆。 */
import "@/app/styles/forums-home.css";

export const dynamic = "force-dynamic";

/** 左栏版块导航的圆点色板（样图 f3b-nav__item i 的六色，按版块 id 取） */
const NAV_DOTS = [
  "#2fa8ff",
  "#2fbf9b",
  "#ffc93c",
  "#5b6bf5",
  "#ff8fc7",
  "#ff7a59",
];

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
  const type: StreamType =
    typeRaw === "sticky" || typeRaw === "digest" ? typeRaw : undefined;
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
  const groups = categories.map((c) => ({
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

  return (
    <div className="pting-forums">
      {/* 左列：用户卡（样图第一块）+ 版块导航（全部版块 + 分组 + 各版块计数） */}
      <div className="fh-left">
        <ForumUserCardBlock me={me} dict={dict} currency={currency} />
        <aside className="pting-sidenav">
          <h2>{dict.forums2.allNodes}</h2>
          <nav aria-label={dict.forums.title}>
            <Link href="/forums" className="on">
              <span
                className="pting-sidenav__dot"
                style={{ background: "var(--sky)" }}
                aria-hidden="true"
              />
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
                    className={g.hidden ? "fh-nav__hidden" : undefined}
                  >
                    <span
                      className="pting-sidenav__dot"
                      style={{
                        background: NAV_DOTS[f.id % NAV_DOTS.length],
                      }}
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
      </div>

      {/* 中栏：页头（搜索/标题/收录数/关注流/发帖）+ 全站话题流 */}
      <div className="pting-board">
        <header className="pting-board__head">
          <ForumSearch
            placeholder={dict.forums2.searchPh}
            button={dict.forums2.searchBtn}
          />
          <div>
            <h1 className="pting-board__title">{dict.forums.title}</h1>
            <p>
              {dict.forums2.totalPrefix}
              {totalTopics}
              {dict.forums2.totalSuffix}
            </p>
          </div>
          <Link href="/forums/feed" className="pting-feed-pill">
            {dict.forums.feedTitle}
          </Link>
          <TopicComposer />
        </header>

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
