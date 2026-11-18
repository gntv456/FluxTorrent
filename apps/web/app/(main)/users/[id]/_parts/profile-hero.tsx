/**
 * 用户公开主页 Hero 与磁贴（从 app/(main)/users/[id]/page.tsx 按域拆出）：
 * ProfileHero 头像 + 用户名/勋章 + 等级头衔 + 在线 + 注册/最近活动（所有 tab 共用）；
 * StatTiles 首屏健康度四卡（分享率/H&R/做种/魔力）+ 上传总量次级信息。无 hooks：server component。
 */

import { avatarFrameStyle, FrameImageOverlay } from "@/lib/format";
import { FollowButton } from "@/components/forum-follow";
import { MedalIcon } from "@/components/medal-icon";
import { Icon } from "@/components/icons";
import { Badge } from "@/components/ui/badge";
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
          /* B3-3：无真实头像时保留人形占位，但不叠头像框（去掉空挂件观感） */
          <span
            aria-hidden
            className="ux-hero__avatar--empty relative flex h-full w-full
              items-center justify-center text-sub"
          >
            <Icon name="user" size={34} />
          </span>
        )}
      </div>
      <div className="up-hero__main">
        <div className="up-hero__name">
          <h1 className="font-display text-2xl text-[var(--text-strong)]">
            {p.username}
          </h1>
          {data.worn_medals.slice(0, 5).map((m) => (
            <span key={m.id} className="medal-chip align-middle" title={m.name}>
              <MedalIcon src={m.asset_ref} size={14} title={m.name} />
            </span>
          ))}
          <span
            className={`rounded-full px-2 py-0.5 text-xs font-bold ${
              data.online
                ? "bg-[var(--mint-soft)] text-mint"
                : "bg-[var(--surface-sunken)] text-sub"
            }`}
            title={data.online ? t.online : t.offline}
          >
            {/* 单表达式拼好整段文本：SSR 与水合的文本节点划分一致，避免 418 */}
            {`● ${data.online ? t.online : t.offline}`}
          </span>
          {p.donor && <Badge tone="warn">{`♥ ${t.donor}`}</Badge>}
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
            <i aria-hidden><Icon name="clock" size={14} /></i>
            {`${t.joined}：${new Date(p.created_at).toLocaleDateString(dateLocale(locale))}`}
          </span>
          {p.last_seen_at && (
            <span>
              <i aria-hidden><Icon name="clock" size={14} /></i>
              {`${t.lastSeen}：${new Date(p.last_seen_at).toLocaleString(dateLocale(locale))}`}
            </span>
          )}
        </div>
      </div>
    </section>
  );
}

/** B3-2：健康度优先磁贴——四张带语义状态 chip 的卡 + 上传总量次级信息。 */
export function StatTiles({
  tiles,
  heading,
  secondary,
}: {
  tiles: {
    key: string;
    icon: string;
    label: string;
    value: string;
    status: "safe" | "watch";
    statusLabel: string;
  }[];
  heading: string;
  secondary: string;
}) {
  return (
    <section className="ux-health" aria-label={heading}>
      <div className="ux-health__head">
        <Icon name="gauge" size={16} />
        <h2>{heading}</h2>
      </div>
      <div className="ux-health__grid">
        {tiles.map((tile) => (
          <article key={tile.key} className="ux-health-card">
            <div className="ux-health-card__top">
              <span className="ux-health-card__icon">
                <Icon name={tile.icon} size={18} />
              </span>
              <span className={`ux-chip ux-chip--${tile.status}`}>
                {tile.statusLabel}
              </span>
            </div>
            <div className="ux-health-card__value">{tile.value}</div>
            <div className="ux-health-card__label">{tile.label}</div>
          </article>
        ))}
      </div>
      <p className="ux-health__secondary">
        <Icon name="upload" size={14} />
        {secondary}
      </p>
    </section>
  );
}
