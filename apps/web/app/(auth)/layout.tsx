import { getDict } from "@/i18n/server";

/**
 * 认证组布局（M5.2，2026-10-18 对齐样图 § 账户与认证）。
 *
 * 版式由每页的 AuthCanvas 自持（左深色 Hero + 右上语言/主题 + 右白卡）：
 * 画布的标题/标语/提示条是逐页内容，放在 layout 里拿不到，故这里只透传
 * children。旧的「品牌头 + 居中窄表 + 底部快捷入口」壳（.auth-shell）已退役，
 * 与 /login 的双栏画布合并成同一套版式。 */
export default async function AuthLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  // getDict 预热字典缓存：子页（client）首帧即拿到当前语言，无闪烁
  await getDict();
  // main 锚点在此（每路由组恰一个）：AuthCanvas 只负责画布与卡片。
  return <main id="main-content">{children}</main>;
}
