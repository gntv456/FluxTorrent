import Link from "next/link";
import { getFeed, getMyFollows } from "@/lib/data";
import { getMyForumActivity } from "@/lib/data-forum";
import { getDict } from "@/i18n/server";
import { requireModule } from "@/components/module-gate";
import { PageHeader } from "@/components/ui/page-header";
import { TabPanel, Tabs } from "@/components/ui/tabs";
import { FeedLatestPanel } from "./_parts/latest-panel";
import { FollowSide } from "./_parts/follow-side";
import { MyRepliesPanel, MyTopicsPanel } from "./_parts/mine-panels";
import "@/app/styles/forums.css";

export const dynamic = "force-dynamic";

/**
 * 关注流（0121）+ 我参与的（2026-10-17 补齐样图 § forum-feed）。
 *
 * 三个标签都是 SSR 链接（`?tab=`），不引入客户端状态：
 *   latest  = 我关注的版块/作者的新主题 + 我关注主题的最新回复（游标翻页）
 *   mine    = 我发的主题（页码翻页）
 *   replies = 我发的回复（页码翻页）
 * 侧栏是「我关注的版块 / 作者」两张可取关的列表——原来只有一行计数，
 * 想取消关注得先自己猜去哪一页上点。
 */
export default async function FeedPage({
  searchParams,
}: {
  searchParams: Promise<{
    before?: string;
    tab?: string;
    page?: string;
  }>;
}) {
  const { before, tab: tabRaw, page: pageRaw } = await searchParams;
  const gate = await requireModule("forums");
  if (gate) return gate;
  const { dict, locale } = await getDict();
  const t = dict.forums;
  const t3 = dict.forums3;
  // 标签白名单：爬虫乱造的值回落「关注动态」，不拿 404 打断浏览
  const tab = tabRaw === "mine" || tabRaw === "replies" ? tabRaw : "latest";
  const pageNum = Number(pageRaw);
  const page =
    Number.isFinite(pageNum) && pageNum > 1 ? Math.floor(pageNum) : 1;
  // 三条腿各自失败各自出声（Promise.all 不带崩整页）：
  // getFeed / getMyForumActivity 失败回 null，getMyFollows 内部已回落空
  const [feed, mine, activity] = await Promise.all([
    tab === "latest"
      ? getFeed(50, before || undefined)
      : Promise.resolve(null),
    getMyFollows(),
    tab === "latest"
      ? Promise.resolve(null)
      : getMyForumActivity(tab === "mine" ? "topics" : "replies", page),
  ]);

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        eyebrow="Following"
        title={t.feedTitle}
        sub={t3.feedDesc}
        aside={
          <Link href="/forums" className="text-sky hover:underline">
            {t.backToForums}
          </Link>
        }
      />

      <div className="ff-grid">
        <div className="ff-main">
          <Tabs
            idPrefix="feed"
            label={t3.tabAria}
            value={tab}
            items={[
              { key: "latest", label: t3.tabFeed },
              { key: "mine", label: t3.tabMyTopics },
              { key: "replies", label: t3.tabMyReplies },
            ]}
            hrefs={[
              "/forums/feed",
              "/forums/feed?tab=mine",
              "/forums/feed?tab=replies",
            ]}
          />
          {tab === "latest" && (
            <TabPanel idPrefix="feed" tabKey="latest">
              <FeedLatestPanel
                feed={feed}
                before={before}
                dict={dict}
                locale={locale}
              />
            </TabPanel>
          )}
          {tab === "mine" && (
            <TabPanel idPrefix="feed" tabKey="mine">
              <MyTopicsPanel data={activity} dict={dict} locale={locale} />
            </TabPanel>
          )}
          {tab === "replies" && (
            <TabPanel idPrefix="feed" tabKey="replies">
              <MyRepliesPanel data={activity} dict={dict} locale={locale} />
            </TabPanel>
          )}
        </div>

        <FollowSide mine={mine} dict={dict} />
      </div>
    </div>
  );
}
