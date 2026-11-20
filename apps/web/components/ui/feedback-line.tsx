"use client";

/** 成功/失败分色反馈行（第二轮评审 P1）：把散落的 setMsg(string)+
 *  text-sub 升级为带 ok 标识 + role=status/alert + mint/coral 的统一形态。 */
export function FeedbackLine({
  ok,
  children,
  className = "",
}: {
  ok: boolean;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <p
      role={ok ? "status" : "alert"}
      className={`text-xs ${
        ok
          ? "text-[var(--success)]"
          : "text-[var(--danger-text)]"
      } ${className}`}
    >
      {children}
    </p>
  );
}
