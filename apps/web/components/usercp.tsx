"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { api, setSessionCookie } from "@/lib/api-client";
import type { NextClass } from "@/lib/usercp-level";
import { USERCP_NAV, type UsercpTab } from "@/lib/usercp-nav";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { LOCALE_COOKIE, siteLangToLocale, type Locale } from "@/i18n/config";
import { OverviewTab } from "@/components/usercp-overview";
import dynamic from "next/dynamic";
import { SkeletonRows } from "@/components/ui/skeleton";
import { Icon } from "@/components/icons";

/* 六个标签页内容件按需分片（2026-11-11 提速批）：/my 的 page chunk 曾
 *  75KB raw——六个标签件全量 eager 进首屏，而用户一次只看一个标签。
 *  overview 是默认落地页保留 eager；其余 dynamic，切标签瞬间才拉对应
 *  小 chunk（骨架行高与表单行对齐防跳）。 */
const dync = (f: () => Promise<unknown>) =>
  dynamic(f as never, {
    ssr: false,
    loading: () => <SkeletonRows rows={6} />,
  }) as unknown as React.ComponentType<Record<string, unknown>>;
const WishlistPanel = dync(
  () => import("@/components/wishlist").then((m) => m.WishlistPanel) as never);
/* 标签页按需分片（2026-11-11 提速批）：/my 的 page chunk 曾 75KB——六
 *  个标签件全量 eager 而用户一次只看一个。overview 保留 eager，其余
 *  dynamic，切标签瞬间拉小 chunk（骨架行高与表单行对齐防跳）。 */
const PersonalTab = dync(
  () =>
    import("@/components/usercp-personal").then((m) => m.PersonalTab) as never);
const UserFieldsTab = dynamic(
  () => import("@/components/usercp-fields").then((m) => m.UserFieldsTab),
  { ssr: false, loading: () => <SkeletonRows rows={6} /> });
const TrackerTab = dynamic(
  () => import("@/components/usercp-tracker").then((m) => m.TrackerTab),
  { ssr: false, loading: () => <SkeletonRows rows={6} /> });
const ForumTab = dynamic(
  () => import("@/components/usercp-forum").then((m) => m.ForumTab),
  { ssr: false, loading: () => <SkeletonRows rows={4} /> });
const SecurityTab = dynamic(
  () => import("@/components/usercp-security").then((m) => m.SecurityTab),
  { ssr: false, loading: () => <SkeletonRows rows={6} /> });

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

/** 安全分区自取数所需的最小回落对象：只有 privacy 一处在 SecurityTab
 *  里读（其余字段由各子组件自己请求），故不需要伪造整份站点设定。 */
const SECURITY_FALLBACK = { privacy: "normal" } as UserSettings;

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
  passkey_masked: string;
  login_trend_30d: { date: string; count: number }[];
  login_days_30d: number;
  login_total_30d: number;
  last_login: string | null;
  /** 最近一次成功登录的 IP / 城市；后端没记到就是 null */
  last_ip?: string | null;
  last_city?: string | null;
  next_class: NextClass | null;
  /** GET /me/overview 的最近阅读主题（样图 § ucp 概览那张表） */
  recent_topics?: {
    topic_id: number; title: string; forum_id: number;
    forum_name: string; replies: number; read_at: string;
  }[];
}

// ============ 主组件 ============
/** 控制面板（NP 口径）：侧栏导航 + 七档内容件（标签页懒加载见 dync） */
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

  /** 站点设定取数（可重试）。
   *
   *  P1-7：取数失败曾把整份控制面板（**含安全分区**）变成一块死区——
   *  用户连改密码都做不了，而且没有重试按钮；错误文案还是后端原文。
   *  现在取数独立成函数：错误区给「重试」，文案统一走 apiErrorMessage，
   *  安全分区不再依赖这份设定（自己取数）。 */
  const loadSettings = useCallback(async () => {
    setErr(null);
    try {
      const s = await api.get<UserSettings>("/api/v1/me/settings");
      setSettings(s);
      // 反向一致：库里语言与当前 locale 不一致（如别的设备改过）→ 以库为准同步 cookie
      const want = siteLangToLocale(s.site_language);
      if (want && want !== locale) {
        syncLocaleCookie(want);
        router.refresh();
      }
    } catch (e) {
      setErr(apiErrorMessage(dict, e));
    }
  }, [dict, locale, router]);

  useEffect(() => {
    void loadSettings();
    if (tab === "overview") {
      api
        .get<Overview>("/api/v1/me/overview")
        .then(setOverview)
        .catch(() => setOverview(null));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab]);

  const patch = (p: Partial<UserSettings>) => {
    // 安全分区能在 settings 取数失败时独立工作：以最小回落对象为底，
    // 这样隐私等级的切换不会因为站点设定没取到而丢失。
    setSettings((prev) => ({ ...(prev ?? SECURITY_FALLBACK), ...p }));
    setSaved(false);
  };

  /** 退出登录（2026-10-17 对齐样图集 § ucp-side 末尾的红字项）。
   *  与 user-menu 的登出同口径：后端撤销失败也照常清本地凭证，并让 SW
   *  丢掉含登录态 RSC 快照的页面缓存（防共用设备离线回看）。 */
  async function logout() {
    await api.post("/api/v1/auth/logout", {}).catch(() => {});
    localStorage.removeItem("flux.user");
    navigator.serviceWorker?.controller?.postMessage("CLEAR_PAGES");
    setSessionCookie(false);
    location.href = "/login";
  }

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
      setErr(apiErrorMessage(dict, e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="usercp-wrap">
      {/* 侧边导航（对齐样图集 § ucp-side）：样图的侧栏**没有分组标题**，
          直接从「账户概览」开始，末尾以红字「退出登录」收尾。原先的
          ⚙「控制面板」橙色渐变小标题既是装饰也占一行，且容易被误读成
          当前选中项，按样图去掉。 */}
      <aside className="usercp-nav">
        {(nav ?? USERCP_NAV).map((n) => (
          <Link
            key={n.key}
            href={
              n.key === "overview" ? "/my?tab=overview" : `/my?tab=${n.key}`
            }
            data-active={tab === n.key ? "true" : undefined}
            onClick={(e) => {
              e.preventDefault();
              setTab(n.key);
              // P2-11：改用客户端路由推进。旧写法 replaceState 会抹掉浏览器
              // 历史（后退回不到上一个 tab）并把 hash 锚点冲掉。
              const hash =
                n.key === "security" && typeof window !== "undefined"
                  ? window.location.hash
                  : "";
              router.push(`/my?tab=${n.key}${hash}`, { scroll: false });
            }}
          >
            <Icon name={n.icon} size={17} className="shrink-0" />
            {t.tabs[n.key]}
          </Link>
        ))}
        <button type="button" className="usercp-nav__logout" onClick={logout}>
          <Icon name="logout" size={17} className="shrink-0" />
          {dict.usercp.logout}
        </button>
      </aside>

      <div className="usercp-content">
        {err && (
          // P1-7：取数失败给出「重试」，且不再把整份面板（含安全分区）锁死
          <div
            role="alert"
            className="usercp-error flex flex-wrap items-center gap-3"
          >
            <span>{err}</span>
            <button
              type="button"
              className="brand-button"
              onClick={() => void loadSettings()}
            >
              {dict.common.retry}
            </button>
          </div>
        )}
        {tab === "overview" ? (
          <OverviewTab ov={overview} loading={!overview && !err} />
        ) : tab === "wishlist" ? (
          <WishlistPanel />
        ) : tab === "security" ? (
          // P0-1：安全分区不进站点设定的 form。它在改密/2FA/Passkey 的
          // 输入框里按回车会隐式提交站点设定，底部弹「已保存」而密码根本没
          // 改。安全分区自取数、各自带提交按钮。
          <SecurityTab s={settings ?? SECURITY_FALLBACK} patch={patch} />
        ) : tab === "fields" ? (
          // 自定义字段走自己的保存口（PUT /me/fields），不共用站点设定表单
          <UserFieldsTab />
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
            {/* P0-1：底部保存行只服务站点设定三档（个人/追踪/论坛）。
                字段分区与安全分区各有自己的保存口，不再共用这一行。 */}
            <div className="brand-wide-table-scroll">
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
            </div>
          </form>
        ) : (
          !err && <p className="usercp-loading">{dict.my.loading}</p>
        )}
      </div>
    </div>
  );
}
