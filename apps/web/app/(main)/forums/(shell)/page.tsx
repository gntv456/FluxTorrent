import Link from "next/link";
import { getForumRecent } from "@/lib/data";
import { hasSessionToken } from "@/lib/session-token";
import { getDict } from "@/i18n/server";
import { ForumSearch } from "@/components/forum-search";
import { TopicComposer } from "@/components/forum-composer";
import {
  ForumHomeStream,
  type StreamType,
} from "@/components/forum-home-stream";

export const dynamic = "force-dynamic";

/** 论坛首页中栏：页头（搜索/关注流/发帖）+ 全站话题流。
 *
 *  左列用户卡/版块导航、右列统计与手机工具条都已上移到
 *  (shell)/layout.tsx——同一布局下的导航只替换本页，两翼不重取。
 *
 *  中栏从「分区→版块卡片目录」换成**话题流**是样图对齐的核心：
 *  中列是跨版块的最新主题列表（头像 + 标题 + 版块徽章 + 回复 /
 *  最后回复），版块目录的职能改由左栏导航承担。 */
export default async function ForumsPage({
  searchParams,
}: {
  searchParams: Promise<{ type?: string }>;
}) {
  // 字典与会话并发（模块守卫已由三栏壳统一处理）
  const dictP = getDict();
  const signedP = hasSessionToken();
  const { type: typeRaw } = await searchParams;
  // 白名单四档（对齐样图筛选行）：notice=公告 / sticky=置顶 / digest=精华；
  // 其余（含爬虫乱造的）一律回落全量。
  const type: StreamType =
    typeRaw === "notice" || typeRaw === "sticky" || typeRaw === "digest"
      ? typeRaw
      : undefined;
  const { dict } = await dictP;
  const signed = await signedP;
  const recent = signed ? await getForumRecent(type) : null;
  const searchBox = (
    <ForumSearch
      placeholder={dict.forums2.searchPh}
      button={dict.forums2.searchBtn}
    />
  );

  return (
    <>
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
    </>
  );
}
