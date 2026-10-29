"use client";

import { Suspense } from "react";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { AuthCanvas } from "@/components/auth-canvas";
import { LoginForm } from "./login-form";

/** 登录页外壳（从 app/login/page.tsx 按域拆出，来源文件是 server
 *  component 页面；本文件含 hooks 故为 client）。
 *
 *  2026-10-18 对齐样图 § 账户与认证：七页统一走 AuthCanvas（左深色 Hero +
 *  右上语言/主题 + 右白卡）。原来这里的语言下拉 / 品牌数据 / 画布结构全部
 *  收进共用画布，本文件只负责把登录表单与安全提示放进卡里。
 *
 *  P-682：底部「本站为邀请制」提示行与真实注册模式同步——开放注册
 *  站点挂着邀请制告示是直接矛盾的信息。mode 未知（探测失败）保持
 *  邀请制文案（fail-closed，不虚报开放）。 */

import { useRegisterMode } from "@/lib/use-register-mode";

export function LoginShell() {
  const { dict } = useI18n();
  // P-682：mode 未知（探测失败）按邀请制渲染（fail-closed，不虚报开放）
  const openReg = useRegisterMode() ?? false;
  return (
    <main id="main-content">
      <AuthCanvas
        title={dict.login.loginTitle}
        desc={dict.login.subtitle}
        heroTag={dict.login.welcome}
        heroDesc={openReg ? dict.login.openRegHero : undefined}
      >
        <Suspense fallback={<div className="bz-form-skeleton" aria-hidden />}>
          <LoginForm />
        </Suspense>
        <p className="bz-login-notice">
          {dict.login.cookieNote}
          <br />
          {fmt(dict.login.failBanNote, { n: 5 })}{" "}
          {openReg ? dict.login.openRegNote : dict.login.inviteOnly}
        </p>
      </AuthCanvas>
    </main>
  );
}
