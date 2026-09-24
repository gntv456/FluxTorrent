import { getDict } from "@/i18n/server";
import { getSiteProfile } from "@/lib/site-profile";

/**
 * 模块页守卫（U1 §6.3）：可选模块页面在开关关闭时渲染统一空态，
 * 不暴露模块内部结构。用法（RSC 页面顶部）：
 *
 *   const gate = await requireModule("games");
 *   if (gate) return gate;
 *
 * 缺键视为开（与 Header 的 mod() 口径一致：/site-profile 未返回该键 = 站点未配置 = 现状）。
 */
export async function requireModule(
  moduleKey: string,
): Promise<React.ReactNode | null> {
  const profile = await getSiteProfile();
  if (profile.modules[moduleKey] !== false) return null;
  const { dict } = await getDict();
  return (
    <div className="mx-auto max-w-xl px-4 py-16 text-center">
      <p className="text-4xl" aria-hidden>
        🚧
      </p>
      <h1 className="mt-4 text-lg font-semibold">
        {dict.mod.disabledTitle}
      </h1>
      <p className="mt-2 text-sm text-muted">
        {dict.mod.disabledBody}
      </p>
    </div>
  );
}
