import Link from "next/link";
import { UsercpPanel, type Overview } from "@/components/usercp";
import { USERCP_NAV, type UsercpTab } from "@/lib/usercp-nav";
import { PageHeader } from "@/components/ui/page-header";
import { Badge } from "@/components/ui/badge";
import { getDict } from "@/i18n/server";
import { accountHealthCopy } from "@/i18n/account-health";
import { getSiteProfile } from "@/lib/site-profile";
import { api } from "@/lib/api-client";

export const dynamic = "force-dynamic";

const VALID_TABS: UsercpTab[] = [
  "overview",
  "personal",
  // 「自定义字段」此前只在 USERCP_NAV 与 UsercpPanel 里存在，这一份白名单漏了它
  // ⇒ 侧栏点得动、`/my?tab=fields` 却静默回落概览（样图 § ucp 的七档之一）。
  "fields",
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
  const { dict, locale } = await getDict();
  const profile = await getSiteProfile().catch(() => null);
  // 页面头所需的身份信息（样图 § pg-head：面包屑 + 标题 + 描述 + 右侧徽章）。
  // 取不到就降级为无描述，绝不因此挡住控制面板本身。
  const ov = await api.get<Overview>("/api/v1/me/overview").catch(() => null);
  // 心愿单 tab 受模块开关（0209 P2-15：此前关模块后 tab 仍在，点了看空面板）
  const wishlistOn = profile?.modules?.wishlist !== false;
  const valid = wishlistOn
    ? VALID_TABS
    : VALID_TABS.filter((t) => t !== "wishlist");
  const sp = await searchParams;
  const tab = valid.includes(sp.tab as UsercpTab)
    ? (sp.tab as UsercpTab)
    : "overview";
  // 侧边栏与页面级过滤同源（0209 P2-15：客户端 setTab 绕过了页面滤 tab）
  const nav = wishlistOn
    ? USERCP_NAV
    : USERCP_NAV.filter((n) => n.key !== "wishlist");
  // 站龄（整年）：描述行「用户名 · 等级 · N 年站龄」（样图措辞）
  const years = ov?.created_at
    ? Math.max(
        0,
        Math.floor(
          (Date.now() - new Date(ov.created_at).getTime()) / 31557600000,
        ),
      )
    : null;
  const sub = ov
    ? [
        ov.username,
        ov.class_name ?? null,
        years === null ? null : `${years} ${dict.my.yearsSuffix}`,
      ]
        .filter(Boolean)
        .join(" · ")
    : undefined;
  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        eyebrow={`${dict.my.homeCrumb} / ${dict.my.center}`}
        title={dict.my.center}
        sub={sub}
        aside={
          ov?.email ? (
            <Badge tone="brand">{dict.my.emailBound}</Badge>
          ) : undefined
        }
      />
      <UsercpPanel initialTab={tab} nav={nav} />
      <p className="flex flex-wrap gap-x-3 text-xs text-sub">
        <Link href="/account-health" className="faqlink">
          {accountHealthCopy[locale].title}
        </Link>
        <Link href="/faq" className="faqlink">
          {dict.faq.title}
        </Link>
      </p>
    </div>
  );
}
