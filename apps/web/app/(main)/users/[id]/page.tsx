/* 用户主页窄屏样式（路由级加载，不回灌 pages.css） */
import "@/app/styles/users-mobile.css";
import { notFound } from "next/navigation";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";
import { petLineOf, PET_SPECIES_KEY } from "@/lib/pet-lines";
import type { PetSpecies } from "@/lib/pet-lines";
import { ProfileHero, StatTiles } from "./_parts/profile-hero";
import { CenterGrid } from "./_parts/profile-center";
import { CommentsTab, PostsTab, TorrentListTable } from "./_parts/profile-tabs";
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
  const tt = t as unknown as Record<string, string>;
  // 宠物展示位（只读快照；没养过 / games 模块关闭 → 接口失败，不渲染）
  const pet = await api
    .get<{
      exists: boolean;
      species?: string;
      name?: string;
      level?: number;
    }>(`/api/v1/games/pet/of?user_id=${encodeURIComponent(uid)}`)
    .catch(() => null);
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
      value: Number(data.spark_balance).toLocaleString(dateLocale(locale)),
    },
    {
      key: "earn",
      icon: "★",
      label: t.seedEarnField,
      value: Number(data.month_seed_earn).toLocaleString(dateLocale(locale)),
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

      {/* 核心指标磁贴（首屏只给 5 个关键数，明细下放到卡片） */}
      <StatTiles statTiles={statTiles} />

      {/* 宠物展示位：养成成果社交化（种类只改外观，零数值差） */}
      {pet?.exists && (
        <section className="up-pet">
          <span className="up-pet-face" aria-hidden>
            {
              petLineOf(pet.species ?? "slime")[
                Math.min((pet.level ?? 1) - 1, 9)
              ]
            }
          </span>
          <div className="up-pet-meta">
            <b>{pet.name || dict.games.cards.pet.title}</b>
            <span className="text-xs text-sub">
              {`Lv.${pet.level ?? 1} · ${
                (dict.games.pet as Record<string, string>)[
                  PET_SPECIES_KEY[(pet.species ?? "slime") as PetSpecies] ??
                    "spSlime"
                ]
              }`}
            </span>
          </div>
          <Link
            href="/games/pet"
            className="text-xs font-bold text-[var(--sky-deep)]"
          >
            {dict.games.cards.pet.title} →
          </Link>
        </section>
      )}

      {/* 标签页导航（憨憨式：链接切换 ?tab=，SSR 渲染对应内容，无客户端状态） */}
      <div className="up-tabs flex flex-wrap items-center gap-1.5">
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
                ? "border-sky bg-sky text-[var(--text-on-accent)]"
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
      {active === "changes" && canSeeChanges && <ChangesSection uid={uid} />}
    </div>
  );
}

/** 变更记录（0482c）：SSR 拉取后交共享审计面板渲染（时间/动作/操作者/详情）；
 *  接口 403（会话过期等）时安静降级为空态，不炸整页。 */
async function ChangesSection({ uid }: { uid: number }) {
  let rows: AuditEntryRow[] = [];
  try {
    rows = await api.get<AuditEntryRow[]>(
      `/api/v1/users/${encodeURIComponent(uid)}/changes`,
    );
  } catch {
    rows = [];
  }
  return <UserAuditLogPanel audit={rows} />;
}
