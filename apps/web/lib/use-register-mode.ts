"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";

/** 注册模式感知 hook（P-682/P-683 抽出）：
 *  拉 /api/v1/register-mode，open / email_verify 视为「开放注册」。
 *  null=未知：调用方按邀请制弱化渲染，不虚报开放。
 *  此前 login-view / login-form / login-assist-links 三处各自 fetch
 *  同一口——同一个登录页对同一端点打三次。
 *
 *  2026-11-11 提速批：响应进**模块级缓存**（含 in-flight promise 去重）。
 *  登录页三消费点 + Next 对 /register /apply 等 auth 页的视口内 Link
 *  预取（各页挂载时同样要 register-mode）曾让同一会话打 4+ 次这一口。
 *  register-mode 是准静态配置（站长改注册模式才变），会话级缓存零
 *  副作用；硬刷新仍是一次真实请求。 */

interface RegModeResp {
  mode?: string;
}

let cached: Promise<RegModeResp> | null = null;

function fetchOnce(): Promise<RegModeResp> {
  if (!cached) {
    cached = api.get<RegModeResp>("/api/v1/register-mode");
    // 失败不缓存（下一个消费点可重试），reject 原样透传给调用方 catch
    cached.catch(() => {
      cached = null;
    });
  }
  return cached;
}

export function useRegisterMode(): boolean | null {
  const [open, setOpen] = useState<boolean | null>(null);
  useEffect(() => {
    let alive = true;
    fetchOnce().then((r) => {
      if (alive) setOpen(r.mode === "open" || r.mode === "email_verify");
    });
    return () => {
      alive = false;
    };
  }, []);
  return open;
}

/** 需要完整响应的调用方（register/apply 页的 ModeInfo 拉取）共用同一份
 *  缓存请求——返回类型收窄由调用方自行断言（字段集比本文件的宽定义多）。 */
export function getRegisterMode<T extends object>(): Promise<T> {
  return fetchOnce() as Promise<T>;
}

/** 测试专用：清掉模块级缓存（vitest 用例间 MODE 变化须重新探测）。 */
export function __resetRegisterModeCache(): void {
  cached = null;
}
