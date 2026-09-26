import Link from "next/link";
import { UsercpPanel, type UsercpTab } from "@/components/usercp";
import { getDict } from "@/i18n/server";
import { getSiteProfile } from "@/lib/site-profile";

export const dynamic = "force-dynamic";

const VALID_TABS: UsercpTab[] = [
  "overview",
  "personal",
  "tracker",
  "forum",
  "security",
  "wishlist",
];

/** 控制面板 —— 像素级复刻 NexusPHP usercp：侧边六项导航 + 账户概览/四组设定 */
export default async function MyPage({
  searchParams,
}: {
  searchParams: Promise<{ tab?: string }>;
}) {
  const { dict } = await getDict();
  const profile = await getSiteProfile().catch(() => null);
  // 心愿单 tab 受模块开关（0209 P2-15：此前关模块后 tab 仍在，点了看空面板）
  const wishlistOn = profile?.modules?.wishlist !== false;
  const valid = wishlistOn
    ? VALID_TABS
    : VALID_TABS.filter((t) => t !== "wishlist");
  const sp = await searchParams;
  const tab = valid.includes(sp.tab as UsercpTab)
    ? (sp.tab as UsercpTab)
    : "overview";
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.my.center}</h1>
      <UsercpPanel initialTab={tab} />
      <p className="text-xs text-sub">
        <Link href="/faq" className="faqlink">
          {dict.my.center}
        </Link>
      </p>
    </div>
  );
}
