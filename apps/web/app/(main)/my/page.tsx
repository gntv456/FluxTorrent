import Link from "next/link";
import { UsercpPanel, type UsercpTab } from "@/components/usercp";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

const VALID_TABS: UsercpTab[] = ["overview", "personal", "tracker", "forum", "security"];

/** 控制面板 —— 像素级复刻 NexusPHP usercp：侧边六项导航 + 账户概览/四组设定 */
export default async function MyPage({
  searchParams,
}: {
  searchParams: Promise<{ tab?: string }>;
}) {
  const { dict } = await getDict();
  const sp = await searchParams;
  const tab = VALID_TABS.includes(sp.tab as UsercpTab)
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
