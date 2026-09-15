import { RssBuilder } from "@/components/rss-builder";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

/** 获取 RSS（好学站 getrss.php 复刻）：筛选条件 → 生成带 passkey 的个性化订阅链接 */
export default async function GetRssPage() {
  const { dict, currency } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.getrss.title}</h1>
        <span className="text-sm text-sub">{dict.getrss.subtitle}</span>
      </div>
      <RssBuilder loginToView={dict.my.loginToView.replace("{magic}", currency)} />
    </div>
  );
}
