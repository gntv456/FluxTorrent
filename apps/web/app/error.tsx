"use client";

import { useEffect } from "react";
import Link from "next/link";

import { EmptyState } from "@/components/ui/empty-state";

/** 运行时错误页（2026-10-03 新建，client 组件——Next 要求）。
 *
 * 为什么需要：此前全站没有 error.tsx，任何未捕获的渲染异常都落在
 * Next.js 内置错误页（同 404 页：硬编码白底、无站点壳、无重试按钮）。
 * 夜间用户看到的是一整页白屏 + 一行英文。
 *
 * 注意与 not-found.tsx 的分工：
 *   - not-found  = 404（路由不存在，RSC，可静态）
 *   - error      = 渲染/数据异常（必须 client，须提供 reset 重试）
 * 两者此前都缺，夜间都是刺眼白页。
 */
export default function GlobalRouteError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  useEffect(() => {
    // digest 是 Next 服务端日志关联 ID，前端只上报不上报堆栈（不泄内部路径）
    console.error("[route-error]", error.digest ?? error.message);
  }, [error]);

  return (
    <main className="grid min-h-[60vh] place-items-center px-4">
      <div className="w-full max-w-md">
        <EmptyState
          icon="warning"
          title="页面出错了"
          desc="数据加载或渲染时发生异常，通常是暂时性的。可重试一次；若反复出现请到「联系管理组」反馈。"
          action={
            <div className="flex justify-center gap-2">
              <button
                type="button"
                onClick={reset}
                className="ui-btn ui-btn--primary"
              >
                重试
              </button>
              {/* 站内导航必须用 Link（<a href="/…"> 会挂
                  no-html-link-for-pages 致命错误，记忆里的老坑） */}
              <Link href="/" className="ui-btn ui-btn--secondary">
                回首页
              </Link>
            </div>
          }
        />
      </div>
    </main>
  );
}
