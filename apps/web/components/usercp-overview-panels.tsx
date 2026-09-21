"use client";

/**
 * 账户概览子面板（从 components/usercp-overview.tsx 按域拆出，295 行门禁）：
 * SummaryRow 摘要行（等级进度/最近登录/邀请/勋章/登录活动）
 * + MoreInfoTable 更多账户信息 + RecentTopics 最近阅读。
 * 纯搬移，无逻辑改动。
 */

import Link from "next/link";
import { useI18n } from "@/i18n/client";
import type { Overview } from "@/components/usercp";

interface DictProp {
  dict: ReturnType<typeof useI18n>["dict"];
}

function daysSince(iso: string | null): number {
  if (!iso) return 0;
  const ms = Date.now() - new Date(iso).getTime();
  return Math.max(0, Math.floor(ms / 86400000));
}

/** 摘要行：等级进度 / 最近登录 / 邀请配额 / 成就勋章 / 登录活动 */
export function SummaryRow({
  ov,
  t,
  dict,
  seedPct,
}: {
  ov: Overview;
  t: Record<string, string>;
  seedPct: number;
} & DictProp) {
  return (
    <div className="uc-summary-row">
      <section className="uc-level-card">
        <h3>{t.levelTitle}</h3>
        <strong>{ov.class_name ?? "—"}</strong>
        <div>
          <i style={{ width: `${seedPct}%` }} />
        </div>
        <p>
          {t.seedPoints} {ov.seed_points.toFixed(1)} /{" "}
          {ov.next_class.required.toLocaleString()}，{t.nextClass}{" "}
          {ov.next_class.name} {t.still}
          {(ov.next_class.required - ov.seed_points).toFixed(1)}
        </p>
      </section>
      <section>
        <h3>{t.lastLoginTitle}</h3>
        <strong>
          {ov.last_login
            ? ov.last_login.replace("T", " ").slice(0, 19)
            : "—"}
        </strong>
        <p>Web</p>
        <Link href="/my?tab=security">{t.checkSecurity} ›</Link>
      </section>
      <section>
        <h3>{t.inviteTitle}</h3>
        <strong>
          {ov.invites_pending} {t.inviteUnit}
        </strong>
        <p>
          {t.invited} {ov.invites_pending + ov.invites_used} ·{" "}
          {t.registered}
          {ov.invites_used}
        </p>
        <Link href="/invites">{t.manageInvites} ›</Link>
      </section>
      <section>
        <h3>{t.medalTitle}</h3>
        <strong>{ov.medals} {t.medalUnit}</strong>
        <p>{ov.medals > 0 ? "" : t.noMedal}</p>
        <Link href="/medals">{t.viewMedals} ›</Link>
      </section>
      <section>
        <h3>{t.loginActivityTitle}</h3>
        <strong>{ov.login_total_30d} {t.times}</strong>
        <p>
          {t.last30d} · {ov.totp_enabled ? t.twofaOn : t.twofaOff} ·{" "}
          {dict.usercp.security.passkeyLabel} {ov.passkey.slice(0, 4)}••••
        </p>
        <Link href="/my?tab=security">{t.accountSecurity} ›</Link>
      </section>
    </div>
  );
}

/** 更多账户信息（经典 rowhead/rowfollow 表格） */
export function MoreInfoTable({
  ov,
  t,
  dict,
  currency,
}: {
  ov: Overview;
  t: Record<string, string>;
  currency: string;
} & DictProp) {
  return (
    <details className="uc-more" open>
      <summary>{t.moreTitle}</summary>
      <table className="nexus-table nexus-form">
        <tbody>
          <tr>
            <td className="rowhead">{t.joinDate}</td>
            <td className="rowfollow">
              {new Date(ov.created_at ?? "").toLocaleString("zh-CN")}（
              {daysSince(ov.created_at)}
              {dict.usercp.days2}）
            </td>
          </tr>
          <tr>
            <td className="rowhead">{t.email}</td>
            <td className="rowfollow">{ov.email}</td>
          </tr>
          <tr>
            <td className="rowhead">{t.ip}</td>
            <td className="rowfollow">
              <span className="uc-hidden-text">
                {ov.passkey ? "127.0.0.1" : "—"}
              </span>
            </td>
          </tr>
          <tr>
            <td className="rowhead">{t.passkeyRow}</td>
            <td className="rowfollow">
              <span className="uc-hidden-text">{ov.passkey}</span>
            </td>
          </tr>
          <tr>
            <td className="rowhead">{t.inviteRow}</td>
            <td className="rowfollow">
              {ov.invites_pending} [<Link href="/invites">{t.send}</Link>]
            </td>
          </tr>
          <tr>
            <td className="rowhead">
              {dict.my.balance.replace("{magic}", currency)}
            </td>
            <td className="rowfollow">
              {ov.spark_balance} [<Link href="/shop">{t.use}</Link>]
            </td>
          </tr>
          <tr>
            <td className="rowhead">{t.commentsRow}</td>
            <td className="rowfollow">
              {ov.comments}[
              <Link href={`/users/${ov.id}#comments`}>{t.view}</Link>]
            </td>
          </tr>
          <tr>
            <td className="rowhead">{t.tokenRow}</td>
            <td className="rowfollow">
              <Link href="/my?tab=security" className="sticker">
                {t.create}
              </Link>
            </td>
          </tr>
        </tbody>
      </table>
    </details>
  );
}

/** 最近阅读主题表头 */
export function RecentTopics({ t }: { t: Record<string, string> }) {
  return (
    <section className="uc-recent-topics">
      <h2>{t.recentTopics}</h2>
      <div className="uc-recent-topics__wrap">
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead" style={{ width: "80%" }}>
                {t.colTopic}
              </td>
              <td className="colhead" style={{ textAlign: "center" }}>
                {t.colRepliesViews}
              </td>
              <td className="colhead" style={{ textAlign: "center" }}>
                {t.colStarter}
              </td>
              <td
                className="colhead"
                style={{ width: "20%", textAlign: "center" }}
              >
                {t.colLastPost}
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>
  );
}
