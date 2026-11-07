import { LoginShell } from "./login-view";
import { getSiteProfile } from "@/lib/site-profile";
/* 手机版认证页样式（≤640px 的品牌带 + 磨砂抽屉），与 (auth) 组共用一份 */
import "@/app/styles/auth-mobile.css";

/**
 * 登录页（设计稿：参考站 2170×1180 artwork 双栏画布复刻）。
 * RSC 页面外壳；图标/表单/语言切换/品牌数据/双栏画布已按域拆出
 * ./login-view（client component）。
 *
 * 站名/logo 由服务端取档后作为初始值传入：此前品牌区只在客户端
 * effect 里拉 site-profile，SSR 首屏必然是 i18n 默认的 FluxTorrent，
 * 接口失败（静默 catch）就一直不换——站长配了站名也像没配。
 */
export default async function LoginPage() {
  const profile = await getSiteProfile().catch(() => null);
  return (
    <LoginShell
      brandName={profile?.brand}
      brandLogo={profile?.site_logo}
    />
  );
}
