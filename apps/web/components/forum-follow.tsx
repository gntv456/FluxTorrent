"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

type TargetType = "user" | "forum" | "topic";

interface MutResp {
  following: boolean;
  followers: number;
}

interface StatusResp {
  following: boolean;
  followers: number;
  is_self: boolean;
}

/**
 * 关注按钮（0121），三处复用：用户页 / 版块页 / 主题页。
 *
 * 设计：状态由组件自己拉（`GET /follows/status`），所以**用户页不需要额外传当前登录者**，
 * 后端把 `is_self` 一并回给前端由组件隐藏按钮（关注自己无意义）。
 * 权限：接口需登录，匿名访问 401 → 组件直接不渲染（与点赞/收藏同口径）。
 */
export function FollowButton({
  targetType,
  targetId,
  showCount = false,
  className = "",
}: {
  targetType: TargetType;
  targetId: number;
  showCount?: boolean;
  className?: string;
}) {
  const { dict } = useI18n();
  const t = dict.forums;
  // null = 未知（未登录 / 请求中），此时不渲染，避免闪一下再消失
  const [st, setSt] = useState<StatusResp | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let alive = true;
    api
      .get<StatusResp>(`/api/v1/follows/status?target_type=${targetType}&target_id=${targetId}`)
      .then((r) => {
        if (alive && r) setSt(r);
      })
      .catch(() => {
        /* 未登录/失败：保持 null，不渲染按钮 */
      });
    return () => {
      alive = false;
    };
  }, [targetType, targetId]);

  if (!st || st.is_self) return null;
  const on = st.following;

  async function toggle() {
    if (busy || !st) return;
    setBusy(true);
    const next = !st.following;
    // 乐观更新
    setSt({ ...st, following: next, followers: st.followers + (next ? 1 : -1) });
    try {
      const r = next
        ? await api.post<MutResp>("/api/v1/follows", {
            target_type: targetType,
            target_id: targetId,
          })
        : await api.del<MutResp>(`/api/v1/follows/${targetType}/${targetId}`);
      if (r) setSt({ ...st, following: r.following ?? next, followers: r.followers ?? st.followers });
    } catch {
      setSt(st); // 回滚
    } finally {
      setBusy(false);
    }
  }

  return (
    <button
      type="button"
      onClick={() => void toggle()}
      disabled={busy}
      aria-pressed={on}
      title={on ? t.following : t.follow}
      className={`inline-flex min-h-[32px] items-center gap-1 rounded-full border px-3 text-xs font-bold transition disabled:opacity-50 ${
        on ? "border-transparent bg-[var(--sky-soft)] text-sky" : "border-line text-sub hover:border-sky hover:text-sky"
      } ${className}`}
    >
      <span aria-hidden="true">{on ? "✓" : "+"}</span>
      {on ? t.following : t.follow}
      {showCount && st.followers > 0 && <span className="num opacity-80">· {st.followers}</span>}
    </button>
  );
}
