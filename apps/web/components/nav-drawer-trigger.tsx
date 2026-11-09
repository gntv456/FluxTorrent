"use client";

/** 汉堡触发按钮（受控）：44px 触控目标。从 nav-drawer 拆出（300 门禁）。 */
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
