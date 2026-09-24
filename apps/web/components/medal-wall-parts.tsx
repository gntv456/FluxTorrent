import type { Dict } from "@/i18n/zh-CN";
import { dateLocale, type Locale } from "@/i18n/config";
import type { MedalRarity } from "@/lib/medal-rarity";
import { MedalBadge } from "@/components/medal-badge";

// 勋章墙展示层（从 medal-wall.tsx 拆出守 300 行上限）。
// 主组件只管取数与筛选状态，这里只管怎么画。

export interface WallMedal {
  medal_name: string;
  asset_ref?: string | null;
  rarity?: string | null;
  wearing: boolean;
  granted_at: string;
}

export interface WallUser {
  user_id: number;
  username: string;
  medal_count: number;
  medals: WallMedal[];
}

export type WallDict = Dict["medalwall"];

/** 稀有度分布条 + 图例：把「10 枚」拆成「史诗 5 / 稀有 4 / 普通 1」的比例条。
 *  段间 2px 缝（CSS gap），故 1x 下也能看出分段；颜色取词表 tone，与角标同源。 */
export function RarityBar({
  medals,
  list,
}: {
  medals: WallMedal[];
  list: MedalRarity[];
}) {
  const rows = list
    .map((r) => ({
      tone: r.tone,
      label: r.label,
      n: medals.filter((m) => m.rarity === r.value).length,
    }))
    .filter((r) => r.n > 0);
  if (rows.length === 0) return null;
  const total = medals.length || 1;
  return (
    <>
      <div className="rbar">
        {rows.map((r) => (
          <i
            key={r.tone}
            data-tone={r.tone}
            style={{ width: `${(r.n / total) * 100}%` }}
          />
        ))}
      </div>
      <div className="rbar-legend">
        {rows.map((r) => (
          <span key={r.tone}>
            <i className="dot" data-tone={r.tone} />
            {r.label} {r.n}
          </span>
        ))}
      </div>
    </>
  );
}

/** 勋章名首字做头像占位（与 MedalBadge 的无图兜底同思路，避免全站同一字形） */
function avatarText(username: string): string {
  const s = (username ?? "").trim();
  return s ? s.slice(0, 2).toUpperCase() : "?";
}

function latestGrant(medals: WallMedal[]): string {
  let best = "";
  for (const m of medals) if (m.granted_at > best) best = m.granted_at;
  return best;
}

export function fmtDay(iso: string, locale: Locale): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleDateString(dateLocale(locale));
}

/** 一卡一人：持勋数 + 稀有度分布 + 徽章槽位（悬停出档案）+ 档案入口 */
export function HonorCard({
  user,
  rank,
  list,
  t,
  locale,
  only,
}: {
  user: WallUser;
  rank: number;
  list: MedalRarity[];
  t: WallDict;
  locale: Locale;
  /** 稀有度筛选选中的 value（null = 全部） */
  only: string | null;
}) {
  const shown = only
    ? user.medals.filter((m) => m.rarity === only)
    : user.medals;
  if (shown.length === 0) return null;
  const recent = fmtDay(latestGrant(shown), locale);
  return (
    <article className="honor">
      <div className="honor-hd">
        <span className="honor-ava">{avatarText(user.username)}</span>
        <div className="honor-who">
          <a className="honor-nm" href={`/users/${user.user_id}`}>
            {user.username}
          </a>
          <span className="honor-rank">
            {t.rankNo.replace("{n}", String(rank))}
          </span>
        </div>
        <div className="honor-cnt">
          <b>{shown.length}</b>
          <span>{t.unitMedal}</span>
        </div>
      </div>
      <RarityBar medals={shown} list={list} />
      <div className="slots">
        {shown.map((m) => (
          <span className="slot" key={m.medal_name}>
            <MedalBadge
              name={m.medal_name}
              assetRef={m.asset_ref}
              rarity={m.rarity}
              list={list}
              worn={m.wearing}
              wornLabel={t.wearing}
            />
            <span className="slot-tip">
              {m.medal_name}
              {" · "}
              <em>
                {m.rarity
                  ? list.find((r) => r.value === m.rarity)?.label
                  : ""}
              </em>
              <br />
              {t.grantedAt.replace("{date}", fmtDay(m.granted_at, locale))}
            </span>
          </span>
        ))}
      </div>
      <div className="honor-ft">
        <a className="lk" href={`/users/${user.user_id}`}>
          {t.profile}
        </a>
        {recent && (
          <span className="tail">
            {t.recentAt.replace("{date}", recent)}
          </span>
        )}
      </div>
    </article>
  );
}

/** 页面级口径统计：全部是「服务端可准确给出」或「明确标注本页」的指标 */
export function WallStats({
  kinds,
  tiers,
  users,
  pageMedals,
  t,
}: {
  kinds: number;
  tiers: number;
  users: number;
  pageMedals: number;
  t: WallDict;
}) {
  return (
    <div className="pgstats">
      <div>
        <div className="k">{t.statKinds}</div>
        <div className="v">
          {kinds} <small>{t.unitKind}</small>
        </div>
      </div>
      <div>
        <div className="k">{t.statTiers}</div>
        <div className="v">
          {tiers} <small>{t.unitTier}</small>
        </div>
      </div>
      <div>
        <div className="k">{t.statUsers}</div>
        <div className="v">
          {users} <small>{t.unitUser}</small>
        </div>
      </div>
      <div>
        <div className="k">{t.statPageMedals}</div>
        <div className="v">
          {pageMedals} <small>{t.unitMedal}</small>
        </div>
      </div>
    </div>
  );
}

/** 稀有度筛选 chips（按词表生成，站长加档位即多一个 chip） */
export function RarityChips({
  list,
  counts,
  value,
  onChange,
  allLabel,
  total,
}: {
  list: MedalRarity[];
  counts: Record<string, number>;
  value: string | null;
  onChange: (v: string | null) => void;
  allLabel: string;
  total: number;
}) {
  const chip = (v: string | null, label: string, n: number, tone?: string) => (
    <button
      key={v ?? "all"}
      type="button"
      className="chip-sel"
      aria-pressed={value === v}
      onClick={() => onChange(v)}
    >
      {tone && <i className="dot" data-tone={tone} />}
      {label} <span>{n}</span>
    </button>
  );
  return (
    <>
      {chip(null, allLabel, total)}
      {list.map((r) =>
        chip(r.value, r.label, counts[r.value] ?? 0, r.tone),
      )}
    </>
  );
}
