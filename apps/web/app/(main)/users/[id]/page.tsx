import type { ReactNode } from "react";
import { notFound } from "next/navigation";
import Link from "next/link";
import { api } from "@/lib/api-client";
import { avatarFrameStyle, FrameImageOverlay } from "@/lib/format";
import { FollowButton } from "@/components/forum-follow";
import { MedalIcon } from "@/components/medal-icon";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";

export const dynamic = "force-dynamic";

interface Profile {
  id: number;
  username: string;
  title: string | null;
  avatar_url: string | null;
  class_id: number;
  class_name: string | null;
  uploaded: number;
  downloaded: number;
  donor: boolean;
  created_at: string;
  last_seen_at: string | null;
  seeding: number;
  leeching: number;
  uploads: number;
  comments: number;
  medals: number;
}

interface RecentUpload {
  id: number;
  name: string;
  small_descr: string | null;
  size: number;
  created_at: string;
}

interface RecentComment {
  torrent_id: number;
  body: string;
  created_at: string;
}

interface TorrentHistRow {
  torrent_id: number;
  name: string;
  size: number;
  seeders: number;
  leechers: number;
  seeding: boolean;
}

interface ProfileData {
  profile: Profile;
  gender: string | null;
  country: string | null;
  isp: string | null;
  upload_speed: number | null;
  download_speed: number | null;
  info: string | null;
  signature: string | null;
  online: boolean;
  real_uploaded: number;
  real_downloaded: number;
  seed_seconds: number;
  hr_unresolved: number;
  hr_limit: number;
  seeding_size: number;
  spark_balance: number;
  month_seed_earn: number;
  completed_snatches: number;
  invites_pending: number;
  inviter_name: string | null;
  inviter_id: number | null;
  client_agent: string | null;
  worn_medals: { id: number; name: string; description: string | null; asset_ref: string | null }[];
  avatar_frame_css: string | null;
  avatar_frame_image: string | null;
  achievements: number;
  next_class: {
    class_id: number;
    name: string;
    uploaded: number;
    uploaded_need: number;
    download_count: number;
    download_count_need: number;
    seed_hours: number;
    seed_hours_need: number;
    account_age_days: number;
    account_age_days_need: number;
  } | null;
  recent_posts: [number, number, string, string][];
  recent_uploads: RecentUpload[];
  recent_comments: RecentComment[];
}

type TorrentLists = {
  uploads: TorrentHistRow[];
  seeding: TorrentHistRow[];
  leeching: TorrentHistRow[];
  completed: TorrentHistRow[];
  incomplete: TorrentHistRow[];
  preserved: TorrentHistRow[];
};

/** 憨憨式标签页：个人中心（默认）+ 发布种子/当前做种/当前下载/完成/未完成/保种区 + 论坛动态/评论 */
const TABS = [
  "center",
  "uploads",
  "seeding",
  "leeching",
  "completed",
  "incomplete",
  "preserved",
  "posts",
  "comments",
] as const;
type Tab = (typeof TABS)[number];

/** 个人中心首屏的网格定席（见 globals.css .up-card--slot-*）：
 *  tl = 左列第一行，bl = 左列第二行，r2 = 右列横跨两行 */
const SLOT_CLASS = {
  tl: "up-card--slot-tl",
  bl: "up-card--slot-bl",
  r2: "up-card--slot-r2",
} as const;

/** 卡片：标题（图标 + 文案）+ 内容块；full 横跨整行，slot 指定首屏定席 */
function Card({
  title,
  icon,
  full,
  slot,
  children,
}: {
  title: string;
  icon?: string;
  full?: boolean;
  slot?: keyof typeof SLOT_CLASS;
  children: ReactNode;
}) {
  const cls = ["up-card", full ? "up-card--full" : "", slot ? SLOT_CLASS[slot] : ""]
    .filter(Boolean)
    .join(" ");
  return (
    <section className={cls}>
      <h2 className="up-card__title">
        {icon ? <i aria-hidden>{icon}</i> : null}
        <span>{title}</span>
      </h2>
      <div className="up-card__body">{children}</div>
    </section>
  );
}

/** 资料行：标签定宽 + 虚线分隔（替代原两列表格的一整块） */
function Row({ label, icon, children }: { label: string; icon?: string; children: ReactNode }) {
  return (
    <div className="up-row">
      <div className="up-row__label">
        {icon ? <i aria-hidden>{icon}</i> : null}
        <span>{label}</span>
      </div>
      <div className="up-row__value">{children}</div>
    </div>
  );
}

/** 用户公开主页：Hero 身份卡 + 核心指标磁贴 + 憨憨式标签页
 *  （个人中心为双列卡片网格；种子/帖子/评论各 tab 沿用原表格） */
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
  const active: Tab = (TABS as readonly string[]).includes(tab ?? "") ? (tab as Tab) : "center";
  // 种子列表类 tab 才拉 torrentlist（NP userdetails Torrent History 口径）
  const listTabs: Tab[] = ["uploads", "seeding", "leeching", "completed", "incomplete", "preserved"];

  let data: ProfileData;
  try {
    data = await api.get<ProfileData>(`/api/v1/users/${encodeURIComponent(uid)}`);
  } catch {
    notFound();
  }
  let torrents: TorrentLists | null = null;
  if (listTabs.includes(active)) {
    torrents = await api
      .get<TorrentLists>(`/api/v1/users/${encodeURIComponent(uid)}/torrentlist?limit=100`)
      .catch(() => null);
  }

  const { dict, locale } = await getDict();
  const t = dict.userProfile2 ?? {
    title: "用户资料",
    uploaded: "上传量",
    downloaded: "下载量",
    ratio: "分享率",
    seeding: "做种中",
    leeching: "下载中",
    uploads: "发布数",
    comments: "评论数",
    medals: "勋章数",
    joined: "注册时间",
    lastSeen: "最近活动",
    recentUploads: "近期发布",
    recentComments: "近期评论",
    torrentHistory: "种子历史",
    histUploads: "发布",
    histSeeding: "做种中",
    histPublicOnly: "仅公开展示",
    noUploads: "暂无公开种子",
    noComments: "暂无评论",
    donor: "捐赠者",
  };
  const p = data.profile;
  const ratio = p.downloaded > 0 ? (p.uploaded / p.downloaded).toFixed(2) : "∞";
  const realRatio =
    data.real_downloaded > 0 ? (data.real_uploaded / data.real_downloaded).toFixed(2) : "∞";
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
  const nc = data.next_class;
  // 首屏核心指标磁贴（5 个：上传/下载/分享率/魔力/本月做种收益）
  const statTiles: { key: string; icon: string; mod?: string; label: string; value: string }[] = [
    { key: "up", icon: "↑", mod: "up", label: t.uploaded, value: gb(p.uploaded) },
    { key: "down", icon: "↓", mod: "down", label: t.downloaded, value: gb(p.downloaded) },
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
  // 种子列表 tab 渲染（每 tab 复用同一张表：名称/大小 + S/L；uploads 标注仅公开）
  const renderList = (rows: TorrentHistRow[], emptyText: string, publicOnly = false) => (
    <div className="baozi-wide-table-scroll">
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">
              {`${t.torrentName}${publicOnly ? `（${t.histPublicOnly}）` : ""}`}
            </td>
            <td className="colhead w-28 text-right">Size</td>
            <td className="colhead w-20 text-right">S/L</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((u) => (
            <tr key={u.torrent_id}>
              <td className="max-w-[420px] truncate">
                <Link href={`/torrent/${u.torrent_id}`} className="text-sky-deep hover:underline">
                  {u.name}
                </Link>
              </td>
              <td className="num shrink-0 text-right text-sub">{gb(u.size)}</td>
              <td className="num shrink-0 text-right text-sub">
                {u.seeders} / {u.leechers}
              </td>
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td colSpan={3} className="py-6 text-center text-sub">
                {emptyText}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );

  return (
    <div className="flex flex-col gap-4">
      {/* Hero 身份卡：头像 + 用户名/勋章 + 等级头衔 + 在线 + 注册/最近活动（所有 tab 共用） */}
      <section className="up-hero">
        <div className="up-hero__avatar">
          {/* 头像 + 佩戴的头像框：image_url 立绘框图叠层优先，否则 CSS 描边 */}
          {p.avatar_url ? (
            // eslint-disable-next-line @next/next/no-img-element
            <span
              className="relative inline-flex h-full w-full"
              style={data.avatar_frame_image ? undefined : avatarFrameStyle(data.avatar_frame_css)}
            >
              <img src={p.avatar_url} alt={p.username} />
              <FrameImageOverlay url={data.avatar_frame_image} />
            </span>
          ) : (
            <span
              aria-hidden
              className="relative flex h-full w-full items-center justify-center text-4xl text-sub"
              style={data.avatar_frame_image ? undefined : avatarFrameStyle(data.avatar_frame_css)}
            >
              👤
              <FrameImageOverlay url={data.avatar_frame_image} />
            </span>
          )}
        </div>
        <div className="up-hero__main">
          <div className="up-hero__name">
            <h1 className="font-display text-2xl text-[var(--text-strong)]">{p.username}</h1>
            {data.worn_medals.slice(0, 3).map((m) => (
              <span key={m.id} className="medal-chip align-middle" title={m.name}>
                <MedalIcon src={m.asset_ref} size={14} title={m.name} />
              </span>
            ))}
            <span
              className={`rounded-full px-2 py-0.5 text-[11px] font-bold ${
                data.online ? "bg-[var(--mint-soft)] text-mint" : "bg-[var(--surface-sunken)] text-sub"
              }`}
              title={data.online ? t.online : t.offline}
            >
              {/* 单表达式拼好整段文本：SSR 与水合的文本节点划分一致，避免 418 */}
              {`● ${data.online ? t.online : t.offline}`}
            </span>
            {p.donor && <span className="sticker bg-sun text-ink">{`♥ ${t.donor}`}</span>}
          </div>
          {/* 等级/头衔 + 关注（0121）：FollowButton 自拉状态，is_self 时自行隐藏 */}
          <div className="mt-2 flex flex-wrap items-center gap-2 text-sm text-sub">
            <span className="up-hero__class">
              {`${p.class_name ?? `LV${p.class_id}`}${p.title ? ` · ${p.title}` : ""}`}
            </span>
            <FollowButton targetType="user" targetId={p.id} showCount />
          </div>
          <div className="up-hero__meta">
            <span>
              <i aria-hidden>◷</i>
              {`${t.joined}：${new Date(p.created_at).toLocaleDateString(dateLocale(locale))}`}
            </span>
            {p.last_seen_at && (
              <span>
                <i aria-hidden>◷</i>
                {`${t.lastSeen}：${new Date(p.last_seen_at).toLocaleString(dateLocale(locale))}`}
              </span>
            )}
          </div>
        </div>
      </section>

      {/* 核心指标磁贴（首屏只给 5 个关键数，明细下放到卡片） */}
      <section className="up-stats">
        {statTiles.map((s) => (
          <div key={s.key} className="up-stat">
            <span className={`up-stat__icon${s.mod ? ` up-stat__icon--${s.mod}` : ""}`} aria-hidden>
              {s.icon}
            </span>
            <div className="min-w-0">
              <div className="up-stat__value">{s.value}</div>
              <div className="up-stat__label">{s.label}</div>
            </div>
          </div>
        ))}
      </section>

      {/* 标签页导航（憨憨式：链接切换 ?tab=，SSR 渲染对应内容，无客户端状态） */}
      <div className="flex flex-wrap items-center gap-1.5">
        {tabMeta.map((m) => (
          <Link
            key={m.key}
            href={m.key === "center" ? `/users/${uid}` : `/users/${uid}?tab=${m.key}`}
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
        <div className="up-grid">
          {/* —— 账号信息（首屏左列第一行） —— */}
          <Card title={t.accountInfo} icon="◈" slot="tl">
            <Row label="UID" icon="#">
              <span className="num">{p.id}</span>
            </Row>
            {data.inviter_name && (
              <Row label={t.inviter} icon="◑">
                <Link href={`/users/${data.inviter_id}`}>{data.inviter_name}</Link>
              </Row>
            )}
            <Row label={t.invitesPending} icon="✦">
              <span className="num">{data.invites_pending}</span>
            </Row>
            {data.gender && (
              <Row label={t.gender} icon="◆">
                {data.gender}
              </Row>
            )}
            {data.country && (
              <Row label={t.country} icon="◍">
                {data.country}
              </Row>
            )}
            {data.isp && (
              <Row label={t.isp} icon="⇄">
                {data.isp}
              </Row>
            )}
            <Row label={t.joined} icon="◷">
              <span className="num">
                {new Date(p.created_at).toLocaleDateString(dateLocale(locale))}
              </span>
            </Row>
            {p.last_seen_at && (
              <Row label={t.lastSeen} icon="◷">
                <span className="num">
                  {new Date(p.last_seen_at).toLocaleString(dateLocale(locale))}
                </span>
              </Row>
            )}
            <Row label={t.medals} icon="✪">
              <span className="num">
                {`${p.medals}${data.achievements > 0 ? ` · ${t.achievements} ${data.achievements}` : ""}`}
              </span>
            </Row>
          </Card>

          {/* —— 下一级进度（首屏左列第二行：紧贴账号信息下方、传输与做种左侧；
                 四项条件各一条，达标满格；最高级显示达顶） —— */}
          <Card title={t.nextLevel} icon="▲" slot="bl">
            {nc ? (
              <>
                <p className="mb-2 text-xs font-bold text-sub">{`→ ${nc.name}`}</p>
                <div className="flex flex-col gap-2 pb-2">
                  {[
                    { label: t.upProgress, ...prog(nc.uploaded, nc.uploaded_need, (n) => gb(n)) },
                    { label: t.dlProgress, ...prog(nc.download_count, nc.download_count_need, (n) => String(n)) },
                    { label: t.seedProgress, ...prog(nc.seed_hours, nc.seed_hours_need, (n) => `${n} ${t.hours}`) },
                    { label: t.ageProgress, ...prog(nc.account_age_days, nc.account_age_days_need, (n) => `${n} ${t.days}`) },
                  ].map((r) => (
                    <div key={r.label} className="flex items-center gap-2 text-xs">
                      <span className="w-16 shrink-0 text-sub">{r.label}</span>
                      <div className="h-2 flex-1 overflow-hidden rounded-full bg-[var(--surface-sunken)]">
                        <div
                          className={`h-full rounded-full ${r.pct >= 100 ? "bg-mint" : "bg-sky"}`}
                          style={{ width: `${r.pct}%` }}
                        />
                      </div>
                      <span className="num w-44 shrink-0 text-right text-sub">
                        {`${r.pct}% · ${r.text}`}
                      </span>
                    </div>
                  ))}
                </div>
              </>
            ) : (
              <p className="py-3 text-center text-xs text-sub">{t.maxLevel}</p>
            )}
          </Card>

          {/* —— 传输与做种（首屏右列，横跨两行，齐平左侧两张卡的总高） —— */}
          <Card title={t.transferSection} icon="⇅" slot="r2">
            <Row label={t.uploaded} icon="↑">
              <span className="num text-mint">{gb(p.uploaded)}</span>
            </Row>
            <Row label={t.downloaded} icon="↓">
              <span className="num text-coral">{gb(p.downloaded)}</span>
            </Row>
            <Row label={t.ratio} icon="≈">
              <span className="num font-bold">{ratio}</span>
            </Row>
            <Row label={t.realRatio} icon="≈">
              <span className="num">
                {`${realRatio}${t.realTraffic}: ${gb(data.real_uploaded)} / ${gb(data.real_downloaded)}`}
              </span>
            </Row>
            <Row label={t.seedSnatchCount} icon="◆">
              <span className="num">{`${p.seeding} / ${p.leeching} / ${data.completed_snatches}`}</span>
            </Row>
            <Row label={t.seedingSize} icon="▣">
              <span className="num">{gb(data.seeding_size)}</span>
            </Row>
            <Row label={t.seedTime} icon="◷">
              <span className="num">{seedDur(data.seed_seconds)}</span>
            </Row>
            <Row label={t.hrField} icon="▲">
              <span className={`num ${data.hr_unresolved > 0 ? "font-bold text-coral" : ""}`}>
                {`${data.hr_unresolved} / ${data.hr_limit}`}
              </span>
            </Row>
            <Row label={t.sparkField} icon="✦">
              <span className="num">{Number(data.spark_balance).toLocaleString("en-US")}</span>
            </Row>
            <Row label={t.seedEarnField} icon="★">
              <span className="num">
                {`${Number(data.month_seed_earn).toLocaleString("en-US")}（${t.thisMonth}）`}
              </span>
            </Row>
          </Card>

          {/* —— 连接信息 —— */}
          <Card title={t.connectInfo} icon="⇄">
            {data.client_agent ? (
              <Row label={t.btClient} icon="⇄">
                <span className="text-xs">{data.client_agent}</span>
              </Row>
            ) : (
              <p className="up-note">{t.noClient}</p>
            )}
            {(data.upload_speed !== null || data.download_speed !== null) && (
              <Row label={t.speed} icon="⇅">
                {`${t.speedUp} ${data.upload_speed ?? 0} ${t.mbps} · ${t.speedDown} ${
                  data.download_speed ?? 0
                } ${t.mbps}`}
              </Row>
            )}
          </Card>

          {/* —— 更多资料 —— */}
          <Card title={t.moreInfo} icon="◍">
            <Row label={t.uploads} icon="☰">
              <span className="num">{p.uploads}</span>
            </Row>
            <Row label={t.comments} icon="✎">
              <span className="num">{p.comments}</span>
            </Row>
            <Row label={t.tabPosts} icon="◈">
              <span className="num">{data.recent_posts.length}</span>
            </Row>
          </Card>

          {/* —— 佩戴勋章（展示位，整行：与上方双列错开，避免留出半格空位） —— */}
          <Card title={t.wornMedals} icon="✪" full>
            {data.worn_medals.length > 0 ? (
              <div className="up-medals">
                {data.worn_medals.map((m) => (
                  <span
                    key={m.id}
                    title={m.description ?? m.name}
                    className="flex items-center gap-1.5 rounded-full border border-line bg-[var(--surface-sunken)] px-2.5 py-0.5 text-xs font-bold text-ink"
                  >
                    <MedalIcon src={m.asset_ref} size={18} title={m.name} />
                    {m.name}
                  </span>
                ))}
              </div>
            ) : (
              <p className="up-note">{t.noMedals}</p>
            )}
          </Card>

          {/* —— 个人简介 / 签名档（有值才展示，整行宽度） —— */}
          {data.info && (
            <Card title={t.infoTitle} icon="✎" full>
              <p className="whitespace-pre-wrap py-2 text-sm">{data.info}</p>
            </Card>
          )}
          {data.signature && (
            <Card title={t.signatureTitle} icon="❞" full>
              <p className="whitespace-pre-wrap border-l-2 border-line py-2 pl-3 text-sm text-sub italic">
                {data.signature}
              </p>
            </Card>
          )}

          {/* —— 近期发布（个人中心保留最近 10 条入口） —— */}
          <Card title={t.recentUploads} icon="☰" full>
            <table className="nexus-table">
              <thead>
                <tr>
                  <td className="colhead">{t.recentUploads}</td>
                  <td className="colhead w-28 text-right">Size</td>
                </tr>
              </thead>
              <tbody>
                {data.recent_uploads.map((u) => (
                  <tr key={u.id}>
                    <td>
                      <Link href={`/torrent/${u.id}`} className="font-bold">
                        {u.name}
                      </Link>
                      {u.small_descr && <p className="text-xs text-sub">{u.small_descr}</p>}
                    </td>
                    <td className="num shrink-0 text-right text-sub">{gb(u.size)}</td>
                  </tr>
                ))}
                {data.recent_uploads.length === 0 && (
                  <tr>
                    <td colSpan={2} className="py-6 text-center text-sub">
                      {t.noUploads}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </Card>
        </div>
      )}

      {active === "uploads" && renderList(torrents?.uploads ?? [], t.noUploads, true)}
      {active === "seeding" && renderList(torrents?.seeding ?? [], t.noUploads)}
      {active === "leeching" && renderList(torrents?.leeching ?? [], t.noLeeching)}
      {active === "completed" && renderList(torrents?.completed ?? [], t.noCompleted)}
      {active === "incomplete" && renderList(torrents?.incomplete ?? [], t.noIncomplete)}
      {active === "preserved" && renderList(torrents?.preserved ?? [], t.noPreserved)}

      {active === "posts" && (
        <section className="nexus-detail">
          <table className="nexus-table">
            <thead>
              <tr>
                <td className="colhead">{t.recentPosts}</td>
              </tr>
            </thead>
            <tbody>
              {data.recent_posts.map(([pid, topicId, body, at]) => (
                <tr key={pid}>
                  <td>
                    <Link href={`/forums/topic/${topicId}`} className="text-xs font-bold text-sky">
                      #{topicId}
                    </Link>
                    <p className="text-sm">{body}</p>
                    <p className="text-[11px] text-sub">
                      {new Date(at).toLocaleString(dateLocale(locale))}
                    </p>
                  </td>
                </tr>
              ))}
              {data.recent_posts.length === 0 && (
                <tr>
                  <td className="py-6 text-center text-sub">{t.noPosts}</td>
                </tr>
              )}
            </tbody>
          </table>
        </section>
      )}

      {active === "comments" && (
        <section className="nexus-detail">
          <table className="nexus-table">
            <thead>
              <tr>
                <td className="colhead">{t.recentComments}</td>
              </tr>
            </thead>
            <tbody>
              {data.recent_comments.map((c, i) => (
                <tr key={i}>
                  <td>
                    <Link href={`/torrent/${c.torrent_id}`} className="text-xs font-bold text-sky">
                      #{c.torrent_id}
                    </Link>
                    <p className="text-sm whitespace-pre-wrap">{c.body}</p>
                    <p className="text-[11px] text-sub">
                      {new Date(c.created_at).toLocaleString(dateLocale(locale))}
                    </p>
                  </td>
                </tr>
              ))}
              {data.recent_comments.length === 0 && (
                <tr>
                  <td className="py-6 text-center text-sub">{t.noComments}</td>
                </tr>
              )}
            </tbody>
          </table>
        </section>
      )}
    </div>
  );
}
