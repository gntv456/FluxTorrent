import Link from "next/link";
import { TorrentListClient } from "@/components/torrent-list-client";
import { getDict } from "@/i18n/server";
import { PageHeader } from "@/components/ui/page-header";
import { TabPanel, Tabs } from "@/components/ui/tabs";
import { BTN_SM_BOLD } from "@/lib/ui-classes";

export const dynamic = "force-dynamic";

/** 我的种子列表（NP getusertorrentlist 口径）：做种中 / 已完成 / 我的发布。
 *  数据来自 GET /me/torrentlist（kind + page/per，2026-10-17 起带 total）。
 *  标签组用共享的 `Tabs` 链接档（此前这里手写一排 rounded-full pill，
 *  与全站标签外观不一致）。 */
export default async function MyTorrentlistPage({
  searchParams,
}: {
  searchParams: Promise<{ kind?: string }>;
}) {
  const { kind } = await searchParams;
  const { dict } = await getDict();
  const t = dict.mytl;
  const k = kind === "completed" || kind === "uploads" ? kind : "seeding";

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        eyebrow={`${dict.my.title} / ${t.title}`}
        title={t.title}
        sub={t.subtitle}
        aside={
          <Link href="/upload" className={BTN_SM_BOLD}>
            {t.newTorrent}
          </Link>
        }
      />

      <Tabs
        idPrefix="mtl"
        label={t.title}
        value={k}
        items={[
          { key: "seeding", label: t.tabSeeding },
          { key: "completed", label: t.tabCompleted },
          { key: "uploads", label: t.tabUploads },
        ]}
        hrefs={[
          "/my/torrentlist",
          "/my/torrentlist?kind=completed",
          "/my/torrentlist?kind=uploads",
        ]}
      />

      <TabPanel idPrefix="mtl" tabKey={k}>
        <TorrentListClient kind={k} emptyText={t.empty} />
      </TabPanel>
      <p className="text-xs text-sub">{t.note}</p>
    </div>
  );
}
