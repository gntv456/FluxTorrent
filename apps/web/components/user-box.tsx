"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { setSessionCookie } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/**
 * 右上角用户位：登录后显示用户名 + 退出，未登录显示登录链接。
 * 服务端 Header 无法读 localStorage 的 JWT，状态由客户端补齐。
 */
export function UserBox({ loginLabel }: { loginLabel: string }) {
  const { dict } = useI18n();
  const [username, setUsername] = useState<string | null>(null);

  useEffect(() => {
    if (!localStorage.getItem("flux.token")) return;
    api
      .get<{ username: string }>("/api/v1/me")
      .then((me) => setUsername(me.username))
      .catch(() => {
        // token 失效：清掉本地凭证，回落到登录链接
        localStorage.removeItem("flux.token");
        localStorage.removeItem("flux.user");
        setSessionCookie(null);
      });
  }, []);

  async function logout() {
    try {
      await api.post("/api/v1/auth/logout", {});
    } catch {
      // 后端撤销失败也照常清理本地凭证
    }
    localStorage.removeItem("flux.token");
    localStorage.removeItem("flux.user");
    setSessionCookie(null);
    location.href = "/login";
  }

  if (!username) {
    return (
      <a
        href="/login"
        className="min-h-[44px] flex items-center text-sm text-white/80 hover:text-sky"
      >
        {loginLabel}
      </a>
    );
  }
  return (
    <div className="flex items-center gap-2">
      <a
        href="/my"
        className="min-h-[44px] flex items-center gap-1.5 text-sm text-white/90 hover:text-sky"
        title={dict.my.center}
      >
        <span
          aria-hidden
          className="flex h-8 w-8 items-center justify-center rounded-full bg-sky-deep text-sm font-bold text-white"
        >
          {username.slice(0, 1).toUpperCase()}
        </span>
        <span className="hidden sm:inline">{username}</span>
      </a>
      <button
        type="button"
        onClick={logout}
        className="min-h-[44px] flex items-center text-sm text-white/60 hover:text-coral"
      >
        {dict.my.logout}
      </button>
    </div>
  );
}
