"use client";

import Link from "next/link";
import { useI18n } from "@/i18n/client";
import { Icon } from "@/components/icons";

/** 首页手机端快捷入口宫格（4×2）。
 *
 *  底栏只有 5 个 tab，其余入口全靠 59 项的抽屉——手机上要找「字幕」「邀请」
 *  得先开抽屉再翻。新版把 7 个高频入口平铺到首屏，聊天室占第一格（活体入口：
 *  红点 + 珊瑚色，区别于普通页面入口，点击开底部抽屉而不是跳页）。 */

interface QuickItem {
  key: string;
  label: string;
  icon: string;
  href: string;
  /** 配色档：sky / sun / mint */
  tone: "" | "is-alt" | "is-alt2";
}

export function MobileQuickGrid({ onChat }: { onChat: () => void }) {
  const { dict } = useI18n();
  const t = dict.mhome;
  const items: QuickItem[] = [
    { key: "chat", label: t.quickChat, icon: "messages",
      href: "", tone: "" },
    { key: "forums", label: t.quickForums, icon: "forums",
      href: "/forums", tone: "is-alt" },
    { key: "subs", label: t.quickSubs, icon: "subtitle",
      href: "/subtitles", tone: "is-alt2" },
    { key: "top", label: t.quickTop, icon: "ranking",
      href: "/top", tone: "" },
    { key: "bank", label: t.quickBank, icon: "bank",
      href: "/bank", tone: "is-alt" },
    { key: "req", label: t.quickReq, icon: "request",
      href: "/requests", tone: "is-alt2" },
    { key: "invite", label: t.quickInvite, icon: "invite",
      href: "/invites", tone: "" },
    { key: "games", label: t.quickGames, icon: "game",
      href: "/games", tone: "is-alt" },
  ];
  return (
    <nav className="mhome-quick md:hidden" aria-label={t.quickTitle}>
      {items.map((it) =>
        it.key === "chat" ? (
          <button
            type="button"
            key={it.key}
            className="mhome-quick__item is-live"
            onClick={onChat}
          >
            <span className="mhome-quick__icon">
              <Icon name={it.icon} size={19} />
              <i className="mhome-quick__dot" aria-hidden="true" />
            </span>
            <span>{it.label}</span>
          </button>
        ) : (
          <Link
            key={it.key}
            href={it.href}
            className={"mhome-quick__item " + it.tone}
          >
            <span className="mhome-quick__icon">
              <Icon name={it.icon} size={19} />
            </span>
            <span>{it.label}</span>
          </Link>
        ),
      )}
    </nav>
  );
}
