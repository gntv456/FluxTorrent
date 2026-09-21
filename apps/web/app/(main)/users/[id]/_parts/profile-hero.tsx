/**
 * 用户公开主页 Hero 与磁贴（从 app/(main)/users/[id]/page.tsx 按域拆出）：
 * ProfileHero 头像 + 用户名/勋章 + 等级头衔 + 在线 + 注册/最近活动（所有 tab 共用）；
 * StatTiles 首屏核心指标磁贴（5 个关键数）。无 hooks：server component。
 */

import { avatarFrameStyle, FrameImageOverlay } from "@/lib/format";
import { FollowButton } from "@/components/forum-follow";
import { MedalIcon } from "@/components/medal-icon";
import type { Locale } from "@/i18n/config";
import { dateLocale } from "@/i18n/config";
import type { ProfileData } from "./profile-types";

export function ProfileHero({
  data,
  locale,
  t,
}: {
  data: ProfileData;
  locale: Locale;
  t: Record<string, string>;
}) {
  const p = data.profile;
  return (
    <section className="up-hero">
      <div className="up-hero__avatar">
        {/* 头像 + 佩戴的头像框：image_url 立绘框图叠层优先，否则 CSS 描边 */}
        {p.avatar_url ? (
          // eslint-disable-next-line @next/next/no-img-element
          <span
            className="relative inline-flex h-full w-full"
            style={
              data.avatar_frame_image
                ? undefined
                : avatarFrameStyle(data.avatar_frame_css)
            }
          >
            <img src={p.avatar_url} alt={p.username} />
            <FrameImageOverlay url={data.avatar_frame_image} />
          </span>
        ) : (
          <span
            aria-hidden
            className="relative flex h-full w-full items-center justify-center text-4xl text-sub"
            style={
              data.avatar_frame_image
                ? undefined
                : avatarFrameStyle(data.avatar_frame_css)
            }
          >
            👤
            <FrameImageOverlay url={data.avatar_frame_image} />
          </span>
        )}
      </div>
      <div className="up-hero__main">
        <div className="up-hero__name">
          <h1 className="font-display text-2xl text-[var(--text-strong)]">
            {p.username}
          </h1>
          {data.worn_medals.slice(0, 3).map((m) => (
            <span key={m.id} className="medal-chip align-middle" title={m.name}>
              <MedalIcon src={m.asset_ref} size={14} title={m.name} />
            </span>
          ))}
          <span
            className={`rounded-full px-2 py-0.5 text-[11px] font-bold ${
              data.online
                ? "bg-[var(--mint-soft)] text-mint"
                : "bg-[var(--surface-sunken)] text-sub"
            }`}
            title={data.online ? t.online : t.offline}
          >
            {/* 单表达式拼好整段文本：SSR 与水合的文本节点划分一致，避免 418 */}
            {`● ${data.online ? t.online : t.offline}`}
          </span>
          {p.donor && (
            <span className="sticker bg-sun text-ink">{`♥ ${t.donor}`}</span>
          )}
        </div>
        {/* 等级/头衔 + 关注（0121）：FollowButton 自拉状态，is_self 时自行隐藏 */}
        <div className="mt-2 flex flex-wrap items-center gap-2 text-sm text-sub">
          <span className="up-hero__class">
            {`${p.class_name ?? `LV${p.class_id}`}${p.title ? ` · ${p.title}` : ""}`}
          </span>
          <FollowButton targetType="user" targetId={p.id} showCount />
        </div>
        <div className="up-hero__meta">
          <span>
            <i aria-hidden>◷</i>
            {`${t.joined}：${new Date(p.created_at).toLocaleDateString(dateLocale(locale))}`}
          </span>
          {p.last_seen_at && (
            <span>
              <i aria-hidden>◷</i>
              {`${t.lastSeen}：${new Date(p.last_seen_at).toLocaleString(dateLocale(locale))}`}
            </span>
          )}
        </div>
      </div>
    </section>
  );
}

export function StatTiles({
  statTiles,
}: {
  statTiles: {
    key: string;
    icon: string;
    mod?: string;
    label: string;
    value: string;
  }[];
}) {
  return (
    <section className="up-stats">
      {statTiles.map((s) => (
        <div key={s.key} className="up-stat">
          <span
            className={`up-stat__icon${s.mod ? ` up-stat__icon--${s.mod}` : ""}`}
            aria-hidden
          >
            {s.icon}
          </span>
          <div className="min-w-0">
            <div className="up-stat__value">{s.value}</div>
            <div className="up-stat__label">{s.label}</div>
          </div>
        </div>
      ))}
    </section>
  );
}
