"use client";

/** 错误面板（DS-PLAN-01 W-03 / §3.2：现代性判据 5「失败可诊断」）。
 *
 *  与后端错误信封 { code, message, request_id } 对齐：
 *  - request_id 一键复制——用户报障时附上，站长可直接 grep 日志；
 *  - 错误码 chip（1002/2001/5003…）便于对照 docs/api/error-codes.md；
 *  - 重试按钮（调用方给 onRetry 才出现）；
 *  - dev 模式折叠堆栈（生产不渲染，避免泄露内部路径）；
 *  - role="alert" + aria-live="assertive"：读屏即时播报（语义双保险——
 *    role=alert 本身隐含 assertive，显式写出便于 lint 与审计识别）。
 *
 *  ⚠️ RSC 边界：Error 实例不可跨 server→client 传 props。服务端页面
 *  catch 后先过 normalizeError() 转普通对象再传给本件（或让页面自身
 *  为 client 组件自行 fetch）。
 */

import { useState } from "react";

import { Icon } from "@/components/icons";
import { ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

export interface PanelError {
  code?: number | string;
  message: string;
  requestId?: string;
}

/** 任意 unknown → 可渲染的普通错误对象（ApiError 保 request_id） */
export function normalizeError(err: unknown): PanelError {
  if (err instanceof ApiError) {
    return { code: err.code, message: err.message, requestId: err.requestId };
  }
  if (err instanceof Error) return { message: err.message };
  if (typeof err === "string") return { message: err };
  return { message: String(err) };
}

export function ErrorPanel({
  error,
  onRetry,
  retryHref,
  title,
  desc,
}: {
  /** 原始错误或 normalizeError() 后的普通对象均可 */
  error: unknown;
  onRetry?: () => void;
  /** 服务端页（RSC）用链接重试（onRetry 回调不可跨 RSC 边界序列化） */
  retryHref?: string;
  title?: string;
  desc?: string;
}) {
  const { dict } = useI18n();
  const e: PanelError =
    error && typeof error === "object" && "message" in error
      ? (error as PanelError)
      : normalizeError(error);
  const [copied, setCopied] = useState(false);
  const stack = error instanceof Error ? error.stack : undefined;

  return (
    <div className="ui-err" role="alert" aria-live="assertive">
      <span className="ui-err__icon" aria-hidden="true">
        <Icon name="warning" size={22} />
      </span>
      <div className="ui-err__body">
        <p className="ui-err__title">{title ?? dict.ui.errorTitle}</p>
        <p className="ui-err__msg">{e.message}</p>
        <p className="ui-err__desc">{desc ?? dict.ui.errorDesc}</p>
        <div className="ui-err__meta">
          {e.code !== undefined ? (
            <span className="ui-err__code">
              {dict.ui.errorCode} {e.code}
            </span>
          ) : null}
          {e.requestId ? (
            <button
              type="button"
              className="ui-err__rid"
              onClick={() => {
                void navigator.clipboard
                  ?.writeText(e.requestId ?? "")
                  .then(() => {
                    setCopied(true);
                    window.setTimeout(() => setCopied(false), 1500);
                  })
                  .catch(() => {
                    /* 非安全上下文等：静默，值仍在屏幕上可手抄 */
                  });
              }}
            >
              <Icon name="copy" size={11} />
              <span>{dict.ui.requestIdLabel}</span>
              <code>{copied ? dict.ui.copied : e.requestId}</code>
            </button>
          ) : null}
          {onRetry ? (
            <button type="button" className="ui-err__retry" onClick={onRetry}>
              <Icon name="refresh" size={12} />
              {dict.common.retry}
            </button>
          ) : retryHref ? (
            <a href={retryHref} className="ui-err__retry">
              <Icon name="refresh" size={12} />
              {dict.common.retry}
            </a>
          ) : null}
        </div>
        {stack && process.env.NODE_ENV !== "production" ? (
          <details className="ui-err__stack">
            <summary>{dict.ui.showDetails}</summary>
            <pre>{stack}</pre>
          </details>
        ) : null}
      </div>
    </div>
  );
}
