import { redirect } from "next/navigation";

export const dynamic = "force-dynamic";

/** 发主题选版块页已退役：选版块并入发帖弹窗（论坛首页/版块页入口），
 *  旧链接重定向回论坛首页。 */
export default function NewTopicPage() {
  redirect("/forums");
}
