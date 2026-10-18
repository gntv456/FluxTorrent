"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { ThemeToggle } from "@/components/theme-toggle";
import { LocaleSwitcher } from "@/components/locale-switcher";

/**
 * 认证页共用画布（2026-10-18，对齐样图集 § 账户与认证）。
 *
 * 样图七页（登录 / 注册 / 忘记密码 / 重置密码 / 邮箱验证 / 自助解封 /
 * 邀请申请）共用同一版式：
 *   - 左：深色 Hero —— 站标 + 站名 + 本页标语 + 说明（--grad-aurora，
 *     与 .up-hero 同源的深海军 → 主蓝 → 亮蓝；前景走 --on-dark*）；
 *   - 右上：主题开关 + 语言下拉（与站内切换器共用 cookie 语义）；
 *   - 右：白卡 —— 品牌行 + 标题 + 说明 + 状态徽章 + 提示条 + 表单 +
 *     辅助链接 + 页脚。卡片本体复用登录画布的 .bz-login-card（栅格 /
 *     吸顶语言行 / 滚动 / 三档响应式规则同源，见 layout.css）。
 *
 * 为什么抽成组件：对齐前只有 /login 是双栏画布，其余六页是「品牌头 +
 * 居中窄表」，同组两种版式。七页现在共用这一份结构与样式，新增认证页
 * 直接复用即可。
 *
 * 品牌数据走 /api/v1/site-profile（站型包默认 + 后台可覆盖），取不到回落
 * i18n 字典——认证页永不因后端故障而空版（同登录页原口径）。
 */

/** 品牌区（站标 / 站名 / 站点说明），失败回落 i18n 字典 */
function useSiteBrand() {
  const { dict } = useI18n();
  const [brand, setBrand] = useState<{
    name: string;
    desc: string;
    logo: string | null;
  }>({
    name: dict.common.brand,
    desc: dict.login.introDesc,
    logo: null,
  });
  useEffect(() => {
    let alive = true;
    api
      .get<{
        brand?: string;
        site_logo?: string | null;
        site_desc?: string | null;
      }>("/api/v1/site-profile")
      .then((p) => {
        if (!alive) return;
        setBrand({
          name: p.brand?.trim() || dict.common.brand,
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

export function AuthCanvas({
  title,
  desc,
  heroTag,
  heroDesc,
  badge,
  notice,
  foot,
  children,
}: {
  /** 卡片标题（= 页面标题，渲染为该页唯一 h1） */
  title: string;
  /** 卡片说明行（样图：标题下方的浅色副标题） */
  desc?: string;
  /** 左栏标语（本页定制，如「欢迎回来 · 种子等你很久啦」） */
  heroTag: string;
  /** 左栏说明（缺省用站点说明） */
  heroDesc?: string;
  /** 状态徽章（邀请制注册 / 待验证 / 一次性机会 / 人工审核） */
  badge?: React.ReactNode;
  /** 提示条（安全说明 / 时效告知） */
  notice?: React.ReactNode;
  /** 卡片页脚（同意条款等小字） */
  foot?: React.ReactNode;
  children: React.ReactNode;
}) {
  const { dict, locale } = useI18n();
  const b = useSiteBrand();
  const logoSrc = b.logo ?? "/brand/logo-mark.png";
  return (
    <div className="bz-login-page">
      <div className="bz-login-canvas">
        {/* 左：深色品牌 Hero */}
        <div className="bz-login-intro">
          <div className="bz-login-intro-inner">
            {/* eslint-disable-next-line @next/next/no-img-element */}
            <img
              src={logoSrc}
              alt=""
              aria-hidden
              className="bz-login-intro-mascot bz-login-intro-logo"
            />
            <p className="bz-login-intro-brand">{b.name}</p>
            <p className="bz-login-intro-tagline">{heroTag}</p>
            <p className="bz-login-intro-desc">{heroDesc ?? b.desc}</p>
          </div>
        </div>

        {/* 右上：主题 + 语言 */}
        <div className="bz-login-lang-row">
          <div className="flex items-center gap-2">
            <ThemeToggle className="min-h-0" label={dict.common.themeToggle} />
            {/* 语言切换器复用站内组件（地球图标 + 弹层），不再自写 select */}
            <LocaleSwitcher current={locale} />
          </div>
        </div>

        {/* 右：表单卡。main 锚点（W-04 SkipLink 目标）由 app/ 层的壳提供
            —— 本组件同时服务 (auth) 组与 /login，main 只能在一处，否则嵌套。 */}
        <div className="bz-login-card">
          <div className="auth-card__brand">
            {/* eslint-disable-next-line @next/next/no-img-element */}
            <img src={logoSrc} alt="" width={22} height={22} />
            <b>{b.name}</b>
          </div>
          <h1 className="auth-card__title">{title}</h1>
          {desc ? <p className="auth-card__desc">{desc}</p> : null}
          {badge ? (
            <div>
              <span className="auth-card__badge">{badge}</span>
            </div>
          ) : null}
          {notice ? <p className="auth-card__notice">{notice}</p> : null}
          {children}
          {foot ? <p className="auth-card__foot">{foot}</p> : null}
        </div>
      </div>
    </div>
  );
}
