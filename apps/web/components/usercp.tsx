"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { LOCALE_COOKIE, siteLangToLocale, type Locale } from "@/i18n/config";
import { WishlistPanel } from "@/components/wishlist";
import { OverviewTab } from "@/components/usercp-overview";
import { PersonalTab } from "@/components/usercp-personal";
import { UserFieldsTab } from "@/components/usercp-fields";
import { TrackerTab } from "@/components/usercp-tracker";
import { ForumTab } from "@/components/usercp-forum";
import { SecurityTab } from "@/components/usercp-security";

/** 控制面板 —— 像素级复刻 NexusPHP usercp（参考站）：
 *  左侧 ⚙控制面板 六项侧边导航（账户概览/个人资料/网站设定/论坛设定/安全设定）
 *  + 各 tab 的 rowhead/rowfollow 经典表格表单，字段与旧站一一对应。
 *  各 tab 面板按域拆出（300 行门禁）：usercp-overview.tsx（账户概览）、
 *  usercp-personal.tsx（个人资料）、usercp-tracker.tsx（网站设定，含
 *  种子列表浏览参数）、usercp-forum.tsx（论坛设定）、usercp-security.tsx
 *  （安全设定）；共用表格行在 usercp-row.tsx。 */

export type UsercpTab =
  "overview" | "personal" | "fields" | "tracker" | "forum" | "security" | "wishlist";

export const USERCP_NAV: { key: UsercpTab; icon: string }[] = [
  { key: "overview", icon: "⌂" },
  { key: "personal", icon: "♙" },
  { key: "fields", icon: "✎" },
  { key: "tracker", icon: "⚙" },
  { key: "forum", icon: "♧" },
  { key: "security", icon: "⌾" },
  { key: "wishlist", icon: "☆" },
];

// ============ 站点语言 ⇄ 前端 locale 映射 ============
// 换算表已收敛到 i18n/config.siteLangToLocale（与后端 getLocale 默认语言回落共用）；
// 此处只保留「保存后同步 cookie 并刷新」的副作用。

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

export interface Overview {
  id: number;
  username: string;
  email: string;
  class_name: string | null;
  avatar_url: string | null;
  avatar_frame_css?: string | null;
  avatar_frame_image?: string | null;
  worn_medals?: { name: string; asset_ref?: string | null }[];
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

// ============ 主组件 ============

export function UsercpPanel({
  initialTab,
  nav,
}: {
  initialTab: UsercpTab;
  /** 页面级过滤后的侧边栏（0209：心愿单关模块时页面已滤，侧边栏须同滤） */
  nav?: { key: UsercpTab; icon: string }[];
}) {
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
        const want = siteLangToLocale(s.site_language);
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
      const want = siteLangToLocale(settings.site_language);
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
        {(nav ?? USERCP_NAV).map((n) => (
          <Link
            key={n.key}
            href={
              n.key === "overview" ? "/my?tab=overview" : `/my?tab=${n.key}`
            }
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
            {tab === "fields" && <UserFieldsTab />}
            {tab === "tracker" && <TrackerTab s={settings} patch={patch} />}
            {tab === "forum" && <ForumTab s={settings} patch={patch} />}
            {tab === "security" && <SecurityTab s={settings} patch={patch} />}
            <table className="nexus-table nexus-form">
              <tbody>
                <tr>
                  <td className="rowhead">{t.saveRow}</td>
                  <td className="rowfollow">
                    <input type="submit" value={t.saveBtn} disabled={saving} />
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
