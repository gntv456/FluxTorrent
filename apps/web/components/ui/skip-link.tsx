import { getDict } from "@/i18n/server";

/** 跳主内容链接（DS-PLAN-01 W-04 / §3.5：键盘与读屏基建第一件）。
 *
 *  全站唯一：挂根布局 body 首位，`Tab` 第一站即本链接；目标锚点
 *  `<main id="main-content">` 由各路由组布局保证（(main)/(auth)/login）。
 *  视觉隐藏、聚焦时滑入左上角（CSS .ui-skip-link，见 ui.css）。
 *
 *  服务端组件：文案走 getDict()（i18n 门禁不允许 tsx 内 CJK 字面量）。
 */
export async function SkipLink() {
  const { dict } = await getDict();
  return (
    <a href="#main-content" className="ui-skip-link">
      {dict.ui.skipToMain}
    </a>
  );
}
