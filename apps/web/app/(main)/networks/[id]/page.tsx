import Link from "next/link";
import { notFound } from "next/navigation";
import { api } from "@/lib/api-client";
import { formatBytes } from "@/lib/format";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

const ITEM_CLS =
  "flex flex-wrap items-baseline justify-between gap-2 " +
  "rounded-lg border border-line bg-card px-4 py-3 hover:border-sky";
const TITLE_CLS = "min-w-0 flex-1 truncate font-bold";

interface NetworkDetail {
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

/** 出品方页（0330 documentary 专项 P1）：该厂牌名下全部过审种。
 *  与合集详情页同形态（面包屑 + 标题 + 行列表），差别是数据源为
 *  /networks/{id}（按 section_dict.name 反查 sections）。 */
export default async function NetworkDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const nid = Number(id);
  if (!Number.isFinite(nid)) notFound();
  let n: NetworkDetail;
  try {
    n = await api.get<NetworkDetail>(
      `/api/v1/networks/${encodeURIComponent(nid)}`,
    );
  } catch {
    notFound();
  }
  const { dict } = await getDict();
  const t = dict.networks;

  return (
    <div className="flex flex-col gap-4">
      <nav className="text-sm text-sub" aria-label="breadcrumb">
        <Link href="/networks">{t.backToList}</Link>
        <span aria-hidden> › </span>
        <span className="text-body">{n.name}</span>
      </nav>
      <h1 className="font-display text-2xl">{n.name}</h1>
      <ul className="flex list-none flex-col gap-2 p-0">
        {n.items.map((it) => (
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
      {n.items.length === 0 && (
        <p className="py-8 text-center text-sub">{t.emptyItems}</p>
      )}
    </div>
  );
}
