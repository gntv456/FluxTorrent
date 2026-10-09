import Link from "next/link";
import { notFound } from "next/navigation";
import { api } from "@/lib/api-client";
import { formatBytes } from "@/lib/format";
import { getDict } from "@/i18n/server";
import { SubscribeNetworkButton } from "@/components/subscribe-network-button";

export const dynamic = "force-dynamic";

const ITEM_CLS =
  "flex flex-wrap items-baseline justify-between gap-2 " +
  "rounded-lg border border-line bg-card px-4 py-3 hover:border-sky";
const TITLE_CLS = "min-w-0 flex-1 truncate font-bold";

interface SubGroupDetail {
  id: number;
  kind: string;
  name: string;
  items: {
    id: number;
    name: string;
    size: number;
    seeders: number;
    times_completed: number;
  }[];
}

/** 字幕组页（0334 anime 专项）：该字幕组名下全部过审种。
 *  与出品方页同构（面包屑 + 标题 + 行列表），差别是数据源 kind
 *  = subtitle_group（同一张 content_networks）。
 *  右上角「追番」按钮 = 订该字幕组新作（复用 0332 订阅链路）。 */
export default async function SubtitleGroupPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const gid = Number(id);
  if (!Number.isFinite(gid)) notFound();
  let g: SubGroupDetail;
  try {
    g = await api.get<SubGroupDetail>(
      `/api/v1/networks/${encodeURIComponent(gid)}`,
    );
  } catch {
    notFound();
  }
  const { dict } = await getDict();
  const t = dict.subtitleGroups;

  return (
    <div className="flex flex-col gap-4">
      <nav className="text-sm text-sub" aria-label="breadcrumb">
        <Link href="/subtitle-groups">{t.backToList}</Link>
        <span aria-hidden> › </span>
        <span className="text-body">{g.name}</span>
      </nav>
      <div className="flex flex-wrap items-center gap-3">
        <h1 className="font-display text-2xl">{g.name}</h1>
        <SubscribeNetworkButton networkId={g.id} />
      </div>
      <ul className="flex list-none flex-col gap-2 p-0">
        {g.items.map((it) => (
          <li key={it.id} data-torrent-id={it.id}>
            <Link href={`/torrent/${it.id}`} className={ITEM_CLS}>
              <span className={TITLE_CLS}>{it.name}</span>
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
      {g.items.length === 0 && (
        <p className="py-8 text-center text-sub">{t.emptyItems}</p>
      )}
    </div>
  );
}
