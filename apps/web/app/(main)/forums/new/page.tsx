import { redirect } from "next/navigation";

/**
 * 发主题选版块页已退役：选版块并入发帖弹窗（论坛首页/版块页入口），
 * 旧链接重定向回论坛首页。
 *
 * 纯转发路由（同 /users、/farm 口径）：redirect 静态可判，无需
 * force-dynamic，也不属数据路由——loading 骨架无意义。
 */
export default function NewTopicPage() {
  redirect("/forums");
}
