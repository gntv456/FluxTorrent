"use client";

import Link from "next/link";
import { useEffect, useRef } from "react";
import { usePathname } from "next/navigation";
import { useI18n } from "@/i18n/client";
import { ThemeToggle } from "@/components/theme-toggle";
import type { NavItem, NavGroup } from "@/lib/nav-menu";

/** 移动导航抽屉（M2）：与桌面 MainMenu 同源（lib/nav-menu.ts 构建的
 *  primary + groups 原样传入），左侧滑入 280ms var(--ease-out)。
 *  用户摘要行由调用方传入（RSC 侧取 /me/overview 摘要或回落游客文案）。
 *  关闭：遮罩点击 / ESC / 右缘 48px 内 pointerdown（手势关闭）。
 *  打开期间锁 body 滚动；路由切换自动关闭。 */

export function NavDrawer({
  open,
  onClose,
  primary,
  groups,
  summary,
}: {
  open: boolean;
  onClose: () => void;
  primary: NavItem[];
  groups: NavGroup[];
  /** 用户摘要（用户名/等级/上传/魔力）；null = 未登录 → 游客文案 */
  summary: {
    username: string;
    className: string;
    uploaded: string;
    spark: string;
  } | null;
}) {
  const pathname = usePathname();
  const { dict } = useI18n();
  const drawerRef = useRef<HTMLDivElement>(null);
  const t = dict.navDrawer;

  // 路由变化自动收起
  useEffect(() => {
    onClose();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pathname]);

  // ESC / body 滚动锁
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = prevOverflow;
    };
  }, [open, onClose]);

  // 右缘手势关闭（pointerdown 起手即收，无需完整 swipe 实现）
  useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      if (window.innerWidth - e.clientX <= 48) onClose();
    };
    document.addEventListener("pointerdown", onDown);
    return () => document.removeEventListener("pointerdown", onDown);
  }, [open, onClose]);

  if (!open) return null;

  const Item = ({ href, label }: NavItem) => (
    <Link
      href={href}
      className="navdrawer__link"
      onClick={() => onClose()}
    >
      {label}
    </Link>
  );

  return (
    <div
      className="navdrawer-root"
      role="dialog"
      aria-modal="true"
      aria-label={t.open}
    >
      <div className="navdrawer-mask" onClick={() => onClose()} />
      <div className="navdrawer" ref={drawerRef}>
        <div className="navdrawer__head">
          <div className="navdrawer__user">
            <span className="navdrawer__avatar" aria-hidden>
              {summary ? summary.username.slice(0, 1).toUpperCase() : "·"}
            </span>
            <div>
              <p className="navdrawer__name">
                {summary ? summary.username : t.guest}
              </p>
              {summary && (
                <p className="navdrawer__stat">
                  {summary.className} · ↑ {summary.uploaded} · ★{" "}
                  {summary.spark}
                </p>
              )}
            </div>
          </div>
          <button
            type="button"
            className="navdrawer__close"
            aria-label={t.close}
            onClick={() => onClose()}
          >
            ✕
          </button>
        </div>
        <nav className="navdrawer__body">
          <div className="navdrawer__group">
            <p className="navdrawer__gtitle">{dict.nav.ariaPrimary}</p>
            <div className="navdrawer__grid">
              {primary.map((n) => (
                <Item key={n.href} {...n} />
              ))}
            </div>
          </div>
          {groups.map((g) => (
            <div key={g.group} className="navdrawer__group">
              <p className="navdrawer__gtitle">{g.group}</p>
              <div className="navdrawer__grid">
                {g.items.map((n) => (
                  <Item key={n.href} {...n} />
                ))}
              </div>
            </div>
          ))}
        </nav>
        {/* 主题切换 <md 从顶栏收进来（280px 外屏态右工具区挤不下） */}
        <div className="navdrawer__foot">
          <ThemeToggle label={dict.common.themeToggle} />
        </div>
      </div>
    </div>
  );
}

/** 汉堡触发按钮（受控）：44px 触控目标 */
export function NavDrawerTrigger({
  onClick,
  label,
}: {
  onClick: () => void;
  label: string;
}) {
  return (
    <button
      type="button"
      className="navdrawer-trigger"
      aria-label={label}
      aria-haspopup="dialog"
      onClick={onClick}
    >
      <svg
        width="20"
        height="20"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
      >
        <path d="M4 7h16M4 12h16M4 17h10" />
      </svg>
    </button>
  );
}
