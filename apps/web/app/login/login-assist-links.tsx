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
 * 渲染（fail-closed，不承诺不存在的开放）。 */

import Link from "next/link";
import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

export function LoginAssistLinks() {
  const { dict } = useI18n();
  // null = 未知（探测中/失败），按邀请制渲染
  const [openReg, setOpenReg] = useState<boolean | null>(null);

  useEffect(() => {
    let alive = true;
    api
      .get<{ mode?: string }>("/api/v1/register-mode")
      .then((r) => {
        if (alive)
          setOpenReg(r.mode === "open" || r.mode === "email_verify");
      })
      .catch(() => setOpenReg(null));
    return () => {
      alive = false;
    };
  }, []);

  return (
    <div className="bz-account-links">
      {openReg ? (
        <Link href="/register" className="bz-open-reg-banner">
          <span className="bz-open-reg-badge">
            {dict.login.openRegBadge}
          </span>
          <span className="bz-open-reg-text">{dict.login.openRegNow}</span>
        </Link>
      ) : (
        <p>
          {dict.login.noAccount}
          <Link href="/register">{dict.login.signup}</Link>
        </p>
      )}
      <div className="bz-assistance">
        <p>
          {dict.login.forgotPassword}
          <Link href="/forgot">{dict.login.forgotLink}</Link>
        </p>
        {/* P-103：未激活用户的自救出口。后端拦截文案让用户「在登录页重发
            激活邮件」，而这张卡里此前没有该入口（resend 表单只在
            /email-verify 页，而那页过去只能由邮件抵达）。 */}
        <p>
          {dict.login.activateIntro}
          <Link href="/email-verify">{dict.login.activateLink}</Link>
        </p>
        <p>
          {dict.login.bannedIntro}
          <Link href="/ban-log">{dict.login.bannedLog}</Link>
        </p>
        <p>
          {dict.login.appealIntro}
          <Link href="/appeals">{dict.login.appealLink}</Link>
        </p>
        {/* 申请制入站出口（2026-10-18 对齐样图）：邀请制站点的无码用户
            在登录页就能找到申请入口，不必先摸到注册页。开放注册时
            这条降级隐藏（有码申请语境不存在，横幅已是主入口）。 */}
        {!openReg && (
          <p>
            {dict.login.applyIntro}
            <Link href="/apply">{dict.login.applyLink}</Link>
          </p>
        )}
      </div>
    </div>
  );
}
