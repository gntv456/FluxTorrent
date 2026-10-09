import Link from "next/link";
import { api } from "@/lib/api-client";
import { formatBytes } from "@/lib/format";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

const ROW_CLS =
  "flex flex-wrap items-baseline justify-between gap-2 " +
  "rounded-lg border border-line bg-card px-4 py-3 hover:border-sky";
const TITLE_CLS = "min-w-0 flex-1 truncate font-bold";
const TAB_CLS = "rounded-full border border-line px-3 py-0.5 text-xs";
const TAB_ON_CLS =
  "rounded-full bg-[var(--baozi-orange)] px-3 py-0.5 text-xs " +
  "font-bold text-white";

interface SeasonItem {
  id: number;
  name: string;
  size: number;
  seeders: number;
  times_completed: number;
  group_id: number | null;
  ep_last: number | null;
  subtitle_groups: string[];
}
interface SeasonalResp {
  season: string;
  seasons: { id: number; name: string; torrents: number }[];
  items: SeasonItem[];
}

/** 当季新番（0334 anime 专项）：按播出季浏览番剧。
 *  动漫站的门面——蜜柑 / AnimeBytes / 动漫花园都按播出季分栏。
 *  顶部季切换条（仅有内容的季）+ 该季番剧行（同番同话多字幕组并列）。 */
export default async function SeasonalPage({
  searchParams,
}: {
  searchParams: Promise<{ season?: string }>;
}) {
  const { season } = await searchParams;
  const { dict } = await getDict();
  const t = dict.seasonal;
  const qs = season ? `?season=${encodeURIComponent(season)}` : "";
  let r: SeasonalResp = { season: "", seasons: [], items: [] };
  let failed = false;
  try {
    r = await api.get<SeasonalResp>(`/api/v1/seasonal${qs}`);
  } catch {
    failed = true;
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{t.title}</h1>
        <span className="text-sm text-sub">{t.subtitle}</span>
      </div>

      {failed && <p className="text-sm text-sub">{t.loadFailed}</p>}

      {r.seasons.length > 0 && (
        <div className="flex flex-wrap items-center gap-2" data-season-tabs>
          {r.seasons.map((s) => (
            <Link
              key={s.id}
              href={`/seasonal?season=${encodeURIComponent(s.name)}`}
              className={s.name === r.season ? TAB_ON_CLS : TAB_CLS}
              data-season={s.name}
            >
              {s.name} · {s.torrents}
            </Link>
          ))}
        </div>
      )}

      {!failed && r.items.length === 0 && (
        <p className="py-8 text-center text-sub">
          {r.season ? `${r.season} ${t.emptySeason}` : t.empty}
        </p>
      )}

      <ul className="flex list-none flex-col gap-2 p-0" data-season-items>
        {r.items.map((it) => (
          <li key={it.id} data-torrent-id={it.id}>
            <Link href={`/torrent/${it.id}`} className={ROW_CLS}>
              <span className={TITLE_CLS}>{it.name}</span>
              {it.ep_last !== null && (
                <span className="text-xs text-sub">
                  {t.epPrefix}
                  {it.ep_last}
                  {t.epSuffix}
                </span>
              )}
              {it.subtitle_groups.length > 0 && (
                <span className="text-xs text-sub" data-subs>
                  {it.subtitle_groups.join(" / ")}
                </span>
              )}
              <span className="text-xs text-sub">
                {" "}
                🌱 {it.seeders} · ✅ {it.times_completed}
                {" · "}
                {formatBytes(it.size)}
              </span>
            </Link>
          </li>
        ))}
      </ul>
    </div>
  );
}
