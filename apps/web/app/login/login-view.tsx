"use client";

import { Suspense, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt, LOCALES, LOCALE_COOKIE, type Locale } from "@/i18n/config";
import { ThemeToggle } from "@/components/theme-toggle";
import { Icon } from "@/components/icons";
import { LoginForm } from "./login-form";

/** 登录页画布外壳（从 app/login/page.tsx 按域拆出，来源文件是 server
 *  component 页面；本文件含 hooks 故为 client）：
 *  语言下拉 + 品牌数据 + 双栏画布（左品牌栏/右上语言行/右表单卡）。
 *  表单本体与 SVG 图标再拆 ./login-form。 */

const LANG_LABELS: Record<Locale, string> = {
  "zh-CN": "简体中文",
  "zh-TW": "繁體中文",
  en: "English",
  ja: "日本語",
};

/** 语言下拉（登录画布右上角，与站内切换器共用 cookie 语义） */
function LoginLangSwitcher({ current }: { current: Locale }) {
  const router = useRouter();
  return (
    <label className="bz-lang">
      <select
        value={current}
        onChange={(e) => {
          const l = e.target.value;
          if (!LOCALES.includes(l as Locale)) return;
          document.cookie = `${LOCALE_COOKIE}=${l}; path=/; max-age=31536000; samesite=lax`;
          router.refresh();
        }}
      >
        {LOCALES.map((l) => (
          <option key={l} value={l}>
            {LANG_LABELS[l]}
          </option>
        ))}
      </select>
    </label>
  );
}

/** 品牌区数据（0143 站型化）：tagline/logo 由站点档案下发（站型包默认 + 后台可覆盖），
 *  拉取失败回落 i18n 字典（原教育站文案），保证登录页永不因后端故障而空版。 */
function useSiteBrand() {
  const { dict } = useI18n();
  const [brand, setBrand] = useState<{
    tagline: string;
    desc: string;
    logo: string | null;
  }>({
    tagline: dict.login.introTagline,
    desc: dict.login.introDesc,
    logo: null,
  });
  useEffect(() => {
    let alive = true;
    api
      .get<{
        tagline?: string;
        site_logo?: string | null;
        site_desc?: string | null;
      }>("/api/v1/site-profile")
      .then((p) => {
        if (!alive) return;
        setBrand({
          tagline: p.tagline?.trim() || dict.login.introTagline,
          desc: p.site_desc?.trim() || dict.login.introDesc,
          logo: p.site_logo?.trim() || null,
        });
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [dict]);
  return brand;
}

/** 登录页外壳（双栏画布 + 右上语言行 + 表单卡） */
export function LoginShell() {
  const { dict, locale } = useI18n();
  const brand = useSiteBrand();
  return (
    <div className="bz-login-page">
      <div className="bz-login-canvas">
        {/* 左：品牌插画栏（奶油底 + 站标 + 标语）——站点 logo 优先，缺省占位图形 */}
        <div className="bz-login-intro">
          <div className="bz-login-intro-inner">
            {brand.logo ? (
              // eslint-disable-next-line @next/next/no-img-element
              <img
                src={brand.logo}
                alt=""
                aria-hidden
                className="bz-login-intro-mascot bz-login-intro-logo"
              />
            ) : (
              <span className="bz-login-intro-mascot" aria-hidden>
                <Icon name="seed" size={64} className="text-[var(--sky)]" />
              </span>
            )}
            <p className="bz-login-intro-tagline">{brand.tagline}</p>
            <p className="bz-login-intro-desc">{brand.desc}</p>
          </div>
        </div>

        {/* 右上：语言行 */}
        <div className="bz-login-lang-row">
          <div className="flex items-center gap-2">
            <ThemeToggle className="min-h-0" label={dict.common.themeToggle} />
            <LoginLangSwitcher current={locale} />
          </div>
        </div>

        {/* 右：表单卡 */}
        <div className="bz-login-card">
          <header className="bz-login-header">
            <h2>{dict.login.loginTitle}</h2>
          </header>
          <Suspense fallback={<div className="bz-form-skeleton" aria-hidden />}>
            <LoginForm />
          </Suspense>
          <p className="bz-login-notice">
            {dict.login.cookieNote}
            <br />
            {fmt(dict.login.failBanNote, { n: 5 })} {dict.login.inviteOnly}
          </p>
        </div>
      </div>
    </div>
  );
}
