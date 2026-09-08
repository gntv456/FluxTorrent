"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";

interface LoginResp {
  token: string;
  must_reset_password: boolean;
  user: { id: number; username: string; class_id: number };
}

/** 登录页（设计稿：吉祥物 + 蜡笔字标题；错误按 code 映射文案 §8.2） */
export default function LoginPage() {
  const router = useRouter();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const messages: Record<number, string> = {
    2004: "用户名或密码不对哦，再想想～",
    2001: "试得太频繁啦，休息一分钟再来",
  };

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const resp = await api.post<LoginResp>("/api/v1/auth/login", {
        username,
        password,
      });
      localStorage.setItem("flux.token", resp.token);
      router.push("/torrents");
    } catch (err) {
      if (err instanceof ApiError) {
        setError(messages[err.code] ?? `登录失败（${err.code}）`);
      } else {
        setError("网络异常，请稍后再试");
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="mx-auto flex max-w-sm flex-col items-center gap-6 py-12">
      <span aria-hidden className="text-[80px] leading-none">
        🦉
      </span>
      <h1 className="font-display text-3xl">欢迎回来</h1>
      <p className="-mt-4 text-sm text-sub">种子等你很久啦</p>
      <form onSubmit={submit} className="flex w-full flex-col gap-3">
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">用户名</span>
          <input
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            autoComplete="username"
            required
            className="min-h-[44px] rounded-[var(--r-sm)] border border-line bg-white px-3 outline-none focus:ring-2 focus:ring-sky/40"
          />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">密码</span>
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="current-password"
            required
            className="min-h-[44px] rounded-[var(--r-sm)] border border-line bg-white px-3 outline-none focus:ring-2 focus:ring-sky/40"
          />
        </label>
        {error && (
          <p role="alert" className="text-sm text-danger">
            {error}
          </p>
        )}
        <button
          type="submit"
          disabled={busy}
          className="min-h-[44px] rounded-full bg-sky font-bold text-white active:scale-[0.97] disabled:opacity-50"
        >
          {busy ? "登录中…" : "登录"}
        </button>
      </form>
    </div>
  );
}
