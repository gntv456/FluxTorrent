"use client";

import Link from "next/link";
import { useEffect } from "react";
import { ErrorPanel } from "@/components/ui/error-panel";
import { useI18n } from "@/i18n/client";
import { normalizeError } from "@/lib/render-error";

/** 路由级错误边界（2026-11-20 五问评审）：局段异常仍保留站点壳与导航，
 *  不会像根级 error.tsx 那样把 (main)/layout 一起替换掉。 */
export default function RouteError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  const { dict } = useI18n();
  useEffect(() => {
    console.error("[route-error]", error.digest ?? error.message);
  }, [error]);
  return (
    <div className="mx-auto max-w-3xl px-4 py-10">
      <h1 className="mb-4 font-display text-2xl">{dict.common.errorTitle}</h1>
      <ErrorPanel
        error={normalizeError(error)}
        onRetry={reset}
        desc={error.digest}
      />
      <Link href="/" className="mt-4 inline-block text-link underline">
        {dict.common.backHome}
      </Link>
    </div>
  );
}
