"use client";

import Link from "next/link";
import { ThemeToggle } from "@/components/theme-toggle";

/** 山海图鉴侧栏页脚：用户境界签（真头像/首字母）+ 主题切换 + 整栏折叠钮。
 *  从 shj-atlas.tsx 按 300 行门禁拆出。 */
export function ShjAtlasFoot({
  summary,
  avaFailed,
  setAvaFailed,
  rail,
  toggleRail,
  themeToggleLabel,
  expandLabel,
  collapseLabel,
}: {
  summary: {
    username: string;
    className: string;
    uploaded: string;
    avatarUrl?: string | null;
  } | null;
  avaFailed: boolean;
  setAvaFailed: (v: boolean) => void;
  rail: boolean;
  toggleRail: () => void;
  themeToggleLabel: string;
  expandLabel: string;
  collapseLabel: string;
}) {
  return (
    <div className="shj-atlas__foot">
      {summary && (
        <Link
          href="/my"
          className="shj-atlas__realm"
          title={summary.username}
        >
          <span className="shj-atlas__rseal" aria-hidden>
            {summary.avatarUrl && !avaFailed ? (
              <img
                src={summary.avatarUrl}
                alt=""
                loading="lazy"
                decoding="async"
                onError={() => setAvaFailed(true)}
              />
            ) : (
              summary.username.slice(0, 1).toUpperCase()
            )}
          </span>
          <span className="shj-atlas__rtext">
            <b>{summary.username}</b>
            <i>
              {summary.className} · ↑ {summary.uploaded}
            </i>
          </span>
        </Link>
      )}
      <ThemeToggle label={themeToggleLabel} />
      {/* 整栏折叠钮（宽屏专属；窄断点由 CSS 隐藏） */}
      <button
        type="button"
        className="shj-atlas__fold"
        onClick={toggleRail}
        aria-expanded={!rail}
        title={rail ? expandLabel : collapseLabel}
      >
        <span aria-hidden>{rail ? "»" : "«"}</span>
        <span className="shj-atlas__foldtext">
          {rail ? expandLabel : collapseLabel}
        </span>
      </button>
    </div>
  );
}
