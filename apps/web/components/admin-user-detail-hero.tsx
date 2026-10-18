"use client";

/**
 * 后台用户详情·资料 Hero（对齐样图 § 用户详情，2026-10-18 从
 * admin-user-detail.tsx 按域拆出，300 行门禁）：
 * 头像 + 用户名 + 状态徽章 + meta 行 + 四项头条统计合成一张卡。
 *
 * 拆分前这些元素散在「身份条 + 账号资料 + 数据统计」三处，页面顶部没有
 * 把人钉住的锚点；上传/下载/分享率/魔力余额从数据统计卡上移到这里
 * （字段不减少，见 admin-user-detail-panels.tsx 的注释）。管理动作
 * 按钮仍留在「管理操作」卡（样本把它们排在 Hero 内，但那一组按钮背后
 * 是十几个面板状态，合并进来要让 Hero 持有一大堆 setter，故保持独立）。
 */

import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import {
  fmtBytes,
  statusLabels,
  type Detail,
} from "./admin-user-detail-shared";

// 状态徽标的公共底色（含警示红与普通两种）
const BADGE_DANGER = "rounded-full bg-coral/20 px-2 py-0.5 text-xs";
const BADGE_PLAIN = "rounded-full px-2 py-0.5 text-xs";
const BADGE_DANGER_TXT = `${BADGE_DANGER} text-danger`;

export function UserDetailHero({
  d,
  currency,
  dt,
}: {
  d: Detail;
  currency: string;
  dt: (s: string | null) => string;
}) {
  const { dict } = useI18n();
  const u = dict.userDetail;
  const STATUS_LABELS = statusLabels(u.statusLabels);
  return (
    <section className="brand-panel flex flex-wrap items-start gap-4 p-4">
      <div
        className="grid h-14 w-14 shrink-0 place-items-center rounded-full
          text-2xl font-bold text-white"
        style={{
          backgroundImage:
            "linear-gradient(135deg, var(--sky), var(--indigo))",
        }}
      >
        {d.username.slice(0, 1)}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-baseline gap-2">
          <p className="font-display text-lg font-bold text-ink">
            {d.username}
          </p>
          <span className="text-sm text-sub">
            UID {d.id} · {d.class_name ?? `LV${d.class_id}`}
          </span>
          {d.title && (
            <span className={`${BADGE_PLAIN} bg-sun/30`}>{d.title}</span>
          )}
          {d.status > 0 && (
            <span className={BADGE_DANGER_TXT}>
              {STATUS_LABELS[d.status] ?? d.status}
            </span>
          )}
          {d.suspended && (
            <span className={BADGE_DANGER_TXT}>{u.badgeSuspended}</span>
          )}
          {!d.download_enabled && (
            <span className={BADGE_DANGER_TXT}>{u.badgeNoDl}</span>
          )}
          {d.parked && (
            <span className={`${BADGE_PLAIN} bg-sky-soft`}>
              {u.badgeParked}
            </span>
          )}
          {d.donor && (
            <span className={`${BADGE_PLAIN} bg-mint/30`}>{u.badgeDonor}</span>
          )}
        </div>
        <p className="mt-1 text-xs text-sub">
          {d.email} · {u.inviter}{" "}
          {d.inviter_name ? `${d.inviter_name} (#${d.invited_by})` : "—"} ·{" "}
          {u.createdAt} {dt(d.created_at)} · {u.lastSeen}{" "}
          {dt(d.last_seen_at)} · {u.lastIp} {d.last_ip ?? "—"}
        </p>
      </div>
      <dl className="flex flex-wrap gap-x-6 gap-y-2">
        <div>
          <dt className="text-xs text-sub">{u.uploaded}</dt>
          <dd className="num font-display text-lg font-bold text-ink">
            {fmtBytes(d.uploaded)}
          </dd>
        </div>
        <div>
          <dt className="text-xs text-sub">{u.downloaded}</dt>
          <dd className="num font-display text-lg font-bold text-ink">
            {fmtBytes(d.downloaded)}
          </dd>
        </div>
        <div>
          <dt className="text-xs text-sub">{u.ratio}</dt>
          <dd className="num font-display text-lg font-bold text-ink">
            {d.downloaded > 0 ? (d.uploaded / d.downloaded).toFixed(3) : "∞"}
          </dd>
        </div>
        <div>
          <dt className="text-xs text-sub">
            {fmt(u.balance, { magic: currency })}
          </dt>
          <dd className="num font-display text-lg font-bold text-ink">
            {d.spark_balance.toLocaleString()}
          </dd>
        </div>
      </dl>
    </section>
  );
}
