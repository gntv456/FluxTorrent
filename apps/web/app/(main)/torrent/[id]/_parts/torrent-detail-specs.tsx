/**
 * 规格徽章条（设计稿 §6：td-chip-row；数值 chip 横排折行）。
 * R3 单源化：维度走 ext.sections（label 由 section_kinds 下发）。
 */

import { Spec } from "@/components/torrent-detail-parts";
import { formatBytes } from "@/lib/format";
import type { Dict } from "@/i18n/zh-CN";
import type { TorrentListItem } from "@fluxtorrent/domain-types";
import type { TorrentDetailExt } from "@/components/torrent-detail-blocks";

export function TorrentSpecs({
  t,
  ext,
  category,
  dict,
}: {
  t: TorrentListItem;
  ext: TorrentDetailExt | null;
  category: string;
  dict: Dict;
}) {
  const d = dict.tdetail;
  const secEntries = Object.entries(ext?.sections ?? {})
    .map(([kind, v]) => ({ kind, ...v }))
    .filter((v) => v.name);
  return (
    <section className="td-specs nexus-detail">
      <Spec value={formatBytes(t.size)} label={dict.torrent.size} num />
      <Spec value={ext?.numfiles ?? "—"} label={dict.torrent.numFiles} num />
      <Spec value={category} label={dict.torrent.category} />
      {secEntries.map((v) => (
        <Spec key={v.kind} value={v.name} label={v.label} />
      ))}
      {t.rating && <Spec value={t.rating} label={d?.ratingLabel} num />}
    </section>
  );
}
