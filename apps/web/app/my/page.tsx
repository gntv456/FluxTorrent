import { CheckinCard } from "@/components/economy-actions";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

/** 个人中心（M09+）：签到 + 数据总览（经济数据客户端拉取） */
export default async function MyPage() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.my.title}</h1>
      <CheckinCard />
    </div>
  );
}
