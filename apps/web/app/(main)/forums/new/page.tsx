import { TopicComposer } from "@/components/forum-composer";
import { getDict } from "@/i18n/server";

/** 发新主题入口（2026-11-20 五问评审 P1）：此前这里只 redirect 回论坛
 *  首页，但论坛首页的发帖入口是**弹窗触发器**，用户点"发新主题"被甩回
 *  首页后还得自己再找一遍按钮。改成与版块页同构：直接渲染发帖弹窗宿主，
 *  由页面 loading 自动打开（TopicComposer 无 forumId 时自带版块选择器）。 */
export default async function NewTopicPage() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="sr-only">{dict.forums2.searchGoPost}</h1>
      <TopicComposer autoOpen />
    </div>
  );
}
