import Link from "next/link";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

const CARD_CLS =
  "block rounded-xl border border-line bg-card p-4 " +
  "transition-transform hover:-translate-y-0.5";

interface SubGroupItem {
  id: number;
  name: string;
  torrents: number;
}

/** 字幕组聚合页（0334 anime 专项）。
 *  动漫站按字幕组找片（「Sakura 做了哪些番」）是高频入口。
 *  subtitle_group 是 multiselect 维度，锚点行已由 0333 回填进
 *  content_networks（kind='subtitle_group'）——本页只是读口：
 *  复用 /networks 的 kind 参数，无需独立后端。 */
export default async function SubtitleGroupsPage() {
  const { dict } = await getDict();
  const t = dict.subtitleGroups;
  let items: SubGroupItem[] = [];
  let failed = false;
  try {
    const r = await api.get<{ items: SubGroupItem[] }>(
      "/api/v1/networks?kind=subtitle_group&limit=100",
    );
    items = r.items ?? [];
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
      {!failed && items.length === 0 && (
        <p className="py-8 text-center text-sub">{t.empty}</p>
      )}
      <ul className="grid list-none gap-3 p-0 sm:grid-cols-2 xl:grid-cols-3">
        {items.map((g) => (
          <li key={g.id} data-subgroup-id={g.id}>
            <Link href={`/subtitle-groups/${g.id}`} className={CARD_CLS}>
              <div className="flex items-center justify-between gap-2">
                <b className="clamp2 min-w-0 flex-1">{g.name}</b>
                <span className="shrink-0 text-xs text-sub">
                  {t.countPrefix}
                  {g.torrents}
                </span>
              </div>
            </Link>
          </li>
        ))}
      </ul>
    </div>
  );
}
