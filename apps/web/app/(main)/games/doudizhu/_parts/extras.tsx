"use client";
import Link from "next/link";
import { useI18n } from "@/i18n/client";

type DzDict = ReturnType<typeof useI18n>["dict"]["games"]["doudizhu"];

interface Entry {
  icon: string;
  label: string;
  href: string;
}

/**
 * P2-2~6 最小骨架：装扮 / 任务签到活动 / 好友赛事排行 / 消息入口，
 * 全部跳转全站已有底座页面，斗地主模块内不新建账本、不发货币、不造伪数据。
 * 实时语音、定时赛事编排、防沉迷合规无对应底座，仅灰显标注「待产品定稿」。
 */
export function DzExtras() {
  const { dict } = useI18n();
  const t: DzDict = dict.games.doudizhu;

  const shop: Entry[] = [
    { icon: "🎁", label: t.extraShop, href: "/shop" },
    { icon: "👗", label: t.extraDressup, href: "/dressup" },
    { icon: "🖼️", label: t.extraFrames, href: "/avatar-frames" },
  ];
  const activities: Entry[] = [
    { icon: "📅", label: t.extraCheckin, href: "/checkin" },
    { icon: "🧾", label: t.extraTasks, href: "/tasks" },
    { icon: "🎉", label: t.extraSeasonal, href: "/seasonal" },
  ];
  const social: Entry[] = [
    { icon: "👥", label: t.extraFriends, href: "/friends" },
    { icon: "🏆", label: t.extraContests, href: "/contests" },
    { icon: "📊", label: t.extraTop, href: "/top" },
    { icon: "✉️", label: t.extraMessages, href: "/messages" },
  ];
  const blocked = [
    { icon: "🎙️", label: t.blockedVoice },
    { icon: "🗓️", label: t.blockedTourney },
    { icon: "🛡️", label: t.blockedGuard },
  ];

  const renderEntry = (e: Entry) => (
    <Link key={e.href} href={e.href} className="dz-extras-item">
      <span className="dz-extras-icon" aria-hidden>
        {e.icon}
      </span>
      <span className="dz-extras-label">{e.label}</span>
    </Link>
  );

  return (
    <section className="dz-extras" aria-label={t.extrasTitle}>
      <h3 className="dz-extras-title">{t.extrasTitle}</h3>
      <div className="dz-extras-group">{shop.map(renderEntry)}</div>
      <div className="dz-extras-group">{activities.map(renderEntry)}</div>
      <div className="dz-extras-group">{social.map(renderEntry)}</div>
      <div className="dz-extras-group is-blocked">
        {blocked.map((b) => (
          <div key={b.label} className="dz-extras-item is-blocked" aria-disabled>
            <span className="dz-extras-icon" aria-hidden>
              {b.icon}
            </span>
            <span className="dz-extras-label">{b.label}</span>
            <span className="dz-extras-tip">{t.blockedTip}</span>
          </div>
        ))}
      </div>
    </section>
  );
}
