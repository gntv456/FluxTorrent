import { CheckinCard } from "@/components/economy-actions";
import { MyProfileCard } from "@/components/my-profile";
import { PasskeyCard } from "@/components/passkey-card";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

/** 个人中心（M09+）：数据总览 + 签到 + 火花流水 + passkey 管理 */
export default async function MyPage() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.my.title}</h1>
      <MyProfileCard />
      <CheckinCard />
      <PasskeyCard />
    </div>
  );
}
