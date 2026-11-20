/* 用户主页窄屏样式（路由级加载，不回灌 pages.css） */
import "@/app/styles/users-mobile.css";
/* B3 UX 专项样式（路由级：与 admin 布局各自 import） */
import "@/app/styles/ux-profile-admin.css";
import { notFound } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";
import { ProfileHero, StatTiles } from "./_parts/profile-hero";
import { CenterGrid } from "./_parts/profile-center";
import { ProfileStatsCard } from "./_parts/profile-stats-card";
import {
  CommentsTab,
  PostsTab,
  ProfileTabGroups,
  TorrentListTable,
} from "./_parts/profile-tabs";
import { UserAuditLogPanel } from "@/components/admin-user-audit-log";
import type { AuditEntryRow } from "@/components/admin-user-detail-shared";
import {
  LIST_TABS,
  LIST_VIEWS,
  TorrentListPager,
  fetchTorrentLists,
} from "./_parts/profile-list";
import {
  TABS,
  type ProfileData,
  type Tab,
  type TorrentLists,
} from "./_parts/profile-types";

export const dynamic = "force-dynamic";

/** 用户公开主页：Hero 身份卡 + 核心指标磁贴 + 憨憨式标签页
 *  （个人中心为双列卡片网格；种子/帖子/评论各 tab 沿用原表格）
 *  Hero/磁贴拆至 _parts/profile-hero.tsx；个人中心网格拆至 _parts/profile-center.tsx；
 *  tab 内容拆至 _parts/profile-tabs.tsx；类型拆至 _parts/profile-types.ts。 */
export default async function UserProfilePage({
  params,
  searchParams,
}: {
  params: Promise<{ id: string }>;
  searchParams: Promise<{ tab?: string; tl_page?: string }>;
}) {
  const { id } = await params;
  const { tab, tl_page: tlPageQ } = await searchParams;
  const uid = Number(id);
  if (!Number.isFinite(uid)) notFound();
  const active: Tab = (TABS as readonly string[]).includes(tab ?? "")
    ? (tab as Tab)
    : "center";
  // 当前 tab 对应的种子列表视图（center/posts/comments 为 null）
  const listView = LIST_VIEWS.find((v) => v.key === active);
  let data: ProfileData;
  try {
    data = await api.get<ProfileData>(
      `/api/v1/users/${encodeURIComponent(uid)}`,
    );
  } catch (e) {
    if (e instanceof ApiError && e.status === 404) notFound();
    else throw e;
  }
  // 种子列表翻页（2026-11-02）：此前一次 limit=100，窄屏那页要滚很久；
  // 现在每页 20 条 + 翻页条。多取一条用来判「还有没有下一批」。
  const tlPage = Math.max(1, Number(tlPageQ ?? 1) || 1);
  let torrents: TorrentLists | null = null;
  let tlHasMore = false;
  if (LIST_TABS.includes(active)) {
    const r = await fetchTorrentLists(uid, tlPage);
    torrents = r.torrents;
    tlHasMore = listView ? (r.hasMore[listView.key] ?? false) : false;
  }

  const { dict, locale } = await getDict();
  const t = dict.userProfile2;
  // 词表按键取值（空态文案由 LIST_VIEWS 指定键名）
  const tt = {
    ...t,
    // 第二轮评审补的 noSeeding 键在 review 段（userProfile2 无此键）
    noSeeding: dict.review.noSeeding,
  } as unknown as Record<string, string>;
  // B3-4：宠物空挂件（「宠物 Lv.1」占位）已移除。ProfileData 无
  // modules.pet 开关，按任务约定「没有开关就直接隐藏该空块」——不额外请求。
  const p = data.profile;
  const ratio = p.downloaded > 0 ? (p.uploaded / p.downloaded).toFixed(2) : "∞";
  const realRatio =
    data.real_downloaded > 0
      ? (data.real_uploaded / data.real_downloaded).toFixed(2)
      : "∞";
  const gb = (n: number) => `${(n / 1024 ** 3).toFixed(2)} GB`;
  // 做种时长（NP 口径：X天 HH:MM:SS）
  const seedDur = (s: number) => {
    if (s <= 0) return "0";
    const d = Math.floor(s / 86400);
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${d}${t.days} ${pad(Math.floor((s % 86400) / 3600))}:${pad(
      Math.floor((s % 3600) / 60),
    )}:${pad(s % 60)}`;
  };
  // 等级进度条（四项条件各一条；达标显示满格）
  const prog = (cur: number, need: number, fmt: (n: number) => string) => {
    const pct = need <= 0 ? 100 : Math.min(100, Math.round((cur / need) * 100));
    return { pct, text: `${fmt(cur)} / ${fmt(need)}` };
  };
  // B3-2：健康度优先——四张带语义状态 chip 的卡；上传总量降为次级信息。
  // 判定口径全部取本页已有 overview 数据，不新增请求：
  // 分享率≥1 安全 / H&R 待办=0 安全 / 在做种>0 安全 / 本月有做种收益=魔力安全。
  const ux = dict.uxOpt;
  const ratioNum = p.downloaded > 0 ? p.uploaded / p.downloaded : Infinity;
  const healthTiles = [
    {
      key: "ratio",
      icon: "ratio",
      label: ux.statRatio,
      value: ratio,
      status: ratioNum >= 1 ? "safe" : "watch",
    },
    {
      key: "hr",
      icon: "warning",
      label: ux.statHr,
      value: `${data.hr_unresolved} / ${data.hr_limit}`,
      status: data.hr_unresolved === 0 ? "safe" : "watch",
    },
    {
      key: "seeding",
      icon: "seed",
      label: ux.statSeeding,
      value: String(p.seeding),
      status: p.seeding > 0 ? "safe" : "watch",
    },
    {
      key: "magic",
      icon: "spark",
      label: ux.statMagic,
      value: Number(data.spark_balance).toLocaleString(dateLocale(locale)),
      status: data.month_seed_earn > 0 ? "safe" : "watch",
    },
  ] as const;
  // 标签页文案 + 计数徽标
  const tabMeta: { key: Tab; label: string; count?: number }[] = [
    { key: "center", label: t.tabCenter },
    { key: "uploads", label: t.tabUploads, count: torrents?.uploads.length },
    { key: "seeding", label: t.tabSeeding, count: p.seeding },
    { key: "leeching", label: t.tabLeeching, count: p.leeching },
    { key: "completed", label: t.tabCompleted, count: data.completed_snatches },
    { key: "incomplete", label: t.tabIncomplete },
    { key: "preserved", label: t.tabPreserved },
    { key: "posts", label: t.tabPosts, count: data.recent_posts.length },
    { key: "comments", label: t.tabComments, count: p.comments },
  ];
  // 变更记录（0482c）：仅本人或职员可见（接口同权限，这里只做显隐）
  const canSeeChanges = !!(data.viewer?.self || data.viewer?.staff);
  if (canSeeChanges) {
    tabMeta.push({ key: "changes", label: dict.userDetail.tabAudit });
  }

  return (
    <div className="flex flex-col gap-4">
      {/* Hero 身份卡：头像 + 用户名/勋章 + 等级头衔 + 在线 + 注册/最近活动（所有 tab 共用） */}
      <ProfileHero
        data={data}
        locale={locale}
        t={t as unknown as Record<string, string>}
      />

      {/* B3-2：健康度四卡 + 上传总量次级信息 */}
      <StatTiles
        tiles={healthTiles.map((tile) => ({
          ...tile,
          statusLabel: tile.status === "safe" ? ux.stateSafe : ux.stateWatch,
        }))}
        heading={ux.profileHealth}
        secondary={`${t.uploaded} · ${gb(p.uploaded)}`}
      />


      {/* B3-6：9 个页签按四组收纳；全部链接仍可达，默认展示中心（概览） */}
      <ProfileTabGroups
        tabMeta={tabMeta}
        active={active}
        uid={uid}
        groupLabels={[
          dict.usercp.tabs.overview,
          t.torrentHistory,
          t.tabPosts,
          ux.profileProgress,
        ]}
        ariaLabel={t.title}
      />

      {active === "center" && (
        <CenterGrid
          data={data}
          locale={locale}
          t={t as unknown as Record<string, string>}
          ratio={ratio}
          realRatio={realRatio}
          gb={gb}
          seedDur={seedDur}
          prog={prog}
          seedLabel={dict.uxOpt.statSeeding}
          isSelf={!!data.viewer?.self}
        />
      )}

      {/* 0503 他人主页可视化（10-26 B 画像遗留）：做种画像卡，
          服务端渲染；无做种自隐 */}
      {active === "center" && (
        <ProfileStatsCard uid={data.profile.id} dict={dict} />
      )}

      {listView && (
        <TorrentListTable
          rows={torrents?.[listView.key] ?? []}
          emptyText={tt[listView.empty]}
          t={tt}
          gb={gb}
          locale={locale}
          withMeta={listView.withMeta}
          publicOnly={listView.publicOnly}
        />
      )}

      {/* 种子列表翻页条（窄屏那页一次 20 条，底部翻页） */}
      {LIST_TABS.includes(active) && (
        <TorrentListPager
          uid={uid}
          tab={active}
          page={tlPage}
          hasMore={tlHasMore}
          prevText={dict.common.prevPage}
          nextText={dict.common.nextPage}
          pageText={dict.common.pageX}
        />
      )}

      {active === "posts" && (
        <PostsTab
          data={data}
          locale={locale}
          t={t as unknown as Record<string, string>}
        />
      )}
      {active === "comments" && (
        <CommentsTab
          data={data}
          locale={locale}
          t={t as unknown as Record<string, string>}
        />
      )}

      {/* 变更记录（0482c）：本人/职员可见；SSR 直拉审计回查，失败静默隐卡 */}
      {active === "changes" && canSeeChanges && <ChangesSection uid={uid} dict={dict} />}
    </div>
  );
}

/** 变更记录（0482c）：SSR 拉取后交共享审计面板渲染（时间/动作/操作者/详情）；
 *  接口 403（会话过期等）时安静降级为空态，不炸整页。 */
async function ChangesSection({
  uid,
  dict,
}: {
  uid: number;
  dict: { review: { sessionExpired: string } };
}) {
  let rows: AuditEntryRow[] = [];
  try {
    rows = await api.get<AuditEntryRow[]>(
      `/api/v1/users/${encodeURIComponent(uid)}/changes`,
    );
  } catch (e) {
    // 403（会话过期）不该静默成「无记录」——给出重登提示
    const status = (e as { status?: number })?.status;
    if (status === 401 || status === 403) {
      return (
        <p className="py-4 text-sm text-sub">
          <a href={"/login?next=" + encodeURIComponent("/users/" + uid)} className="text-link underline">
            {dict.review.sessionExpired}
          </a>
        </p>
      );
    }
    rows = [];
  }
  return <UserAuditLogPanel audit={rows} />;
}
