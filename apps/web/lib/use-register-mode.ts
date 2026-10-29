"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";

/** 注册模式感知 hook（P-682/P-683 抽出）：
 *  拉 /api/v1/register-mode，open / email_verify 视为「开放注册」。
 *  null=未知：调用方按邀请制弱化渲染，不虚报开放。
 *  此前 login-view / login-form / login-assist-links 三处各自 fetch
 *  同一口——同一个登录页对同一端点打三次。 */
export function useRegisterMode(): boolean | null {
  const [open, setOpen] = useState<boolean | null>(null);
  useEffect(() => {
    let alive = true;
    api
      .get<{ mode?: string }>("/api/v1/register-mode")
      .then((r) => {
        if (alive)
          setOpen(r.mode === "open" || r.mode === "email_verify");
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);
  return open;
}
