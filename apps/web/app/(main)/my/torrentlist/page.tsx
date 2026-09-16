import Link from "next/link";
import { TorrentListClient } from "@/components/torrent-list-client";
import { getDict } from "@/i18n/server";
import { formatBytes } from "@/lib/format";

export const dynamic = "force-dynamic";

/** 我的种子列表（NP getusertorrentlist 口径）：做种中 / 已完成 / 我的发布 三 tab。
 *  数据来自已有 GET /me/torrentlist（kind=seeding|completed|uploads）。 */
export default async function MyTorrentlistPage({
  searchParams,
}: {
  searchParams: Promise<{ kind?: string }>;
}) {
  const { kind } = await searchParams;
  const { dict } = await getDict();
  const k = kind === "completed" || kind === "uploads" ? kind : "seeding";

  const tabs = [
    { key: "seeding", label: dict.mytl.tabSeeding, href: "/my/torrentlist" },
    {
      key: "completed",
      label: dict.mytl.tabCompleted,
      href: "/my/torrentlist?kind=completed",
    },
    {
      key: "uploads",
      label: dict.mytl.tabUploads,
      href: "/my/torrentlist?kind=uploads",
    },
  ];

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.mytl.title}</h1>
        <span className="text-sm text-sub">{dict.mytl.subtitle}</span>
      </div>
      <nav className="flex flex-wrap gap-2" aria-label={dict.mytl.title}>
        {tabs.map((t) => (
          <Link
            key={t.key}
            href={t.href}
            aria-current={t.key === k ? "page" : undefined}
            className={`min-h-[36px] rounded-full px-4 text-sm leading-9 ${
              t.key === k
                ? "bg-sky-deep text-white"
                : "border border-line text-sub hover:border-sky hover:text-sky"
            }`}
          >
            {t.label}
          </Link>
        ))}
      </nav>
      <TorrentListClient kind={k} emptyText={dict.mytl.empty} />
      <p className="text-xs text-sub">{dict.mytl.note}</p>
    </div>
  );
}
