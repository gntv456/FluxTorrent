"use client";

import { useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { LOCALE_COOKIE, type Locale } from "@/i18n/config";
import { PushSettings } from "@/components/push-settings";
import { TwoFactorSetup } from "@/components/twofa-setup";
import { ApiTokens } from "@/components/api-tokens";
import { NoticePrefsCard } from "@/components/notice-prefs";
import { WishlistPanel } from "@/components/wishlist";

/** 控制面板 —— 像素级复刻 NexusPHP usercp（参考站）：
 *  左侧 ⚙控制面板 六项侧边导航（账户概览/个人资料/网站设定/论坛设定/安全设定）
 *  + 各 tab 的 rowhead/rowfollow 经典表格表单，字段与旧站一一对应。 */

export type UsercpTab =
  | "overview"
  | "personal"
  | "tracker"
  | "forum"
  | "security"
  | "wishlist";

export const USERCP_NAV: { key: UsercpTab; icon: string }[] = [
  { key: "overview", icon: "⌂" },
  { key: "personal", icon: "♙" },
  { key: "tracker", icon: "⚙" },
  { key: "forum", icon: "♧" },
  { key: "security", icon: "⌾" },
  { key: "wishlist", icon: "☆" },
];

// ============ 站点语言 ⇄ 前端 locale 映射 ============
// 库里存 NexusPHP 风格码（en/chs/cht），前端 i18n 用 BCP47（zh-CN/zh-TW/en）。
// 两套体系此前割裂：控制面板改语言只落库，页面语言纹丝不动 —— 修复为保存时同步 cookie 并刷新。

const LANG_TO_LOCALE: Record<string, Locale> = {
  en: "en",
  chs: "zh-CN",
  cht: "zh-TW",
};
const LOCALE_TO_LANG: Record<Locale, string> = {
  en: "en",
  "zh-CN": "chs",
  "zh-TW": "cht",
};

function syncLocaleCookie(locale: Locale) {
  document.cookie = `${LOCALE_COOKIE}=${locale}; path=/; max-age=31536000; samesite=lax`;
}

// ============ 类型 ============

export interface UserSettings {
  parked: boolean;
  accept_pm: string;
  delete_pm: boolean;
  save_pm: boolean;
  comment_pm: boolean;
  notify_topic_reply: boolean;
  notify_hr: boolean;
  gender: number;
  country: number;
  download_speed: number;
  upload_speed: number;
  isp: number;
  info: string | null;
  avatar_url: string | null;
  browsecat: string | null;
  stylesheet: string;
  fontsize: string;
  site_language: string;
  pm_per_page: number;
  show_description: boolean;
  show_imdb: boolean;
  show_comment: boolean;
  show_ad: boolean;
  time_type: string;
  torrents_per_page: number;
  incl_dead: number;
  sp_state: number;
  incl_bookmarked: number;
  tooltip: string;
  append_sticky: boolean;
  append_new: boolean;
  append_promotion: string;
  append_picked: boolean;
  small_descr: boolean;
  dl_icon: boolean;
  bm_icon: boolean;
  show_com_num: boolean;
  show_last_com: string;
  topics_per_page: number;
  posts_per_page: number;
  view_avatars: boolean;
  view_signatures: boolean;
  tt_last_post: boolean;
  click_topic: string;
  signature: string | null;
  privacy: string;
}

interface Overview {
  id: number;
  username: string;
  email: string;
  class_name: string | null;
  avatar_url: string | null;
  created_at: string | null;
  uploaded: number;
  downloaded: number;
  ratio: number | null;
  seeding: number;
  leeching: number;
  uploads: number;
  comments: number;
  spark_balance: number;
  invites_pending: number;
  invites_used: number;
  medals: number;
  parked: boolean;
  privacy: string;
  totp_enabled: boolean;
  passkey: string;
  login_trend_30d: { date: string; count: number }[];
  login_days_30d: number;
  login_total_30d: number;
  last_login: string | null;
  seed_points: number;
  next_class: { name: string; required: number };
}

// 旧站口径的选项表（值与 NexusPHP 一致）
const BANDWIDTH = [
  "64kbps", "128kbps", "256kbps", "512kbps", "768kbps", "1Mbps", "1.5Mbps",
  "2Mbps", "3Mbps", "4Mbps", "5Mbps", "6Mbps", "7Mbps", "8Mbps", "9Mbps",
  "10Mbps", "48Mbps", "100Mbit",
];
const ISPS = ["中国电信", "中国网通", "中国铁通", "中国移动", "中国联通", "中国教育网", "Other"];
const COUNTRIES = [
  "Sweden", "United States of America", "Russia", "Finland", "Canada", "France",
  "Germany", "Algeria", "Angola", "Argentina", "Australia", "Austria", "Bahamas",
  "Bangladesh", "Barbados", "Belgium", "Brazil", "Bulgaria", "Cambodia", "Chile",
  "China", "Colombia", "Congo", "Costa Rica", "Croatia", "Cuba", "Czech Republic",
  "Denmark", "Dominican Republic", "Ecuador", "Egypt", "Estonia", "Greece",
  "Guatemala", "Honduras", "Hungary", "Iceland", "India", "Ireland", "Israel",
  "Italy", "Jamaica", "Japan", "Kiribati", "Laos", "Latvia", "Lebanon",
  "Lithuania", "Luxembourg", "Malaysia", "Mexico", "Nauru", "Netherlands",
  "Netherlands Antilles", "New Zealand", "Nigeria", "North Korea", "Norway",
  "Pakistan", "Paraguay", "Peru", "Philippines", "Poland", "Portugal",
  "Puerto Rico", "Romania", "Senegal", "Serbia", "Seychelles", "Singapore",
  "Slovenia", "South Africa", "South Korea", "Spain", "Switzerland", "Thailand",
  "Togo", "Trinidad & Tobago", "Turkey", "Turkmenistan", "Ukraine",
  "United Kingdom", "Uruguay", "Uzbekistan", "Vanuatu", "Venezuela", "Vietnam",
  "Western Samoa", "Yugoslavia",
];

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
  return Math.max(0, Math.floor((Date.now() - new Date(iso).getTime()) / 86400000));
}

// ============ 主组件 ============

export function UsercpPanel({ initialTab }: { initialTab: UsercpTab }) {
  const { dict, locale } = useI18n();
  const router = useRouter();
  const t = dict.usercp;
  const [tab, setTab] = useState<UsercpTab>(initialTab);
  const [settings, setSettings] = useState<UserSettings | null>(null);
  const [overview, setOverview] = useState<Overview | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setTab(initialTab);
  }, [initialTab]);

  useEffect(() => {
    setErr(null);
    api
      .get<UserSettings>("/api/v1/me/settings")
      .then((s) => {
        setSettings(s);
        // 反向一致：库里语言与当前 locale 不一致（如别的设备改过）→ 以库为准同步 cookie
        const want = LANG_TO_LOCALE[s.site_language];
        if (want && want !== locale) {
          syncLocaleCookie(want);
          router.refresh();
        }
      })
      .catch((e) =>
        setErr(e instanceof ApiError ? e.message : dict.common.loadFailed),
      );
    if (tab === "overview") {
      api
        .get<Overview>("/api/v1/me/overview")
        .then(setOverview)
        .catch(() => setOverview(null));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab]);

  const patch = (p: Partial<UserSettings>) => {
    if (settings) setSettings({ ...settings, ...p });
    setSaved(false);
  };

  const save = async () => {
    if (!settings) return;
    setSaving(true);
    setSaved(false);
    try {
      await api.put("/api/v1/me/settings", settings);
      setSaved(true);
      // 站点语言变更 → 同步前端 locale cookie 并刷新整站（RSC 重取词典）
      const want = LANG_TO_LOCALE[settings.site_language];
      if (want && want !== locale) {
        syncLocaleCookie(want);
        router.refresh();
      }
    } catch (e) {
      setErr(e instanceof ApiError ? e.message : dict.common.loadFailed);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="usercp-wrap">
      {/* 侧边导航：⚙ 控制面板 + 五项 */}
      <aside className="usercp-nav">
        <h1>
          <span aria-hidden="true">⚙</span>
          {t.navTitle}
        </h1>
        {USERCP_NAV.map((n) => (
          <Link
            key={n.key}
            href={n.key === "overview" ? "/my?tab=overview" : `/my?tab=${n.key}`}
            data-active={tab === n.key ? "true" : undefined}
            onClick={(e) => {
              e.preventDefault();
              window.history.replaceState(null, "", `/my?tab=${n.key}`);
              setTab(n.key);
            }}
          >
            <span aria-hidden="true">{n.icon}</span>
            {t.tabs[n.key]}
          </Link>
        ))}
      </aside>

      <div className="usercp-content">
        {err && <p className="usercp-error">{err}</p>}
        {tab === "overview" ? (
          <OverviewTab ov={overview} loading={!overview && !err} />
        ) : tab === "wishlist" ? (
          <WishlistPanel />
        ) : settings ? (
          <form
            className="usercp-form"
            onSubmit={(e) => {
              e.preventDefault();
              void save();
            }}
          >
            {tab === "personal" && <PersonalTab s={settings} patch={patch} />}
            {tab === "tracker" && <TrackerTab s={settings} patch={patch} />}
            {tab === "forum" && <ForumTab s={settings} patch={patch} />}
            {tab === "security" && <SecurityTab s={settings} patch={patch} />}
            <table className="nexus-table nexus-form">
              <tbody>
                <tr>
                  <td className="rowhead">{t.saveRow}</td>
                  <td className="rowfollow">
                    <input
                      type="submit"
                      value={t.saveBtn}
                      disabled={saving}
                    />
                    {saved && <span className="usercp-saved">{t.savedOk}</span>}
                  </td>
                </tr>
              </tbody>
            </table>
          </form>
        ) : (
          !err && <p className="usercp-loading">{dict.my.loading}</p>
        )}
      </div>
    </div>
  );
}

// ============ 账户概览 ============

function OverviewTab({ ov, loading }: { ov: Overview | null; loading: boolean }) {
  const { dict } = useI18n();
  const t = dict.usercp.overview;
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
  // 补齐 30 天序列
  const days = useMemo(() => {
    const arr: { date: string; count: number }[] = [];
    const byDate = new Map(ov.login_trend_30d.map((d) => [d.date, d.count]));
    for (let i = 29; i >= 0; i--) {
      const d = new Date(Date.now() - i * 86400000).toISOString().slice(0, 10);
      arr.push({ date: d, count: byDate.get(d) ?? 0 });
    }
    return arr;
  }, [ov.login_trend_30d]);

  const seedPct = Math.min(
    100,
    Math.round((ov.seed_points / Math.max(1, ov.next_class.required)) * 100),
  );

  return (
    <div className="usercp-dashboard">
      {/* 资料卡 */}
      <section className="uc-profile-card">
        <span className="uc-profile-card__avatar">
          {ov.avatar_url ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img src={ov.avatar_url} alt="" />
          ) : (
            <i className="uc-avatar-fallback">{ov.username.slice(0, 1).toUpperCase()}</i>
          )}
        </span>
        <div>
          <h2>{ov.username}</h2>
          <p>
            {t.joined}
            {new Date(ov.created_at ?? "").toLocaleString("zh-CN")}
            （{daysSince(ov.created_at)} {dict.usercp.days}）
          </p>
        </div>
        <Link href="/my?tab=personal">{t.editProfile} ✎</Link>
      </section>

      {/* 分享率概况 */}
      <section className="uc-ratio-card" aria-label={t.ratioCard}>
        <div className="uc-ratio-card__content">
          <header className="uc-ratio-card__header">
            <span className="uc-ratio-card__label">{t.ratio}</span>
            {ratioText.status && <em className="uc-ratio-card__status">{ratioText.status}</em>}
          </header>
          <strong className="uc-ratio-card__value">{ratioText.main}</strong>
          {ratioText.hint && <p className="uc-ratio-card__hint">{ratioText.hint}</p>}
          <dl className="uc-ratio-card__metrics">
            <div className="uc-metric uc-metric--upload">
              <dt><span aria-hidden="true">⇧</span>{t.uploaded}</dt>
              <dd className="num">{fmtBytes(ov.uploaded)}</dd>
            </div>
            <div className="uc-metric uc-metric--download">
              <dt><span aria-hidden="true">⇩</span>{t.downloaded}</dt>
              <dd className="num">{fmtBytes(ov.downloaded)}</dd>
            </div>
            <div className="uc-metric uc-metric--active">
              <dt><span aria-hidden="true">⌁</span>{t.active}</dt>
              <dd className="num">{ov.seeding + ov.leeching}</dd>
              <small>
                {t.seeding} {ov.seeding} · {t.leeching} {ov.leeching}
              </small>
            </div>
            <div className="uc-metric uc-metric--bonus">
              <dt><span aria-hidden="true">★</span>{dict.my.balance}</dt>
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
              style={{ height: `${Math.round((d.count / trendMax) * 100)}%` }}
              title={`${d.date}：${d.count} ${t.trendUnit}`}
            />
          ))}
        </div>
      </section>

      {/* 摘要行：等级进度 / 最近登录 / 邀请配额 / 成就勋章 / 登录活动 */}
      <div className="uc-summary-row">
        <section className="uc-level-card">
          <h3>{t.levelTitle}</h3>
          <strong>
            {ov.class_name ?? "—"}
          </strong>
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
            {ov.last_login ? ov.last_login.replace("T", " ").slice(0, 19) : "—"}
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
            {t.invited} {ov.invites_pending + ov.invites_used} · {t.registered}{" "}
            {ov.invites_used}
          </p>
          <Link href="/invites">{t.manageInvites} ›</Link>
        </section>
        <section>
          <h3>{t.medalTitle}</h3>
          <strong>
            {ov.medals} {t.medalUnit}
          </strong>
          <p>{ov.medals > 0 ? "" : t.noMedal}</p>
          <Link href="/medals">{t.viewMedals} ›</Link>
        </section>
        <section>
          <h3>{t.loginActivityTitle}</h3>
          <strong>
            {ov.login_total_30d} {t.times}
          </strong>
          <p>
            {t.last30d} · {ov.totp_enabled ? t.twofaOn : t.twofaOff} ·{" "}
            {dict.usercp.security.passkeyLabel} {ov.passkey.slice(0, 4)}••••
          </p>
          <Link href="/my?tab=security">{t.accountSecurity} ›</Link>
        </section>
      </div>

      {/* 更多账户信息（经典 rowhead/rowfollow 表格） */}
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
                <span className="uc-hidden-text">{ov.passkey ? "127.0.0.1" : "—"}</span>
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
              <td className="rowhead">{dict.my.balance}</td>
              <td className="rowfollow">
                {ov.spark_balance} [<Link href="/shop">{t.use}</Link>]
              </td>
            </tr>
            <tr>
              <td className="rowhead">{t.commentsRow}</td>
              <td className="rowfollow">
                {ov.comments} [<Link href={`/users/${ov.id}#comments`}>{t.view}</Link>]
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

      {/* 最近阅读主题 */}
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
                <td className="colhead" style={{ width: "20%", textAlign: "center" }}>
                  {t.colLastPost}
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>
    </div>
  );
}

// ============ 个人资料 ============

function Row({
  head,
  children,
}: {
  head: string;
  children: React.ReactNode;
}) {
  return (
    <tr>
      <td className="rowhead nowrap">{head}</td>
      <td className="rowfollow">{children}</td>
    </tr>
  );
}

function PersonalTab({
  s,
  patch,
}: {
  s: UserSettings;
  patch: (p: Partial<UserSettings>) => void;
}) {
  const { dict } = useI18n();
  const t = dict.usercp.personal;
  return (
    <table className="nexus-table nexus-form">
      <tbody>
        <Row head={t.parked}>
          <label>
            <input
              type="checkbox"
              checked={s.parked}
              onChange={(e) => patch({ parked: e.target.checked })}
            />
            {t.parkedLabel}
          </label>
          <br />
          <span className="uc-note">
            <b>{dict.usercp.note}</b>
            {t.parkedNote}
          </span>
        </Row>
        <Row head={t.pm}>
          {t.pmAccept}
          <label>
            <input
              type="radio"
              name="accept_pm"
              checked={s.accept_pm === "yes"}
              onChange={() => patch({ accept_pm: "yes" })}
            />
            {t.pmAll}
          </label>
          <label>
            <input
              type="radio"
              name="accept_pm"
              checked={s.accept_pm === "friends"}
              onChange={() => patch({ accept_pm: "friends" })}
            />
            {t.pmFriends}
          </label>
          <label>
            <input
              type="radio"
              name="accept_pm"
              checked={s.accept_pm === "no"}
              onChange={() => patch({ accept_pm: "no" })}
            />
            {t.pmStaff}
          </label>
          <br />
          <label>
            <input
              type="checkbox"
              checked={s.delete_pm}
              onChange={(e) => patch({ delete_pm: e.target.checked })}
            />{" "}
            {t.pmDelete}
          </label>
          <br />
          <label>
            <input
              type="checkbox"
              checked={s.save_pm}
              onChange={(e) => patch({ save_pm: e.target.checked })}
            />{" "}
            {t.pmSave}
          </label>
          <br />
          <label>
            <input
              type="checkbox"
              checked={s.comment_pm}
              onChange={(e) => patch({ comment_pm: e.target.checked })}
            />{" "}
            {t.pmComment}
          </label>
          <br />
          <label>
            <input
              type="checkbox"
              checked={s.notify_topic_reply}
              onChange={(e) => patch({ notify_topic_reply: e.target.checked })}
            />{" "}
            {t.pmTopicReply}
          </label>
          <br />
          <label>
            <input
              type="checkbox"
              checked={s.notify_hr}
              onChange={(e) => patch({ notify_hr: e.target.checked })}
            />{" "}
            {t.pmHr}
          </label>
        </Row>
        <Row head={t.gender}>
          <label>
            <input
              type="radio"
              name="gender"
              checked={s.gender === 0}
              onChange={() => patch({ gender: 0 })}
            />
            {t.genderNA}
          </label>
          <label>
            <input
              type="radio"
              name="gender"
              checked={s.gender === 1}
              onChange={() => patch({ gender: 1 })}
            />
            {t.genderMale}
          </label>
          <label>
            <input
              type="radio"
              name="gender"
              checked={s.gender === 2}
              onChange={() => patch({ gender: 2 })}
            />
            {t.genderFemale}
          </label>
        </Row>
        <Row head={t.trackerUrl}>
          <select
            value={s.country >= 0 ? "0" : "0"}
            onChange={() => undefined}
          >
            <option value="0">---- {t.notSelected} ----</option>
            <option value="1">{t.trackerAnnounceFallback}</option>
          </select>
          <br />
          <span className="uc-note">
            <b>{dict.usercp.note}</b>
            {t.trackerNote}
          </span>
        </Row>
        <Row head={t.country}>
          <select
            value={String(s.country)}
            onChange={(e) => patch({ country: Number(e.target.value) })}
          >
            <option value="0">---- {t.notSelected} ----</option>
            {COUNTRIES.map((c, i) => (
              <option key={c} value={i + 1}>
                {c}
              </option>
            ))}
          </select>
        </Row>
        <Row head={t.bandwidth}>
          <b>{t.downBand}</b>:{" "}
          <select
            value={String(s.download_speed)}
            onChange={(e) => patch({ download_speed: Number(e.target.value) })}
          >
            <option value="0">---- {t.notSelected} ----</option>
            {BANDWIDTH.map((b, i) => (
              <option key={b} value={i + 1}>
                {b}
              </option>
            ))}
          </select>{" "}
          <b>{t.upBand}</b>:{" "}
          <select
            value={String(s.upload_speed)}
            onChange={(e) => patch({ upload_speed: Number(e.target.value) })}
          >
            <option value="0">---- {t.notSelected} ----</option>
            {BANDWIDTH.map((b, i) => (
              <option key={b} value={i + 1}>
                {b}
              </option>
            ))}
          </select>{" "}
          <b>{t.isp}</b>:{" "}
          <select value={String(s.isp)} onChange={(e) => patch({ isp: Number(e.target.value) })}>
            <option value="0">---- {t.notSelected} ----</option>
            {ISPS.map((c, i) => (
              <option key={c} value={i === 6 ? 20 : i + 1}>
                {c}
              </option>
            ))}
          </select>
        </Row>
        <Row head={t.avatar}>
          {s.avatar_url ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img src={s.avatar_url} alt="" className="uc-avatar-preview" />
          ) : (
            <span className="uc-avatar-preview uc-avatar-preview--empty" />
          )}
          <br />
          <input
            type="text"
            className="uc-input-wide"
            placeholder="https://"
            value={s.avatar_url ?? ""}
            onChange={(e) => patch({ avatar_url: e.target.value })}
          />
          <br />
          {t.avatarNote}
        </Row>
        <Row head={t.info}>
          <textarea
            className="uc-textarea"
            rows={10}
            value={s.info ?? ""}
            onChange={(e) => patch({ info: e.target.value })}
          />
          <br />
          {t.infoNote}
        </Row>
      </tbody>
    </table>
  );
}

// ============ 网站设定 ============

const TRACKER_CATS = [
  { id: "401", name: "电影" },
  { id: "402", name: "剧集" },
  { id: "403", name: "综艺" },
  { id: "404", name: "纪录片" },
  { id: "405", name: "动漫" },
  { id: "406", name: "音乐视频" },
  { id: "407", name: "体育运动" },
  { id: "408", name: "高品质音频" },
  { id: "410", name: "短剧" },
  { id: "409", name: "其他" },
];

function TrackerTab({
  s,
  patch,
}: {
  s: UserSettings;
  patch: (p: Partial<UserSettings>) => void;
}) {
  const { dict } = useI18n();
  const t = dict.usercp.tracker;
  const cats = (s.browsecat ?? "").split(",").filter(Boolean);
  const toggleCat = (id: string, on: boolean) => {
    const next = on ? [...new Set([...cats, id])] : cats.filter((c) => c !== id);
    patch({ browsecat: next.join(",") });
  };
  return (
    <table className="nexus-table nexus-form">
      <tbody>
        <Row head={t.defaultCat}>
          <div className="uc-cat-groups">
            <section className="uc-cat-card">
              <table>
                <caption>
                  <b>{t.catSection}</b>
                </caption>
                <tbody>
                  <tr>
                    <td className="bottom" colSpan={5}>
                      {TRACKER_CATS.map((c) => (
                        <label key={c.id} className="uc-cat-item">
                          <input
                            type="checkbox"
                            checked={cats.includes(c.id)}
                            onChange={(e) => toggleCat(c.id, e.target.checked)}
                          />
                          {c.name}
                        </label>
                      ))}
                    </td>
                  </tr>
                </tbody>
              </table>
            </section>
            <section className="uc-cat-card">
              <table>
                <caption>
                  <b>{t.extraSection}</b>
                </caption>
                <tbody>
                  <tr>
                    <td className="bottom">
                      <b>{t.showDead}</b>
                      <br />
                      <select
                        value={String(s.incl_dead)}
                        onChange={(e) => patch({ incl_dead: Number(e.target.value) })}
                      >
                        <option value="0">{t.deadBoth}</option>
                        <option value="1">{t.deadAlive}</option>
                        <option value="2">{t.deadDead}</option>
                      </select>
                    </td>
                    <td className="bottom">
                      <b>{t.showPromo}</b>
                      <br />
                      <select
                        value={String(s.sp_state)}
                        onChange={(e) => patch({ sp_state: Number(e.target.value) })}
                      >
                        <option value="0">{t.promoAll}</option>
                        <option value="1">{t.promoNormal}</option>
                        <option value="2">{dict.promotion.free}</option>
                        <option value="3">2X</option>
                        <option value="4">2X{dict.promotion.free}</option>
                        <option value="5">{dict.promotion.half}</option>
                        <option value="6">2X {dict.promotion.half}</option>
                        <option value="7">{dict.promotion.p30}</option>
                      </select>
                    </td>
                    <td className="bottom">
                      <b>{t.showBookmark}</b>
                      <br />
                      <select
                        value={String(s.incl_bookmarked)}
                        onChange={(e) =>
                          patch({ incl_bookmarked: Number(e.target.value) })
                        }
                      >
                        <option value="0">{t.bmAll}</option>
                        <option value="1">{t.bmOnly}</option>
                        <option value="2">{t.bmNone}</option>
                      </select>
                    </td>
                  </tr>
                </tbody>
              </table>
            </section>
          </div>
        </Row>
        <Row head={t.stylesheet}>
          <select
            value={s.stylesheet}
            onChange={(e) => patch({ stylesheet: e.target.value })}
          >
            <option value="FluxTorrent">FluxTorrent</option>
          </select>{" "}
          <span className="uc-note">
            {t.stylesheetMore}
            <a>{dict.usercp.clickHere}</a>
          </span>
        </Row>
        <Row head={t.fontsize}>
          <select
            value={s.fontsize}
            onChange={(e) => patch({ fontsize: e.target.value })}
          >
            <option value="small">{t.small}</option>
            <option value="medium">{t.medium}</option>
            <option value="large">{t.large}</option>
          </select>
        </Row>
        <Row head={t.language}>
          <select
            value={s.site_language}
            onChange={(e) => patch({ site_language: e.target.value })}
          >
            <option value="en">English</option>
            <option value="chs">简体中文</option>
            <option value="cht">繁體中文</option>
          </select>{" "}
          <span className="uc-note">
            {t.languageMore}
            <a>{dict.usercp.helpTranslate}</a>
          </span>
        </Row>
        <Row head={t.pmPerPage}>
          {t.pmPerPagePrefix}
          <input
            type="text"
            size={5}
            className="uc-num-input"
            value={s.pm_per_page}
            onChange={(e) => patch({ pm_per_page: Number(e.target.value) || 0 })}
          />
          {t.pmPerPageSuffix}
        </Row>
        <Row head={t.detailPage}>
          <label>
            <input
              type="checkbox"
              checked={s.show_description}
              onChange={(e) => patch({ show_description: e.target.checked })}
            />
            {t.showDescr}
          </label>
          <br />
          <label>
            <input
              type="checkbox"
              checked={s.show_imdb}
              onChange={(e) => patch({ show_imdb: e.target.checked })}
            />
            {t.showImdb}
          </label>
        </Row>
        <Row head={t.discussion}>
          <label>
            <input
              type="checkbox"
              checked={s.show_comment}
              onChange={(e) => patch({ show_comment: e.target.checked })}
            />
            {t.showComments}
          </label>
        </Row>
        <Row head={t.showAd}>
          <label>
            <input type="checkbox" checked disabled />
            {t.wantAds}
          </label>
          <br />
          <b className="uc-uploader">发布员</b>
          {t.adNote1}
          <br />
          <b className="uc-poweruser">Power User</b>
          {t.adNote2}
          <a>{dict.usercp.clickHere}</a>
        </Row>
        <Row head={t.timeType}>
          <label>
            <input
              type="radio"
              name="time_type"
              checked={s.time_type === "timeadded"}
              onChange={() => patch({ time_type: "timeadded" })}
            />
            {t.timeAdded}
          </label>
          <label>
            <input
              type="radio"
              name="time_type"
              checked={s.time_type === "timealive"}
              onChange={() => patch({ time_type: "timealive" })}
            />
            {t.timeAlive}
          </label>
        </Row>
        <Row head={t.torrentPage}>
          <div className="uc-browse-settings">
            <p className="uc-browse-warning">
              <b>{dict.usercp.warning}</b>
              {t.browseWarn}
            </p>
            <div className="uc-browse-grid">
              <section>
                <h3>{t.perPageTitle}</h3>
                <div>
                  {t.perPagePrefix}
                  <input
                    type="text"
                    size={5}
                    className="uc-num-input"
                    value={s.torrents_per_page}
                    onChange={(e) =>
                      patch({ torrents_per_page: Number(e.target.value) || 0 })
                    }
                  />
                  {t.perPageSuffix}
                </div>
              </section>
              <section>
                <h3>{t.tooltipTitle}</h3>
                <label>
                  <input
                    type="radio"
                    name="tooltip"
                    checked={s.tooltip === "minorimdb"}
                    onChange={() => patch({ tooltip: "minorimdb" })}
                  />
                  {t.tooltipMinor}
                </label>
                <label>
                  <input
                    type="radio"
                    name="tooltip"
                    checked={s.tooltip === "medianimdb"}
                    onChange={() => patch({ tooltip: "medianimdb" })}
                  />
                  {t.tooltipMedian}
                </label>
                <label>
                  <input
                    type="radio"
                    name="tooltip"
                    checked={s.tooltip === "off"}
                    onChange={() => patch({ tooltip: "off" })}
                  />
                  {t.tooltipOff}
                </label>
              </section>
              <section>
                <h3>{t.specialTitle}</h3>
                <label>
                  <input
                    type="checkbox"
                    checked={s.append_sticky}
                    onChange={(e) => patch({ append_sticky: e.target.checked })}
                  />
                  {t.specialSticky}
                </label>
                <label>
                  <input
                    type="checkbox"
                    checked={s.append_new}
                    onChange={(e) => patch({ append_new: e.target.checked })}
                  />
                  {t.specialNew}
                </label>
                <div className="uc-option-line">
                  <span>{t.specialPromo}：</span>
                  <label>
                    <input
                      type="radio"
                      name="append_promotion"
                      checked={s.append_promotion === "highlight"}
                      onChange={() => patch({ append_promotion: "highlight" })}
                    />
                    {t.promoHighlight}
                  </label>
                  <label>
                    <input
                      type="radio"
                      name="append_promotion"
                      checked={s.append_promotion === "word"}
                      onChange={() => patch({ append_promotion: "word" })}
                    />
                    {t.promoWord}
                  </label>
                  <label>
                    <input
                      type="radio"
                      name="append_promotion"
                      checked={s.append_promotion === "icon"}
                      onChange={() => patch({ append_promotion: "icon" })}
                    />
                    {t.promoIcon}
                  </label>
                  <label>
                    <input
                      type="radio"
                      name="append_promotion"
                      checked={s.append_promotion === "off"}
                      onChange={() => patch({ append_promotion: "off" })}
                    />
                    {t.promoOff}
                  </label>
                </div>
                <label>
                  <input
                    type="checkbox"
                    checked={s.append_picked}
                    onChange={(e) => patch({ append_picked: e.target.checked })}
                  />
                  {t.specialPicked}
                </label>
              </section>
              <section>
                <h3>{t.titleTitle}</h3>
                <label>
                  <input
                    type="checkbox"
                    checked={s.small_descr}
                    onChange={(e) => patch({ small_descr: e.target.checked })}
                  />
                  {t.showSubtitle}
                </label>
              </section>
              <section>
                <h3>{t.actionIconsTitle}</h3>
                <label>
                  <input
                    type="checkbox"
                    checked={s.dl_icon}
                    onChange={(e) => patch({ dl_icon: e.target.checked })}
                  />
                  {t.dlIcon}
                </label>
                <label>
                  <input
                    type="checkbox"
                    checked={s.bm_icon}
                    onChange={(e) => patch({ bm_icon: e.target.checked })}
                  />
                  {t.bmIcon}
                </label>
              </section>
              <section>
                <h3>{t.commentsTitle}</h3>
                <div className="uc-option-line">
                  <label>
                    <input
                      type="checkbox"
                      checked={s.show_com_num}
                      onChange={(e) => patch({ show_com_num: e.target.checked })}
                    />
                    {t.showComNum}
                  </label>
                  <select
                    value={s.show_last_com}
                    onChange={(e) => patch({ show_last_com: e.target.value })}
                  >
                    <option value="yes">{t.and}</option>
                    <option value="no">{t.butNot}</option>
                  </select>
                  <span>{t.hoverLastCom}</span>
                </div>
              </section>
            </div>
          </div>
        </Row>
      </tbody>
    </table>
  );
}

// ============ 论坛设定 ============

function ForumTab({
  s,
  patch,
}: {
  s: UserSettings;
  patch: (p: Partial<UserSettings>) => void;
}) {
  const { dict } = useI18n();
  const t = dict.usercp.forum;
  return (
    <table className="nexus-table nexus-form">
      <tbody>
        <Row head={t.topicsPerPage}>
          <input
            type="text"
            size={10}
            className="uc-num-input"
            value={s.topics_per_page}
            onChange={(e) => patch({ topics_per_page: Number(e.target.value) || 0 })}
          />
          {t.zeroDefault}
        </Row>
        <Row head={t.postsPerPage}>
          <input
            type="text"
            size={10}
            className="uc-num-input"
            value={s.posts_per_page}
            onChange={(e) => patch({ posts_per_page: Number(e.target.value) || 0 })}
          />{" "}
          {t.zeroDefault}
        </Row>
        <Row head={t.viewAvatars}>
          <label>
            <input
              type="checkbox"
              checked={s.view_avatars}
              onChange={(e) => patch({ view_avatars: e.target.checked })}
            />
            {t.lowBandwidth}
          </label>
        </Row>
        <Row head={t.viewSignatures}>
          <label>
            <input
              type="checkbox"
              checked={s.view_signatures}
              onChange={(e) => patch({ view_signatures: e.target.checked })}
            />
            {t.lowBandwidth}
          </label>
        </Row>
        <Row head={t.hoverLastPost}>
          <label>
            <input
              type="checkbox"
              checked={s.tt_last_post}
              onChange={(e) => patch({ tt_last_post: e.target.checked })}
            />
            {t.hoverNote}
          </label>
        </Row>
        <Row head={t.clickTopic}>
          <label>
            <input
              type="radio"
              name="click_topic"
              checked={s.click_topic === "firstpage"}
              onChange={() => patch({ click_topic: "firstpage" })}
            />
            {t.firstPage}
          </label>
          <label>
            <input
              type="radio"
              name="click_topic"
              checked={s.click_topic === "lastpage"}
              onChange={() => patch({ click_topic: "lastpage" })}
            />
            {t.lastPage}
          </label>
        </Row>
        <Row head={t.signature}>
          <textarea
            className="uc-textarea"
            rows={10}
            value={s.signature ?? ""}
            onChange={(e) => patch({ signature: e.target.value })}
          />
          <br />
          {t.signatureNote}
        </Row>
      </tbody>
    </table>
  );
}

// ============ 安全设定 ============

function SecurityTab({
  s,
  patch,
}: {
  s: UserSettings;
  patch: (p: Partial<UserSettings>) => void;
}) {
  const { dict } = useI18n();
  const t = dict.usercp.security;
  const [newPass, setNewPass] = useState("");
  const [newPass2, setNewPass2] = useState("");
  const [oldPass, setOldPass] = useState("");
  const [pwBusy, setPwBusy] = useState(false);
  const [pwMsg, setPwMsg] = useState<string | null>(null);
  const [pkBusy, setPkBusy] = useState(false);
  const [pkResult, setPkResult] = useState<string | null>(null);
  const [pkMsg, setPkMsg] = useState<string | null>(null);

  async function changePasswordNow() {
    setPwMsg(null);
    if (newPass.length < 8) {
      setPwMsg(dict.usercp.security.pwTooShort);
      return;
    }
    if (newPass !== newPass2) {
      setPwMsg(dict.usercp.security.pwMismatch);
      return;
    }
    setPwBusy(true);
    try {
      await api.post("/api/v1/me/password/change", {
        old_password: oldPass,
        new_password: newPass,
      });
      setPwMsg(dict.usercp.security.pwChanged);
      setOldPass("");
      setNewPass("");
      setNewPass2("");
    } catch (e) {
      setPwMsg(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : dict.common.networkError);
    } finally {
      setPwBusy(false);
    }
  }

  async function rotatePasskeyNow() {
    if (!window.confirm(dict.my.passkeyConfirm)) return;
    setPkBusy(true);
    setPkMsg(null);
    try {
      const r = await api.post<{ passkey: string }>("/api/v1/me/passkey/rotate", {});
      setPkResult(r.passkey);
      setPkMsg(dict.my.passkeyRotated);
    } catch (e) {
      setPkMsg(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : dict.common.networkError);
    } finally {
      setPkBusy(false);
    }
  }

  return (
    <table className="nexus-table nexus-form">
      <tbody>
        <Row head={t.resetPasskey}>
          <button
            type="button"
            disabled={pkBusy}
            onClick={rotatePasskeyNow}
            className="min-h-[36px] rounded-full bg-coral px-4 text-xs font-bold text-white disabled:opacity-50"
          >
            {pkBusy ? dict.my.passkeyRotating : dict.my.passkeyRotate}
          </button>
          {pkResult && (
            <>
              <br />
              <code className="num break-all text-xs">{pkResult}</code>
            </>
          )}
          {pkMsg && <p className="text-xs text-sub">{pkMsg}</p>}
          <br />
          <span className="uc-note">
            <b>{dict.usercp.note}</b>
            {t.resetPasskeyNote}
          </span>
        </Row>
        <Row head={t.twoStep}>
          <TwoFactorSetup />
          <br />
          {t.twoStepHint}
        </Row>
        <tr>
          <td className="rowhead">{t.pushNotify}</td>
          <td className="rowfollow p-0">
            <PushSettings />
          </td>
        </tr>
        {/* 通知偏好（0075）：站内通知事件类开关 */}
        <tr>
          <td className="rowhead nowrap">{dict.noticePrefs.title}</td>
          <td className="rowfollow p-0">
            <NoticePrefsCard />
          </td>
        </tr>
        <Row head={t.passkeyLabel}>
          {t.passkeyHint}
        </Row>
        <Row head={t.tgBind}>
          {t.tgBindHint}
        </Row>
        <tr>
          <td className="rowhead">{dict.apitokens.title}</td>
          <td className="rowfollow p-0">
            <ApiTokens />
          </td>
        </tr>
        <tr>
          <td className="rowhead nowrap">{t.oldPassword}</td>
          <td className="rowfollow">
            <input
              type="password"
              className="uc-password"
              autoComplete="current-password"
              value={oldPass}
              onChange={(e) => setOldPass(e.target.value)}
            />
          </td>
        </tr>
        <tr>
          <td className="rowhead nowrap">{t.newPassword}</td>
          <td className="rowfollow">
            <input
              type="password"
              className="uc-password"
              autoComplete="new-password"
              value={newPass}
              onChange={(e) => setNewPass(e.target.value)}
            />
          </td>
        </tr>
        <tr>
          <td className="rowhead nowrap">{t.confirmPassword}</td>
          <td className="rowfollow">
            <input
              type="password"
              className="uc-password"
              autoComplete="new-password"
              value={newPass2}
              onChange={(e) => setNewPass2(e.target.value)}
            />
            <br />
            <button
              type="button"
              disabled={pwBusy || !oldPass || !newPass || !newPass2}
              onClick={changePasswordNow}
              className="mt-2 min-h-[36px] rounded-full bg-sky-deep px-4 text-xs font-bold text-white disabled:opacity-50"
            >
              {pwBusy ? "…" : t.changeBtn}
            </button>
            {pwMsg && <p className="mt-1 text-xs text-sub">{pwMsg}</p>}
          </td>
        </tr>
        <Row head={t.privacy}>
          <label>
            <input
              type="radio"
              name="privacy"
              checked={s.privacy === "normal"}
              onChange={() => patch({ privacy: "normal" })}
            />{" "}
            {t.privacyNormal}
          </label>
          <label>
            <input
              type="radio"
              name="privacy"
              checked={s.privacy === "low"}
              onChange={() => patch({ privacy: "low" })}
            />{" "}
            {t.privacyLow}
          </label>
          <label>
            <input
              type="radio"
              name="privacy"
              checked={s.privacy === "strong"}
              onChange={() => patch({ privacy: "strong" })}
            />{" "}
            {t.privacyStrong}
          </label>
        </Row>
      </tbody>
    </table>
  );
}
