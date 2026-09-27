import Link from "next/link";
import { getDict } from "@/i18n/server";
import { getSiteProfile } from "@/lib/site-profile";
import { Icon } from "@/components/icons";

/** 认证组布局（M5.1）：register/forgot/reset 共用（login 保留自己的
 *  双栏画布 bz-login-canvas，不进本组——见概念稿 §13 认证页补设计）。
 *  设计口径：认证页不进 (main) 组（未登录点底 Tab 只会弹回登录），
 *  但也不再是裸 div——统一「品牌头 + 居中卡片 + 返回首页」极简壳；
 *  手机单列居中，≥md 卡片化，三档自适应。 */
export default async function AuthLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  const { dict } = await getDict();
  const profile = await getSiteProfile().catch(() => null);
  const brand = profile?.brand || dict.common.brand;
  return (
    <div className="auth-shell">
      <header className="auth-shell__head">
        <Link href="/" className="auth-shell__brand" title={brand}>
          <Icon name="seed" size={26} className="shrink-0 text-[var(--sky)]" />
          <span className="truncate font-display text-xl text-ink">
            {brand}
          </span>
        </Link>
      </header>
      <main className="auth-shell__body">{children}</main>
      <footer className="auth-shell__foot">
        <Link href="/" className="auth-shell__home">
          ← {dict.nav.home}
        </Link>
        <span className="auth-shell__sep" aria-hidden>
          ·
        </span>
        <Link href="/ban-log" className="auth-shell__minor">
          {dict.login.bannedLog}
        </Link>
        <span className="auth-shell__sep" aria-hidden>
          ·
        </span>
        <Link href="/appeals" className="auth-shell__minor">
          {dict.login.appealLink}
        </Link>
      </footer>
    </div>
  );
}
