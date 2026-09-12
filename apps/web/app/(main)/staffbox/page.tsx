import { StaffBox } from "@/components/staff-box";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

/** 管理组信箱（好学站 staffbox.php 复刻）：与管理组的往来短讯 */
export default async function StaffBoxPage() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.staffbox.title}</h1>
      <StaffBox />
    </div>
  );
}
