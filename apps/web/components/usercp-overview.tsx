"use client";

import { useMemo } from "react";
import Link from "next/link";
import { avatarFrameStyle, FrameImageOverlay } from "@/lib/format";
import { useI18n } from "@/i18n/client";
import { MedalIcon } from "@/components/medal-icon";
import type { Overview } from "@/components/usercp";

/** 账户概览面板（从 usercp.tsx 按域拆出，300 行门禁）：
 *  资料卡 / 分享率 / 30 天登录趋势 / 摘要行 / 更多账户信息 / 最近阅读。 */
interface OverviewProps { ov: Overview | null; loading: boolean }


function fmtBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v >= 100 ? v.toFixed(0) : v.toFixed(2)} ${units[i]}`;
}

function daysSince(iso: string | null): number {
  if (!iso) return 0;
  const ms = Date.now() - new Date(iso).getTime();
  return Math.max(0, Math.floor(ms / 86400000));
}

export function OverviewTab({ ov, loading }: OverviewProps) {
  const { dict, currency } = useI18n();
  const t = dict.usercp.overview;
  // Hooks 必须先于早退（rules-of-hooks）：loading/空态判断移到 hook 之后
  // 补齐 30 天序列
  const days = useMemo(() => {
    const arr: { date: string; count: number }[] = [];
    const byDate = new Map(
      (ov?.login_trend_30d ?? []).map((d) => [d.date, d.count]),
    );
    for (let i = 29; i >= 0; i--) {
      const d = new Date(Date.now() - i * 86400000).toISOString().slice(0, 10);
      arr.push({ date: d, count: byDate.get(d) ?? 0 });
    }
    return arr;
  }, [ov?.login_trend_30d]);

  if (loading) return <p className="usercp-loading">{dict.my.loading}</p>;
  if (!ov) return null;

  const ratioText =
    ov.downloaded === 0
      ? { main: "∞", status: t.ratioNoDownload, hint: t.ratioHint }
      : {
          main: (ov.ratio ?? 0).toFixed(2),
          status: "",
          hint: "",
        };

  const trendMax = Math.max(1, ...ov.login_trend_30d.map((d) => d.count));

  const seedPct = Math.min(
    100,
    Math.round((ov.seed_points / Math.max(1, ov.next_class.required)) * 100),
  );

  return (
    <div className="usercp-dashboard">
      {/* 资料卡 */}
      <ProfileCard ov={ov} t={t} dict={dict} />

      {/* 分享率概况 */}
      <section className="uc-ratio-card" aria-label={t.ratioCard}>
        <div className="uc-ratio-card__content">
          <header className="uc-ratio-card__header">
            <span className="uc-ratio-card__label">{t.ratio}</span>
            {ratioText.status && (
              <em className="uc-ratio-card__status">{ratioText.status}</em>
            )}
          </header>
          <strong className="uc-ratio-card__value">{ratioText.main}</strong>
          {ratioText.hint && (
            <p className="uc-ratio-card__hint">{ratioText.hint}</p>
          )}
          <dl className="uc-ratio-card__metrics">
            <div className="uc-metric uc-metric--upload">
              <dt>
                <span aria-hidden="true">⇧</span>
                {t.uploaded}
              </dt>
              <dd className="num">{fmtBytes(ov.uploaded)}</dd>
            </div>
            <div className="uc-metric uc-metric--download">
              <dt>
                <span aria-hidden="true">⇩</span>
                {t.downloaded}
              </dt>
              <dd className="num">{fmtBytes(ov.downloaded)}</dd>
            </div>
            <div className="uc-metric uc-metric--active">
              <dt>
                <span aria-hidden="true">⌁</span>
                {t.active}
              </dt>
              <dd className="num">{ov.seeding + ov.leeching}</dd>
              <small>
                {t.seeding} {ov.seeding} · {t.leeching} {ov.leeching}
              </small>
            </div>
            <div className="uc-metric uc-metric--bonus">
              <dt>
                <span aria-hidden="true">★</span>
                {dict.my.balance.replace("{magic}", currency)}
              </dt>
              <dd className="num">{ov.spark_balance}</dd>
            </div>
          </dl>
        </div>
      </section>

      {/* 登录活跃趋势（30 天柱状图） */}
      <section className="uc-trend-card">
        <header>
          <h3>
            {t.trendTitle} <small>{t.trendRange}</small>
          </h3>
          <span>
            {ov.login_days_30d} {t.trendDays}
          </span>
        </header>
        <div className="uc-trend-chart" aria-label={t.trendAria}>
          {days.map((d) => (
            <i
              key={d.date}
              style={{
                height: `${Math.round((d.count / trendMax) * 100)}%`,
              }}
              title={`${d.date}：${d.count} ${t.trendUnit}`}
            />
          ))}
        </div>
      </section>

      {/* 摘要行：等级进度 / 最近登录 / 邀请配额 / 成就勋章 / 登录活动 */}
      <SummaryRow ov={ov} t={t} dict={dict} />

      {/* 更多账户信息（经典 rowhead/rowfollow 表格） */}
      <MoreInfoTable ov={ov} t={t} dict={dict} currency={currency} />

      {/* 最近阅读主题 */}
      <RecentTopics t={t} />
    </div>
  );
}

/** 资料卡（头像 + 佩戴勋章，从 OverviewTab 拆出，行数门禁 295） */
function ProfileCard({
  ov,
  t,
  dict,
}: {
  ov: Overview;
  t: Record<string, string>;
  dict: ReturnType<typeof useI18n>["dict"];
}) {
  return (
    <section className="uc-profile-card">
      <span
        className="uc-profile-card__avatar relative"
        style={
          ov.avatar_frame_image
            ? undefined
            : avatarFrameStyle(ov.avatar_frame_css)
        }
      >
        {ov.avatar_url ? (
          // eslint-disable-next-line @next/next/no-img-element
          <img src={ov.avatar_url} alt="" />
        ) : (
          <i className="uc-avatar-fallback">
            {ov.username.slice(0, 1).toUpperCase()}
          </i>
        )}
        <FrameImageOverlay url={ov.avatar_frame_image} />
      </span>
      <div>
        <h2>
          {ov.username}
          {(ov.worn_medals ?? []).slice(0, 3).map((m) => (
            <span
              key={m.name}
              className="medal-chip ml-1 align-middle"
              title={m.name}
            >
              <MedalIcon src={m.asset_ref} size={14} title={m.name} />
            </span>
          ))}
        </h2>
        <p>
          {t.joined}
          {new Date(ov.created_at ?? "").toLocaleString("zh-CN")}
          （{daysSince(ov.created_at)} {dict.usercp.days}）
        </p>
      </div>
      <Link href="/my?tab=personal">{t.editProfile} ✎</Link>
    </section>
  );
}

      {/* 摘要行：等级进度 / 最近登录 / 邀请配额 / 成就勋章 / 登录活动 */}
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

      {/* 更多账户信息（经典 rowhead/rowfollow 表格） */}
      <MoreInfoTable ov={ov} t={t} dict={dict} currency={currency} />

      {/* 最近阅读主题 */}
      <RecentTopics t={t} />
    </div>
  );
}

/** 更多账户信息（经典 rowhead/rowfollow 表格，从 OverviewTab 拆出） */
function MoreInfoTable({
  ov,
  t,
  dict,
  currency,
}: {
  ov: Overview;
  t: Record<string, string>;
  dict: ReturnType<typeof useI18n>["dict"];
  currency: string;
}) {
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

/** 最近阅读主题表头（从 OverviewTab 尾部拆出，行数门禁 295） */
function RecentTopics({ t }: { t: Record<string, string> }) {
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
