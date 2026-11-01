import { LoginShell } from "./login-view";
/* 手机版认证页样式（≤640px 的品牌带 + 磨砂抽屉），与 (auth) 组共用一份 */
import "@/app/styles/auth-mobile.css";

/**
 * 登录页（设计稿：参考站 2170×1180 artwork 双栏画布复刻）。
 * RSC 页面外壳；图标/表单/语言切换/品牌数据/双栏画布已按域拆出
 * ./login-view（client component）。
 */
export default function LoginPage() {
  return <LoginShell />;
}
