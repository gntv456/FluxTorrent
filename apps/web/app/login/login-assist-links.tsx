"use client";

/** 登录页辅助链接卡（2026-10-18 从 login-form.tsx 按域拆出，300 行门禁）：
 * NexusPHP login.php 全口径的六条出路 —— 注册 / 找回密码 / 重发激活 /
 * 封禁记录 / 申诉 / 申请邀请码。文案走 dict.login，链接一律站内相对路径。
 *
 * P-682 注册状态感知：注册入口的文案与观感此前写死「邀请码注册」+
 * 「本站为邀请制」——站长切到开放注册后，访客在登录页完全看不出
 * 现在能直接注册（触达缺口与 P-680 的设置侧错位是一对）。现在登录页
 * 拉 register-mode：open / email_verify 时把注册行提为高亮横幅
 * （「开放注册中 · 立即注册」），邀请制保持原样。探测失败按邀请制
 * 渲染（fail-closed，不承诺不存在的开放）。
 *
 * 2026-11-01 手机版重设计：清单收成一份 items，渲染两处——桌面仍是那张
 * 多行卡（.bz-assist-desktop，样式一字未动），手机端只留「找回密码」
 * 常显，其余收进底部抽屉（复用共用件 BottomSheet，已在磨砂清单里）。
 * 抽屉每条 48px 热区 + 14px 正文，比原先 11.5px 的四行虚线列表好点。 */

import Link from "next/link";
import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { getRegisterMode, useRegisterMode } from "@/lib/use-register-mode";
import { BottomSheet } from "@/components/bottom-sheet";

/** register-mode 响应里辅助卡关心的字段（开放轮窗口） */
type RegModeResp = {
  mode?: string;
  reg_window?: { slots_left?: number; slots_total?: number | null };
};

/** 一条出路：左问句、右动作词（桌面卡与手机抽屉共用同一份清单） */
type HelpItem = { href: string; intro: string; action: string };

export function LoginAssistLinks() {
  const { dict } = useI18n();
  // null = 未知（探测中/失败），按邀请制渲染
  const openReg = useRegisterMode();
  // P-684 开放轮窗口（横幅里显示剩余名额）
  const [win, setWin] = useState<{
    slots_left?: number;
    slots_total?: number | null;
  } | null>(null);
  const [helpOpen, setHelpOpen] = useState(false);

  useEffect(() => {
    let alive = true;
    getRegisterMode<RegModeResp>()
      .then((r) => {
        if (!alive) return;
        setWin(
          r.mode === "open" || r.mode === "email_verify"
            ? (r.reg_window ?? null)
            : null,
        );
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  // P-103：「收不到激活邮件？」是后端拦截文案指的那条路——resend 表单
  // 只在 /email-verify 页，而那页过去只能由邮件抵达。
  // 申请邀请码：邀请制站点的无码用户在登录页就能申请，不必先摸到注册
  // 页；开放注册时降级隐藏（有码申请的语境不存在，横幅已是主入口）。
  const items: HelpItem[] = [
    {
      href: "/forgot",
      intro: dict.login.forgotPassword,
      action: dict.login.forgotLink,
    },
    {
      href: "/email-verify",
      intro: dict.login.activateIntro,
      action: dict.login.activateLink,
    },
    {
      href: "/ban-log",
      intro: dict.login.bannedIntro,
      action: dict.login.bannedLog,
    },
    {
      href: "/appeals",
      intro: dict.login.appealIntro,
      action: dict.login.appealLink,
    },
    ...(openReg
      ? []
      : [
          {
            href: "/apply",
            intro: dict.login.applyIntro,
            action: dict.login.applyLink,
          },
        ]),
  ];

  return (
    <div className="bz-account-links">
      {openReg ? (
        <Link href="/register" className="bz-open-reg-banner">
          <span className="bz-open-reg-badge">
            {dict.login.openRegBadge}
          </span>
          <span className="bz-open-reg-text">
            {dict.login.openRegNow}
            {win?.slots_left != null && (
              <strong className="bz-open-reg-slots">
                {" "}
                {fmt(dict.login.openRegSlots, { n: win.slots_left })}
              </strong>
            )}
          </span>
        </Link>
      ) : (
        <p className="bz-no-account">
          {dict.login.noAccount}
          <Link href="/register">{dict.login.signup}</Link>
        </p>
      )}
      <div className="bz-assistance bz-assist-desktop">
        {items.map((it) => (
          <p key={it.href}>
            {it.intro}
            <Link href={it.href}>{it.action}</Link>
          </p>
        ))}
      </div>
      {/* 手机端：找回密码是最高频出路，留在原位常显；其余进抽屉 */}
      <div className="bz-m-help">
        <Link href="/forgot" className="bz-m-help-link">
          {dict.login.forgotLink}
        </Link>
        <span className="bz-m-help-sep" aria-hidden />
        <button
          type="button"
          className="bz-m-help-btn"
          aria-expanded={helpOpen}
          aria-haspopup="dialog"
          onClick={() => setHelpOpen(true)}
        >
          {dict.login.moreHelp}
        </button>
      </div>
      <BottomSheet
        open={helpOpen}
        onClose={() => setHelpOpen(false)}
        title={dict.login.moreHelp}
      >
        <nav className="bz-help-list">
          {items.map((it) => (
            <Link key={it.href} href={it.href} className="bz-help-row">
              {it.intro.replace(/[？?]$/, "")}
              <em>{it.action}</em>
            </Link>
          ))}
        </nav>
      </BottomSheet>
    </div>
  );
}
