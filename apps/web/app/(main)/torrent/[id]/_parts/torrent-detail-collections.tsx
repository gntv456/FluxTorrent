import type { Dict } from "@/i18n/server";
import { CollectionTagLink } from "@/components/collection-tag-link";

/** 所属合集（0157：一种可入多个合集/系列）。
 *  从 page.tsx 按域外移（详情页基线只许瘦不许胖）；显隐门仍由调用方
 *  的 sectOn("collections") 决定，本组件只判「有没有数据」。 */
export function TorrentCollections({
  items,
  dict,
}: {
  items: { id: number; kind: string; name: string }[];
  dict: Dict;
}) {
  if (items.length === 0) return null;
  return (
    <section className="td-collections nexus-detail">
      <h2 className="td-sec-title">{dict.collections.inTitle}</h2>
      <div className="td-collections__list">
        {items.map((c) => (
          <CollectionTagLink
            key={c.id}
            href={`/collections/${c.id}`}
            name={c.name}
            series={c.kind === "series"}
          />
        ))}
      </div>
    </section>
  );
}
