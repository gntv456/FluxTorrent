"use client";

/** 登录页辅助链接卡（2026-10-18 从 login-form.tsx 按域拆出，300 行门禁）：
 *  NexusPHP login.php 全口径的六条出路 —— 注册 / 找回密码 / 重发激活 /
 *  封禁记录 / 申诉 / 申请邀请码。文案走 dict.login，链接一律站内相对路径。 */

import Link from "next/link";
import { useI18n } from "@/i18n/client";

export function LoginAssistLinks() {
  const { dict } = useI18n();
  return (
    <div className="bz-account-links">
      <div className="bz-assistance">
        <p>
          {dict.login.noAccount}
          <Link href="/register">{dict.login.signup}</Link>
        </p>
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
            在登录页就能找到申请入口，不必先摸到注册页。 */}
        <p>
          {dict.login.applyIntro}
          <Link href="/apply">{dict.login.applyLink}</Link>
        </p>
      </div>
    </div>
  );
}
