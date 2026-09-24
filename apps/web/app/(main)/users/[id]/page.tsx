import { notFound } from "next/navigation";
import Link from "next/link";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { ProfileHero, StatTiles } from "./_parts/profile-hero";
import { CenterGrid } from "./_parts/profile-center";
import { CommentsTab, PostsTab, TorrentListTable } from "./_parts/profile-tabs";
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
  searchParams: Promise<{ tab?: string }>;
}) {
  const { id } = await params;
  const { tab } = await searchParams;
  const uid = Number(id);
  if (!Number.isFinite(uid)) notFound();
  const active: Tab = (TABS as readonly string[]).includes(tab ?? "")
    ? (tab as Tab)
    : "center";
  // 种子列表类 tab 才拉 torrentlist（NP userdetails Torrent History 口径）
  const listTabs: Tab[] = [
    "uploads",
    "seeding",
    "leeching",
    "completed",
    "incomplete",
    "preserved",
  ];

  let data: ProfileData;
  try {
    data = await api.get<ProfileData>(
      `/api/v1/users/${encodeURIComponent(uid)}`,
    );
  } catch {
    notFound();
  }
  let torrents: TorrentLists | null = null;
  if (listTabs.includes(active)) {
    torrents = await api
      .get<TorrentLists>(
        `/api/v1/users/${encodeURIComponent(uid)}/torrentlist?limit=100`,
      )
      .catch(() => null);
  }

  const { dict, locale } = await getDict();
  const t = dict.userProfile2;
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
  // 首屏核心指标磁贴（5 个：上传/下载/分享率/魔力/本月做种收益）
  const statTiles: {
    key: string;
    icon: string;
    mod?: string;
    label: string;
    value: string;
  }[] = [
    {
      key: "up",
      icon: "↑",
      mod: "up",
      label: t.uploaded,
      value: gb(p.uploaded),
    },
    {
      key: "down",
      icon: "↓",
      mod: "down",
      label: t.downloaded,
      value: gb(p.downloaded),
    },
    { key: "ratio", icon: "≈", label: t.ratio, value: ratio },
    {
      key: "spark",
      icon: "✦",
      mod: "spark",
      label: t.sparkField,
      value: Number(data.spark_balance).toLocaleString("en-US"),
    },
    {
      key: "earn",
      icon: "★",
      label: t.seedEarnField,
      value: Number(data.month_seed_earn).toLocaleString("en-US"),
    },
  ];
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

  return (
    <div className="flex flex-col gap-4">
      {/* Hero 身份卡：头像 + 用户名/勋章 + 等级头衔 + 在线 + 注册/最近活动（所有 tab 共用） */}
      <ProfileHero
        data={data}
        locale={locale}
        t={t as unknown as Record<string, string>}
      />

      {/* 核心指标磁贴（首屏只给 5 个关键数，明细下放到卡片） */}
      <StatTiles statTiles={statTiles} />

      {/* 标签页导航（憨憨式：链接切换 ?tab=，SSR 渲染对应内容，无客户端状态） */}
      <div className="flex flex-wrap items-center gap-1.5">
        {tabMeta.map((m) => (
          <Link
            key={m.key}
            href={
              m.key === "center"
                ? `/users/${uid}`
                : `/users/${uid}?tab=${m.key}`
            }
            className={`rounded-full border px-3 py-1 text-xs font-bold transition-colors ${
              active === m.key
                ? "border-sky bg-sky text-white"
                : "border-line bg-[var(--surface-card)] text-sub hover:text-ink"
            }`}
          >
            {/* 计数徽标拼进单字符串（水合安全） */}
            {`${m.label}${typeof m.count === "number" ? ` (${m.count})` : ""}`}
          </Link>
        ))}
      </div>

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
        />
      )}

      {active === "uploads" && (
        <TorrentListTable
          rows={torrents?.uploads ?? []}
          emptyText={t.noUploads}
          t={t as unknown as Record<string, string>}
          gb={gb}
          publicOnly
        />
      )}
      {active === "seeding" && (
        <TorrentListTable
          rows={torrents?.seeding ?? []}
          emptyText={t.noUploads}
          t={t as unknown as Record<string, string>}
          gb={gb}
        />
      )}
      {active === "leeching" && (
        <TorrentListTable
          rows={torrents?.leeching ?? []}
          emptyText={t.noLeeching}
          t={t as unknown as Record<string, string>}
          gb={gb}
        />
      )}
      {active === "completed" && (
        <TorrentListTable
          rows={torrents?.completed ?? []}
          emptyText={t.noCompleted}
          t={t as unknown as Record<string, string>}
          gb={gb}
        />
      )}
      {active === "incomplete" && (
        <TorrentListTable
          rows={torrents?.incomplete ?? []}
          emptyText={t.noIncomplete}
          t={t as unknown as Record<string, string>}
          gb={gb}
        />
      )}
      {active === "preserved" && (
        <TorrentListTable
          rows={torrents?.preserved ?? []}
          emptyText={t.noPreserved}
          t={t as unknown as Record<string, string>}
          gb={gb}
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
    </div>
  );
}
