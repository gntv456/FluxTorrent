"use client";

/** 四态路由器（DS-PLAN-01 W-03 / §3.2：消灭「三态各自手写」的关键件）。
 *
 *  页面只声明四态，内部调度 Skeleton / ErrorPanel / EmptyState：
 *
 *    <StatePanel
 *      loading={rows === null}
 *      error={err}
 *      empty={rows?.length === 0}
 *      onRetry={() => reload()}
 *    >
 *      <Rows rows={rows} />
 *    </StatePanel>
 *
 *  判定顺序：loading → error → empty → children（错误优先于空——
 *  拉取失败时 rows 往往是空数组，先报错才不会误显「暂无内容」）。
 *  loading 形态默认 SkeletonBlock（媒体占位），表格/表单场景传
 *  loadingFallback 覆盖（如 <SkeletonRows />）。
 */

import type { ReactNode } from "react";

import { EmptyState } from "./empty-state";
import type { EmptyTone } from "./empty-state";
import { SkeletonBlock } from "./skeleton";
import { ErrorPanel } from "./error-panel";
import { useI18n } from "@/i18n/client";

export function StatePanel({
  loading,
  error,
  empty,
  children,
  loadingFallback,
  onRetry,
  emptyTitle,
  emptyDesc,
  emptyAction,
  emptyIcon,
  emptyTone,
}: {
  loading?: boolean;
  /** 非 null/undefined 即进入错误态（原始错误或普通对象均可） */
  error?: unknown;
  empty?: boolean;
  children: ReactNode;
  /** 自定义加载形态（表格用 SkeletonRows、详情用组合骨架） */
  loadingFallback?: ReactNode;
  onRetry?: () => void;
  emptyTitle?: string;
  emptyDesc?: string;
  emptyAction?: ReactNode;
  emptyIcon?: string;
  /** 空态语气色（EmptyState tone 协议透传，缺省 neutral） */
  emptyTone?: EmptyTone;
}) {
  const { dict } = useI18n();
  if (loading) {
    return <>{loadingFallback ?? <SkeletonBlock height={180} />}</>;
  }
  if (error !== undefined && error !== null) {
    return <ErrorPanel error={error} onRetry={onRetry} />;
  }
  if (empty) {
    return (
      <EmptyState
        icon={emptyIcon ?? "info"}
        tone={emptyTone}
        title={emptyTitle ?? dict.ui.emptyTitle}
        desc={emptyDesc ?? dict.ui.emptyDesc}
        action={emptyAction}
      />
    );
  }
  return <>{children}</>;
}
